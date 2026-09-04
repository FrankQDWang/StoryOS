use axum::body::to_bytes;
use sha2::{Digest, Sha256};
use storyos_application::{
    AuthorCommandAdmissionIds, EditorClientBinding, EditorSessionId,
    ProjectCommandChallengeBinding, UndoLatestAuthorActionCommand, UndoLatestAuthorActionError,
    UndoLatestAuthorActionSettlementEffect,
};

use super::editor_session::{exact_header, session_binding_ref};
use super::project_command_challenge::{
    hex_bytes, plain_digest, valid_uuid_v7, validate_json_content_type,
};
use super::*;

pub(super) async fn undo_latest_author_action(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    request: Request,
) -> Result<Json<contracts::UndoLatestAuthorActionResponse>, ApiError> {
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
    let body = serde_json::from_slice::<contracts::UndoLatestAuthorActionRequest>(&bytes)
        .map_err(|_| invalid_request_shape())?;
    let input = &body.undo_latest_author_action_input;
    let session_handle = session_cookie(&headers).ok_or_else(authentication_required)?;
    let session = state
        .client_session_binding(session_handle)
        .ok_or_else(authentication_required)?;
    let Some(expected_frontier) = input
        .expected_author_undo_frontier_sequence
        .parse::<u64>()
        .ok()
        .filter(|sequence| *sequence >= 1)
    else {
        return Err(invalid_request());
    };
    if body.command_schema != contracts::UNDO_LATEST_AUTHOR_ACTION_REQUEST_SCHEMA_ID
        || input.client_contract_revision != session.client_contract_revision
        || input.security_policy_revision != session.security_policy_revision
    {
        return Err(invalid_request());
    }
    valid_uuid(&input.correlation_id)?;
    valid_uuid(&input.expected_authoritative_revision_id)?;
    valid_uuid(&input.editor_session_id)?;
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
        contracts::UNDO_LATEST_AUTHOR_ACTION_DIGEST_PROFILE
    );
    let store = project_reader(&state)?;
    let command = UndoLatestAuthorActionCommand {
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
            method: contracts::UNDO_LATEST_AUTHOR_ACTION_METHOD.to_owned(),
            route_template: contracts::UNDO_LATEST_AUTHOR_ACTION_PATH.to_owned(),
            command_schema: body.command_schema.clone(),
            command_kind: "undoLatestAuthorAction".to_owned(),
            canonical_command_digest: canonical_command_digest.clone(),
            idempotency_key: idempotency_key.to_owned(),
        },
        nonce_digest: plain_digest(nonce.as_bytes()),
        canonical_command_bytes,
        correlation_id: input.correlation_id.clone(),
        ids: AuthorCommandAdmissionIds {
            command_id: Uuid::now_v7().to_string(),
            author_command_admission_id: Uuid::now_v7().to_string(),
            receipt_id: Uuid::now_v7().to_string(),
        },
        editor_session_id: EditorSessionId::new(input.editor_session_id.clone()),
        expected_author_undo_frontier_sequence: expected_frontier,
        expected_authoritative_revision_id: input.expected_authoritative_revision_id.clone(),
    };
    let settlement = storyos_application::undo_latest_author_action(&store, &command)
        .await
        .map_err(undo_error)?;
    let project = open_project(&store, &scope)
        .await
        .map_err(service_unavailable)?
        .ok_or_else(resource_unavailable)?;
    undo_response(&command, &digest_hex, project, settlement)
}

