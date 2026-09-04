use axum::body::to_bytes;
use sha2::{Digest, Sha256};
use storyos_application::{
    AuthorCommandAdmissionIds, CreateChapterCommand, CreateChapterError,
    CreateChapterSettlementEffect, EditorClientBinding, ProjectCommandChallengeBinding,
};

use super::editor_session::{exact_header, session_binding_ref};
use super::project_command_challenge::{
    hex_bytes, plain_digest, valid_uuid_v7, validate_json_content_type,
};
use super::*;

pub(super) async fn create_chapter(
    State(state): State<Arc<ServerState>>,
    Path((project_id, volume_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::CreateChapterResponse>, ApiError> {
    let (parts, body_stream) = request.into_parts();
    let headers = parts.headers;
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::StateChanging,
    )?;
    valid_uuid(&volume_id)?;
    validate_json_content_type(&headers)?;
    let bytes = to_bytes(body_stream, contracts::AUTHOR_EDIT_MAX_WIRE_BODY_BYTES)
        .await
        .map_err(|_| payload_too_large())?;
    let body = serde_json::from_slice::<contracts::CreateChapterRequest>(&bytes)
        .map_err(|_| invalid_request_shape())?;
    let input = &body.create_chapter_input;
    let Some(expected_tree_revision) = input
        .expected_tree_revision
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
    if body.command_schema != contracts::CREATE_CHAPTER_REQUEST_SCHEMA_ID
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
        contracts::CREATE_CHAPTER_DIGEST_PROFILE
    );
    let store = project_reader(&state)?;
    let command = CreateChapterCommand {
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
            method: contracts::CREATE_CHAPTER_METHOD.to_owned(),
            route_template: contracts::CREATE_CHAPTER_PATH.to_owned(),
            command_schema: body.command_schema.clone(),
            command_kind: "createChapter".to_owned(),
            canonical_command_digest: canonical_command_digest.clone(),
            idempotency_key: idempotency_key.to_owned(),
        },
        nonce_digest: plain_digest(nonce.as_bytes()),
        canonical_command_bytes,
        correlation_id: input.correlation_id.clone(),
        volume_id: volume_id.clone(),
        title: input.title.clone(),
        expected_tree_revision,
        ids: AuthorCommandAdmissionIds {
            command_id: Uuid::now_v7().to_string(),
            author_command_admission_id: Uuid::now_v7().to_string(),
            receipt_id: Uuid::now_v7().to_string(),
        },
    };
    let settlement = storyos_application::create_chapter(&store, &command)
        .await
        .map_err(create_chapter_error)?;
    let project = open_project(&store, &scope)
        .await
        .map_err(service_unavailable)?
        .ok_or_else(resource_unavailable)?;
    create_chapter_response(&command, &digest_hex, project, settlement)
}

fn create_chapter_response(
    command: &CreateChapterCommand,
    digest_hex: &str,
    project: storyos_application::Project,
    settlement: storyos_application::CreateChapterSettlement,
) -> Result<Json<contracts::CreateChapterResponse>, ApiError> {
    let (receipt_result, effect) = match settlement.effect {
        CreateChapterSettlementEffect::Applied {
            tree_revision,
            chapter_id,
            order,
            ..
        } => {
            let current_chapter_id = project
                .current_chapter_id
                .as_ref()
                .ok_or_else(resource_unavailable)?
                .as_ref()
                .to_owned();
            (
                contracts::DomainReceiptResult::AuthoritativeApplied,
                contracts::CreateChapterEffect::AuthoritativeApplied {
                    volume_id: command.volume_id.clone(),
                    chapter_id,
                    title: command.title.clone(),
                    tree_revision: tree_revision.to_string(),
                    order: order.to_string(),
                    current_chapter_id,
                    project_activity_position: settlement.project_activity_position.to_string(),
                },
            )
        }
        CreateChapterSettlementEffect::Conflicted { reason } => (
            contracts::DomainReceiptResult::Conflicted,
            contracts::CreateChapterEffect::Conflicted {
                reason: match reason {
                    storyos_core::CreateChapterConflict::StaleTreeRevision => {
                        contracts::CreateChapterConflictReason::StaleTreeRevision
                    }
                },
            },
        ),
        CreateChapterSettlementEffect::Refused { reason } => (
            contracts::DomainReceiptResult::Refused,
            contracts::CreateChapterEffect::Refused {
                reason: match reason {
                    storyos_core::CreateChapterRefusal::ArchivedProject => {
                        contracts::CreateChapterRefusalReason::ArchivedProject
                    }
                    storyos_core::CreateChapterRefusal::InvalidTitle => {
                        contracts::CreateChapterRefusalReason::InvalidTitle
                    }
                    storyos_core::CreateChapterRefusal::InvalidVolumeJoin => {
                        contracts::CreateChapterRefusalReason::InvalidVolumeJoin
                    }
                    storyos_core::CreateChapterRefusal::MissingProject => {
                        return Err(create_chapter_error(CreateChapterError::MissingProject));
                    }
                },
            },
        ),
    };
    let contract_project_scope = contract_scope(&command.project_scope);
    Ok(Json(contracts::CreateChapterResponse {
        schema_id: contracts::CREATE_CHAPTER_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: command.correlation_id.clone(),
        project_scope: contract_project_scope.clone(),
        command_id: settlement.ids.command_id,
        author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
        receipt: contracts::DomainReceipt {
            receipt_id: settlement.ids.receipt_id.clone(),
            project_scope: contract_project_scope,
            command_kind: contracts::DomainReceiptCommandKind::CreateChapter,
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: contracts::CREATE_CHAPTER_DIGEST_PROFILE.to_owned(),
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

fn canonical_body_bytes(body: &contracts::CreateChapterRequest) -> Result<Vec<u8>, ApiError> {
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

fn create_chapter_error(error: CreateChapterError) -> ApiError {
    match error {
        CreateChapterError::BindingConflict => problem(
            StatusCode::CONFLICT,
            "idempotency_binding_conflict",
            "The Create Chapter binding conflicts.",
        ),
        CreateChapterError::InvalidChallenge => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "challenge_invalid",
            "The Create Chapter challenge is invalid.",
        ),
        CreateChapterError::MissingProject => resource_unavailable(),
        CreateChapterError::Unavailable(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "project_store_unavailable",
            "The Project store is unavailable.",
        ),
    }
}
