use axum::body::to_bytes;
use sha2::{Digest, Sha256};
use storyos_application::{
    AuthorCommandAdmissionIds, EditorClientBinding, ProjectAssistanceRecord,
    ProjectCommandChallengeBinding, UpdateProjectAssistanceCommand, UpdateProjectAssistanceError,
    UpdateProjectAssistanceSettlementEffect, open_project_assistance,
};
use storyos_core::AssistanceAvailability;

use super::editor_session::{exact_header, session_binding_ref};
use super::project_command_challenge::{
    hex_bytes, plain_digest, valid_uuid_v7, validate_json_content_type,
};
use super::*;

pub(super) async fn get_project_assistance(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<contracts::GetProjectAssistanceResponse>, ApiError> {
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    let reader = project_reader(&state).await?;
    let Some(assistance) = open_project_assistance(&reader, &scope)
        .await
        .map_err(service_unavailable)?
    else {
        return Err(resource_unavailable());
    };
    Ok(Json(contracts::GetProjectAssistanceResponse {
        schema_id: contracts::GET_PROJECT_ASSISTANCE_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: Uuid::now_v7().to_string(),
        project_scope: contract_scope(&scope),
        assistance: contract_assistance(&assistance),
    }))
}

pub(super) async fn update_project_assistance(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    request: Request,
) -> Result<Json<contracts::UpdateProjectAssistanceResponse>, ApiError> {
    let (parts, body_stream) = request.into_parts();
    let headers = parts.headers;
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::StateChanging,
    )?;
    validate_json_content_type(&headers)?;
    let bytes = to_bytes(body_stream, contracts::AUTHOR_EDIT_MAX_WIRE_BODY_BYTES)
        .await
        .map_err(|_| payload_too_large())?;
    let body = serde_json::from_slice::<contracts::UpdateProjectAssistanceRequest>(&bytes)
        .map_err(|_| invalid_request_shape())?;
    let input = &body.update_project_assistance_input;
    let Some(expected_revision) = input.expected_assistance_revision.parse::<u64>().ok() else {
        return Err(invalid_request());
    };
    let session_handle = session_cookie(&headers).ok_or_else(authentication_required)?;
    let session = state
        .client_session_binding(session_handle)
        .ok_or_else(authentication_required)?;
    if body.command_schema != contracts::UPDATE_PROJECT_ASSISTANCE_REQUEST_SCHEMA_ID
        || input.client_contract_revision != session.client_contract_revision
        || input.security_policy_revision != session.security_policy_revision
    {
        return Err(invalid_request());
    }
    valid_uuid(&input.correlation_id)?;
    let idempotency_key = exact_header(&headers, "idempotency-key")?;
    let nonce = exact_header(&headers, "x-storyos-anti-forgery")?;
    if !valid_uuid_v7(idempotency_key)
        || nonce.len() != 64
        || !nonce
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(invalid_request());
    }
    let secret = state
        .config
        .project_command_challenge_secret
        .as_deref()
        .filter(|secret| secret.len() >= 32)
        .ok_or_else(challenge_store_unavailable)?;
    let binding_ref = session_binding_ref(secret, session_handle);
    let canonical_command_bytes = canonical_body_bytes(&body)?;
    let digest_hex = hex_bytes(&Sha256::digest(&canonical_command_bytes));
    let canonical_command_digest = format!(
        "sha256:{}:{digest_hex}",
        contracts::UPDATE_PROJECT_ASSISTANCE_DIGEST_PROFILE
    );
    let store = project_reader(&state).await?;
    let settlement = storyos_application::update_project_assistance(
        &store,
        &UpdateProjectAssistanceCommand {
            project_scope: scope.clone(),
            client_binding: EditorClientBinding {
                binding_ref: binding_ref.clone(),
                session_generation: session.session_generation,
                client_contract_revision: session.client_contract_revision.clone(),
                security_policy_revision: session.security_policy_revision.clone(),
            },
            challenge_binding: ProjectCommandChallengeBinding {
                project_scope: scope.clone(),
                client_session_binding_digest: binding_ref,
                client_session_generation: session.session_generation,
                client_contract_revision: session.client_contract_revision.clone(),
                security_policy_revision: session.security_policy_revision.clone(),
                limit_profile_revision: contracts::LIMIT_PROFILE_REVISION.to_owned(),
                challenge_rate_policy_revision:
                    storyos_application::PROJECT_COMMAND_CHALLENGE_RATE_POLICY_REVISION.to_owned(),
                method: contracts::UPDATE_PROJECT_ASSISTANCE_METHOD.to_owned(),
                route_template: contracts::UPDATE_PROJECT_ASSISTANCE_PATH.to_owned(),
                command_schema: body.command_schema.clone(),
                command_kind: "updateProjectAssistance".to_owned(),
                canonical_command_digest: canonical_command_digest.clone(),
                idempotency_key: idempotency_key.to_owned(),
            },
            nonce_digest: plain_digest(nonce.as_bytes()),
            canonical_command_bytes,
            correlation_id: input.correlation_id.clone(),
            availability: match input.availability {
                contracts::ProjectAssistanceAvailability::Available => {
                    AssistanceAvailability::Available
                }
                contracts::ProjectAssistanceAvailability::Unavailable => {
                    AssistanceAvailability::Unavailable
                }
            },
            expected_revision,
            ids: AuthorCommandAdmissionIds {
                command_id: Uuid::now_v7().to_string(),
                author_command_admission_id: Uuid::now_v7().to_string(),
                receipt_id: Uuid::now_v7().to_string(),
            },
        },
    )
    .await
    .map_err(update_project_assistance_error)?;
    super::acknowledgement_hold::hold_first_acknowledgement_if_requested(idempotency_key).await;
    update_project_assistance_response(
        &scope,
        &input.correlation_id,
        &digest_hex,
        idempotency_key,
        settlement,
    )
}

