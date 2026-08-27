use axum::body::to_bytes;
use sha2::{Digest, Sha256};
use storyos_application::{
    AuthorCommandAdmissionIds, ChapterId, EditorClientBinding, ProjectCommandChallengeBinding,
    UpdateChapterCommand, UpdateChapterError, UpdateChapterSettlementEffect,
};

use super::editor_session::{exact_header, session_binding_ref};
use super::project_command_challenge::{
    hex_bytes, plain_digest, valid_uuid_v7, validate_json_content_type,
};
use super::*;

pub(super) async fn update_chapter(
    State(state): State<Arc<ServerState>>,
    Path((project_id, chapter_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::UpdateChapterResponse>, ApiError> {
    let (parts, body_stream) = request.into_parts();
    let headers = parts.headers;
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::StateChanging,
    )?;
    valid_uuid(&chapter_id)?;
    validate_json_content_type(&headers)?;
    let bytes = to_bytes(body_stream, contracts::AUTHOR_EDIT_MAX_WIRE_BODY_BYTES)
        .await
        .map_err(|_| payload_too_large())?;
    let body = serde_json::from_slice::<contracts::UpdateChapterRequest>(&bytes)
        .map_err(|_| invalid_request_shape())?;
    let input = &body.update_chapter_input;
    let Some(expected_chapter_revision) = input
        .expected_chapter_revision
        .parse::<u64>()
        .ok()
        .filter(|revision| *revision >= 1)
    else {
        return Err(invalid_request());
    };
    let Some(order) = input.order.parse::<u64>().ok().filter(|order| *order >= 1) else {
        return Err(invalid_request());
    };
    let session_handle = session_cookie(&headers).ok_or_else(authentication_required)?;
    let session = state
        .config
        .session_bindings
        .get(session_handle)
        .ok_or_else(authentication_required)?;
    if body.command_schema != contracts::UPDATE_CHAPTER_REQUEST_SCHEMA_ID
        || input.client_contract_revision != session.client_contract_revision
        || input.security_policy_revision != session.security_policy_revision
        || input.title.is_empty()
        || input.title.len() > 1024
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
        contracts::UPDATE_CHAPTER_DIGEST_PROFILE
    );
    let store = project_reader(&state)?;
    let command = UpdateChapterCommand {
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
            method: contracts::UPDATE_CHAPTER_METHOD.to_owned(),
            route_template: contracts::UPDATE_CHAPTER_PATH.to_owned(),
            command_schema: body.command_schema.clone(),
            command_kind: "updateChapter".to_owned(),
            canonical_command_digest: canonical_command_digest.clone(),
            idempotency_key: idempotency_key.to_owned(),
        },
        nonce_digest: plain_digest(nonce.as_bytes()),
        canonical_command_bytes,
        correlation_id: input.correlation_id.clone(),
        chapter_id: ChapterId::new(chapter_id),
        title: input.title.clone(),
        order,
        expected_chapter_revision,
        ids: AuthorCommandAdmissionIds {
            command_id: Uuid::now_v7().to_string(),
            author_command_admission_id: Uuid::now_v7().to_string(),
            receipt_id: Uuid::now_v7().to_string(),
        },
    };
    let settlement = storyos_application::update_chapter(&store, &command)
        .await
        .map_err(update_chapter_error)?;
    let project = open_project(&store, &scope)
        .await
        .map_err(service_unavailable)?
        .ok_or_else(resource_unavailable)?;
    update_chapter_response(&command, &digest_hex, project, settlement)
}

fn update_chapter_response(
    command: &UpdateChapterCommand,
    digest_hex: &str,
    project: storyos_application::Project,
    settlement: storyos_application::UpdateChapterSettlement,
) -> Result<Json<contracts::UpdateChapterResponse>, ApiError> {
    let (receipt_result, effect) = match settlement.effect {
        UpdateChapterSettlementEffect::Applied {
            title,
            order,
            tree_revision,
        } => (
            contracts::DomainReceiptResult::AuthoritativeApplied,
            contracts::UpdateChapterEffect::AuthoritativeApplied {
                chapter_id: command.chapter_id.as_ref().to_owned(),
                title,
                tree_revision: tree_revision.to_string(),
                order: order.to_string(),
                project_activity_position: settlement.project_activity_position.to_string(),
            },
        ),
        UpdateChapterSettlementEffect::NoEffect { reason } => (
            contracts::DomainReceiptResult::NoEffect,
            contracts::UpdateChapterEffect::NoEffect {
                reason: match reason {
                    storyos_core::UpdateChapterNoEffect::Unchanged => {
                        contracts::UpdateChapterNoEffectReason::Unchanged
                    }
                },
            },
        ),
        UpdateChapterSettlementEffect::Conflicted { reason } => (
            contracts::DomainReceiptResult::Conflicted,
            contracts::UpdateChapterEffect::Conflicted {
                reason: match reason {
                    storyos_core::UpdateChapterConflict::StaleChapterRevision => {
                        contracts::UpdateChapterConflictReason::StaleChapterRevision
                    }
                },
            },
        ),
        UpdateChapterSettlementEffect::Refused { reason } => (
            contracts::DomainReceiptResult::Refused,
            contracts::UpdateChapterEffect::Refused {
                reason: match reason {
                    storyos_core::UpdateChapterRefusal::ArchivedProject => {
                        contracts::UpdateChapterRefusalReason::ArchivedProject
                    }
                    storyos_core::UpdateChapterRefusal::InvalidTitle => {
                        contracts::UpdateChapterRefusalReason::InvalidTitle
                    }
                    storyos_core::UpdateChapterRefusal::InvalidOrder => {
                        contracts::UpdateChapterRefusalReason::InvalidOrder
                    }
                    storyos_core::UpdateChapterRefusal::InvalidChapterJoin => {
                        contracts::UpdateChapterRefusalReason::InvalidChapterJoin
                    }
                    storyos_core::UpdateChapterRefusal::MissingProject => {
                        return Err(update_chapter_error(UpdateChapterError::MissingProject));
                    }
                },
            },
        ),
    };
    let contract_project_scope = contract_scope(&command.project_scope);
    Ok(Json(contracts::UpdateChapterResponse {
        schema_id: contracts::UPDATE_CHAPTER_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: command.correlation_id.clone(),
        project_scope: contract_project_scope.clone(),
        command_id: settlement.ids.command_id,
        author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
        receipt: contracts::DomainReceipt {
            receipt_id: settlement.ids.receipt_id.clone(),
            project_scope: contract_project_scope,
            command_kind: contracts::DomainReceiptCommandKind::UpdateChapter,
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: contracts::UPDATE_CHAPTER_DIGEST_PROFILE.to_owned(),
                value_hex_lowercase: digest_hex.to_owned(),
            },
            idempotency_key: command.challenge_binding.idempotency_key.clone(),
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

fn canonical_body_bytes(body: &contracts::UpdateChapterRequest) -> Result<Vec<u8>, ApiError> {
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

fn update_chapter_error(error: UpdateChapterError) -> ApiError {
    match error {
        UpdateChapterError::BindingConflict => problem(
            StatusCode::CONFLICT,
            "idempotency_binding_conflict",
            "The Update Chapter binding conflicts.",
        ),
        UpdateChapterError::InvalidChallenge => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "challenge_invalid",
            "The Update Chapter challenge is invalid.",
        ),
        UpdateChapterError::MissingProject => resource_unavailable(),
        UpdateChapterError::Unavailable(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "project_store_unavailable",
            "The Project store is unavailable.",
        ),
    }
}
