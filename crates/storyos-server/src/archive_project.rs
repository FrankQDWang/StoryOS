use axum::body::to_bytes;
use sha2::{Digest, Sha256};
use storyos_application::{
    ArchiveProjectCommand, ArchiveProjectError, ArchiveProjectSettlementEffect,
    AuthorCommandAdmissionIds, EditorClientBinding, ProjectCommandChallengeBinding,
};

use super::editor_session::{exact_header, session_binding_ref};
use super::project_command_challenge::{
    hex_bytes, plain_digest, valid_uuid_v7, validate_json_content_type,
};
use super::*;

pub(super) async fn archive_project(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    request: Request,
) -> Result<Json<contracts::ArchiveProjectResponse>, ApiError> {
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
    let body = serde_json::from_slice::<contracts::ArchiveProjectRequest>(&bytes)
        .map_err(|_| invalid_request_shape())?;
    let input = &body.archive_project_input;
    let Some(expected_revision) = input
        .expected_project_revision
        .parse::<u64>()
        .ok()
        .filter(|revision| *revision >= 1)
    else {
        return Err(invalid_request());
    };
    let session_handle = session_cookie(&headers).ok_or_else(authentication_required)?;
    let session = state
        .client_session_binding(session_handle)
        .ok_or_else(authentication_required)?;
    if body.command_schema != contracts::ARCHIVE_PROJECT_REQUEST_SCHEMA_ID
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
        contracts::ARCHIVE_PROJECT_DIGEST_PROFILE
    );
    let store = project_reader(&state)?;
    let settlement = storyos_application::archive_project(
        &store,
        &ArchiveProjectCommand {
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
                method: contracts::ARCHIVE_PROJECT_METHOD.to_owned(),
                route_template: contracts::ARCHIVE_PROJECT_PATH.to_owned(),
                command_schema: body.command_schema.clone(),
                command_kind: "archiveProject".to_owned(),
                canonical_command_digest: canonical_command_digest.clone(),
                idempotency_key: idempotency_key.to_owned(),
            },
            nonce_digest: plain_digest(nonce.as_bytes()),
            canonical_command_bytes,
            correlation_id: input.correlation_id.clone(),
            expected_revision,
            ids: AuthorCommandAdmissionIds {
                command_id: Uuid::now_v7().to_string(),
                author_command_admission_id: Uuid::now_v7().to_string(),
                receipt_id: Uuid::now_v7().to_string(),
            },
        },
    )
    .await
    .map_err(archive_project_error)?;
    let project = open_project(&store, &scope)
        .await
        .map_err(service_unavailable)?
        .ok_or_else(resource_unavailable)?;
    archive_project_response(
        &scope,
        &input.correlation_id,
        &digest_hex,
        idempotency_key,
        project,
        settlement,
    )
}

fn archive_project_response(
    scope: &ApplicationScope,
    correlation_id: &str,
    digest_hex: &str,
    idempotency_key: &str,
    project: storyos_application::Project,
    settlement: storyos_application::ArchiveProjectSettlement,
) -> Result<Json<contracts::ArchiveProjectResponse>, ApiError> {
    let (receipt_result, effect) = match settlement.effect {
        ArchiveProjectSettlementEffect::Applied { revision } => (
            contracts::DomainReceiptResult::AuthoritativeApplied,
            contracts::ArchiveProjectEffect::AuthoritativeApplied {
                revision: revision.to_string(),
                project_activity_position: settlement.project_activity_position.to_string(),
            },
        ),
        ArchiveProjectSettlementEffect::NoEffect { reason } => (
            contracts::DomainReceiptResult::NoEffect,
            contracts::ArchiveProjectEffect::NoEffect {
                reason: match reason {
                    storyos_core::ArchiveProjectNoEffect::AlreadyArchived => {
                        contracts::ArchiveProjectNoEffectReason::AlreadyArchived
                    }
                },
            },
        ),
        ArchiveProjectSettlementEffect::Conflicted { reason } => (
            contracts::DomainReceiptResult::Conflicted,
            contracts::ArchiveProjectEffect::Conflicted {
                reason: match reason {
                    storyos_core::ArchiveProjectConflict::StaleProjectRevision => {
                        contracts::ArchiveProjectConflictReason::StaleProjectRevision
                    }
                },
            },
        ),
        ArchiveProjectSettlementEffect::Refused { .. } => {
            return Err(archive_project_error(ArchiveProjectError::BindingConflict));
        }
    };
    let contract_project_scope = contract_scope(scope);
    Ok(Json(contracts::ArchiveProjectResponse {
        schema_id: contracts::ARCHIVE_PROJECT_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: correlation_id.to_owned(),
        project_scope: contract_project_scope.clone(),
        command_id: settlement.ids.command_id,
        author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
        receipt: contracts::DomainReceipt {
            receipt_id: settlement.ids.receipt_id.clone(),
            project_scope: contract_project_scope,
            command_kind: contracts::DomainReceiptCommandKind::ArchiveProject,
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: contracts::ARCHIVE_PROJECT_DIGEST_PROFILE.to_owned(),
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
        effect,
    }))
}

fn canonical_body_bytes(body: &contracts::ArchiveProjectRequest) -> Result<Vec<u8>, ApiError> {
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

fn archive_project_error(error: ArchiveProjectError) -> ApiError {
    match error {
        ArchiveProjectError::BindingConflict => problem(
            StatusCode::CONFLICT,
            "idempotency_binding_conflict",
            "The Archive Project binding conflicts.",
        ),
        ArchiveProjectError::InvalidChallenge => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "challenge_invalid",
            "The Archive Project challenge is invalid.",
        ),
        ArchiveProjectError::MissingProject => resource_unavailable(),
        ArchiveProjectError::Unavailable(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "project_store_unavailable",
            "The Project store is unavailable.",
        ),
    }
}