fn update_project_assistance_response(
    scope: &ApplicationScope,
    correlation_id: &str,
    digest_hex: &str,
    idempotency_key: &str,
    settlement: storyos_application::UpdateProjectAssistanceSettlement,
) -> Result<Json<contracts::UpdateProjectAssistanceResponse>, ApiError> {
    let project = settlement.response_project;
    let (receipt_result, effect) = match settlement.effect {
        UpdateProjectAssistanceSettlementEffect::Initialized {
            availability,
            revision,
        } => (
            contracts::DomainReceiptResult::AuthoritativeApplied,
            contracts::UpdateProjectAssistanceEffect::Initialized {
                availability: contract_availability(availability),
                revision: revision.to_string(),
                project_activity_position: settlement.project_activity_position.to_string(),
            },
        ),
        UpdateProjectAssistanceSettlementEffect::Applied {
            availability,
            revision,
        } => (
            contracts::DomainReceiptResult::AuthoritativeApplied,
            contracts::UpdateProjectAssistanceEffect::AuthoritativeApplied {
                availability: contract_availability(availability),
                revision: revision.to_string(),
                project_activity_position: settlement.project_activity_position.to_string(),
            },
        ),
        UpdateProjectAssistanceSettlementEffect::NoEffect { reason } => (
            contracts::DomainReceiptResult::NoEffect,
            contracts::UpdateProjectAssistanceEffect::NoEffect {
                reason: match reason {
                    storyos_core::UpdateProjectAssistanceNoEffect::AvailabilityUnchanged => {
                        contracts::UpdateProjectAssistanceNoEffectReason::AvailabilityUnchanged
                    }
                },
            },
        ),
        UpdateProjectAssistanceSettlementEffect::Conflicted { reason } => (
            contracts::DomainReceiptResult::Conflicted,
            contracts::UpdateProjectAssistanceEffect::Conflicted {
                reason: match reason {
                    storyos_core::UpdateProjectAssistanceConflict::StaleAssistanceRevision => {
                        contracts::UpdateProjectAssistanceConflictReason::StaleAssistanceRevision
                    }
                },
            },
        ),
        UpdateProjectAssistanceSettlementEffect::Refused { .. } => {
            return Err(update_project_assistance_error(
                UpdateProjectAssistanceError::BindingConflict,
            ));
        }
    };
    let Some(assistance) = settlement.assistance else {
        return Err(problem(
            StatusCode::CONFLICT,
            "stale_assistance_revision",
            "The Project assistance revision is stale.",
        ));
    };
    let contract_project_scope = contract_scope(scope);
    Ok(Json(contracts::UpdateProjectAssistanceResponse {
        schema_id: contracts::UPDATE_PROJECT_ASSISTANCE_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: correlation_id.to_owned(),
        project_scope: contract_project_scope.clone(),
        command_id: settlement.ids.command_id,
        author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
        receipt: contracts::DomainReceipt {
            receipt_id: settlement.ids.receipt_id.clone(),
            project_scope: contract_project_scope,
            command_kind: contracts::DomainReceiptCommandKind::UpdateProjectAssistance,
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: contracts::UPDATE_PROJECT_ASSISTANCE_DIGEST_PROFILE.to_owned(),
                value_hex_lowercase: digest_hex.to_owned(),
            },
            idempotency_key: idempotency_key.to_owned(),
            producer_cause: contracts::DomainReceiptProducerCause::AuthorCommandAdmission,
            author_command_admission_id: settlement.ids.author_command_admission_id,
            expected_heads: Vec::new(),
            prior_heads: Vec::new(),
            resulting_heads: Vec::new(),
            authoritative_revision_ids: Vec::new(),
            proposal_revision_ids: Vec::new(),
            authoritative_commit_ids: Vec::new(),
            author_action_sequence: None,
            draft_artifact_refs: Vec::new(),
            artifact_lifecycle_event_refs: Vec::new(),
            condition_refs: Vec::new(),
            result: receipt_result,
            created_at: settlement.receipt_created_at,
        },
        project: contracts::ControlledProject {
            project_id: project.project_id.as_ref().to_owned(),
            title: project.title,
            open: match project.current_chapter_id {
                Some(chapter_id) => contracts::ProjectOpenState::CurrentChapter {
                    current_chapter_id: chapter_id.as_ref().to_owned(),
                },
                None => contracts::ProjectOpenState::Empty,
            },
        },
        assistance: contract_assistance(&assistance),
        effect,
    }))
}