fn undo_response(
    command: &UndoLatestAuthorActionCommand,
    digest_hex: &str,
    project: storyos_application::Project,
    settlement: storyos_application::UndoLatestAuthorActionSettlement,
) -> Result<Json<contracts::UndoLatestAuthorActionResponse>, ApiError> {
    let (receipt_result, effect, revision_ids, commit_ids, resulting_head, action_sequence) =
        match settlement.effect {
            UndoLatestAuthorActionSettlementEffect::Compensated {
                source_sequence,
                author_action_sequence,
                authoritative_commit_id,
                revision_id,
                body,
                blocks,
                author_undo_frontier_sequence,
            } => (
                contracts::DomainReceiptResult::AuthoritativeApplied,
                contracts::UndoLatestAuthorActionEffect::Compensated {
                    source_sequence: source_sequence.to_string(),
                    author_action_sequence: author_action_sequence.to_string(),
                    authoritative_commit_id: authoritative_commit_id.clone(),
                    authoritative_revision: contract_chapter_revision(
                        revision_id.clone(),
                        body,
                        &blocks,
                    ),
                    project_activity_position: settlement.project_activity_position.to_string(),
                    author_undo_frontier_sequence: author_undo_frontier_sequence
                        .map(|sequence| sequence.to_string()),
                },
                vec![revision_id.clone()],
                vec![authoritative_commit_id],
                revision_id,
                Some(author_action_sequence.to_string()),
            ),
            UndoLatestAuthorActionSettlementEffect::Conflicted { reason } => (
                contracts::DomainReceiptResult::Conflicted,
                contracts::UndoLatestAuthorActionEffect::Conflicted {
                    reason: match &reason {
                        storyos_core::UndoLatestAuthorActionConflict::FrontierMismatch {
                            ..
                        } => contracts::UndoLatestAuthorActionConflictReason::FrontierMismatch,
                        storyos_core::UndoLatestAuthorActionConflict::WrongTargetHead => {
                            contracts::UndoLatestAuthorActionConflictReason::WrongTargetHead
                        }
                    },
                    current_author_undo_frontier_sequence: match reason {
                        storyos_core::UndoLatestAuthorActionConflict::FrontierMismatch {
                            current_author_undo_frontier_sequence,
                        } => current_author_undo_frontier_sequence
                            .map(|sequence| sequence.to_string()),
                        storyos_core::UndoLatestAuthorActionConflict::WrongTargetHead => None,
                    },
                },
                Vec::new(),
                Vec::new(),
                command.expected_authoritative_revision_id.clone(),
                None,
            ),
            UndoLatestAuthorActionSettlementEffect::Unavailable { reason } => (
                contracts::DomainReceiptResult::Refused,
                contracts::UndoLatestAuthorActionEffect::Unavailable {
                    reason: match reason {
                        storyos_core::UndoLatestAuthorActionUnavailable::NoFrontier => {
                            contracts::UndoLatestAuthorActionUnavailableReason::NoFrontier
                        }
                        storyos_core::UndoLatestAuthorActionUnavailable::Barrier => {
                            contracts::UndoLatestAuthorActionUnavailableReason::Barrier
                        }
                    },
                },
                Vec::new(),
                Vec::new(),
                command.expected_authoritative_revision_id.clone(),
                None,
            ),
        };
    let contract_project_scope = contract_scope(&command.project_scope);
    let expected = vec![command.expected_authoritative_revision_id.clone()];
    Ok(Json(contracts::UndoLatestAuthorActionResponse {
        schema_id: contracts::UNDO_LATEST_AUTHOR_ACTION_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: command.correlation_id.clone(),
        project_scope: contract_project_scope.clone(),
        command_id: settlement.ids.command_id,
        author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
        receipt: contracts::DomainReceipt {
            receipt_id: settlement.ids.receipt_id.clone(),
            project_scope: contract_project_scope,
            command_kind: contracts::DomainReceiptCommandKind::UndoLatestAuthorAction,
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: contracts::UNDO_LATEST_AUTHOR_ACTION_DIGEST_PROFILE.to_owned(),
                value_hex_lowercase: digest_hex.to_owned(),
            },
            idempotency_key: command.challenge_binding.idempotency_key.clone(),
            producer_cause: contracts::DomainReceiptProducerCause::AuthorCommandAdmission,
            author_command_admission_id: settlement.ids.author_command_admission_id,
            expected_heads: expected.clone(),
            prior_heads: expected.clone(),
            resulting_heads: vec![resulting_head],
            authoritative_revision_ids: revision_ids,
            proposal_revision_ids: Vec::new(),
            authoritative_commit_ids: commit_ids,
            author_action_sequence: action_sequence,
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

fn canonical_body_bytes(
    body: &contracts::UndoLatestAuthorActionRequest,
) -> Result<Vec<u8>, ApiError> {
    let canonical =
        canonical_json(serde_json::to_value(body).map_err(|_| invalid_request_shape())?);
    serde_json::to_vec(&canonical).map_err(|_| invalid_request_shape())
}

fn canonical_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(key, nested)| (key, canonical_json(nested)))
                .collect(),
        ),
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(canonical_json).collect())
        }
        scalar => scalar,
    }
}

fn undo_error(error: UndoLatestAuthorActionError) -> ApiError {
    match error {
        UndoLatestAuthorActionError::BindingConflict => problem(
            StatusCode::CONFLICT,
            "idempotency_binding_conflict",
            "The Undo Latest Author Action binding conflicts.",
        ),
        UndoLatestAuthorActionError::InvalidChallenge => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "challenge_invalid",
            "The Undo Latest Author Action challenge is invalid.",
        ),
        UndoLatestAuthorActionError::MissingProject => resource_unavailable(),
        UndoLatestAuthorActionError::Unavailable(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "project_store_unavailable",
            "The Project store is unavailable.",
        ),
    }
}