fn contract_assistance(record: &ProjectAssistanceRecord) -> contracts::ProjectAssistanceBinding {
    contracts::ProjectAssistanceBinding {
        availability: contract_availability(record.availability),
        revision: record.revision.to_string(),
        model_registration_revision: record.model_registration_revision.clone(),
        processing_destination_identity: record.processing_destination_identity.clone(),
        processing_destination_identity_evidence_revision: record
            .processing_destination_identity_evidence_revision
            .to_string(),
        project_model_use_binding_revision: record.project_model_use_binding_revision.clone(),
        external_compatibility_decision: record.external_compatibility_decision.clone(),
    }
}

fn contract_availability(
    availability: AssistanceAvailability,
) -> contracts::ProjectAssistanceAvailability {
    match availability {
        AssistanceAvailability::Available => contracts::ProjectAssistanceAvailability::Available,
        AssistanceAvailability::Unavailable => {
            contracts::ProjectAssistanceAvailability::Unavailable
        }
    }
}

fn canonical_body_bytes(
    body: &contracts::UpdateProjectAssistanceRequest,
) -> Result<Vec<u8>, ApiError> {
    let canonical =
        canonical_json(serde_json::to_value(body).map_err(|_| invalid_request_shape())?);
    serde_json::to_vec(&canonical).map_err(|_| invalid_request_shape())
}

fn canonical_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(canonical_json).collect())
        }
        serde_json::Value::Object(values) => serde_json::Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, canonical_json(value)))
                .collect(),
        ),
        scalar => scalar,
    }
}

fn update_project_assistance_error(error: UpdateProjectAssistanceError) -> ApiError {
    match error {
        UpdateProjectAssistanceError::BindingConflict => problem(
            StatusCode::CONFLICT,
            "idempotency_binding_conflict",
            "The Update Project Assistance binding conflicts.",
        ),
        UpdateProjectAssistanceError::HistoricalAcknowledgementUnavailable => problem(
            StatusCode::CONFLICT,
            "historical_acknowledgement_unavailable",
            "The original Update Project Assistance acknowledgement cannot be recovered. Refresh to inspect the current Project.",
        ),
        UpdateProjectAssistanceError::InvalidChallenge => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "challenge_invalid",
            "The Update Project Assistance challenge is invalid.",
        ),
        UpdateProjectAssistanceError::MissingProject => resource_unavailable(),
        UpdateProjectAssistanceError::Unavailable(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "project_store_unavailable",
            "The Project store is unavailable.",
        ),
    }
}
