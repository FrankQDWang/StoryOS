use super::editor_session::{exact_header, session_binding_ref};
use super::project_command_challenge::{plain_digest, valid_uuid_v7, validate_json_content_type};
use super::*;
use axum::body::to_bytes;
use storyos_application::{
    AuthorCommandAdmissionIds, CloseEditorFlowDraftCommand, DraftCloseError, EditorClientBinding,
    ProjectCommandChallengeBinding, ProjectScope,
};

pub(super) async fn close_editor_flow_draft(
    State(state): State<Arc<ServerState>>,
    Path((project_id, draft_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::CloseEditorFlowDraftResponse>, ApiError> {
    let (parts, body) = request.into_parts();
    let headers = parts.headers;
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::StateChanging,
    )?;
    validate_json_content_type(&headers)?;
    valid_uuid(&draft_id)?;
    let bytes = to_bytes(body, contracts::AUTHOR_EDIT_MAX_WIRE_BODY_BYTES)
        .await
        .map_err(|_| payload_too_large())?;
    let request: contracts::CloseEditorFlowDraftRequest =
        serde_json::from_slice(&bytes).map_err(|_| invalid_request_shape())?;
    let input = &request.close_editor_flow_draft_input;
    let handle = session_cookie(&headers).ok_or_else(authentication_required)?;
    let session = state
        .client_session_binding(handle)
        .ok_or_else(authentication_required)?;
    if request.command_schema != contracts::CLOSE_EDITOR_FLOW_DRAFT_REQUEST_SCHEMA_ID
        || input.client_contract_revision != session.client_contract_revision
        || input.security_policy_revision != session.security_policy_revision
        || input.draft_kind != "refused_edit"
        || input.expected_closure != "open"
        || input.close_reason != "abandoned"
        || input.source_draft_payload_digest.len() != 64
        || !input
            .source_draft_payload_digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid_request());
    }
    for value in [
        &input.correlation_id,
        &input.editor_session_id,
        &input.source_current_draft_revision_id,
    ] {
        valid_uuid(value)?;
    }
    let key = exact_header(&headers, "idempotency-key")?;
    let nonce = exact_header(&headers, "x-storyos-anti-forgery")?;
    if !valid_uuid_v7(key)
        || nonce.len() != 64
        || !nonce
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid_request());
    }
    let secret = state
        .config
        .project_command_challenge_secret
        .as_deref()
        .filter(|s| s.len() >= 32)
        .ok_or_else(challenge_store_unavailable)?;
    let binding_ref = session_binding_ref(secret, handle);
    let canonical_command_bytes = storyos_core::canonical_json(
        &serde_json::to_value(&request).map_err(|_| invalid_request_shape())?,
    )
    .into_bytes();
    let digest = storyos_core::hex_sha256(&canonical_command_bytes);
    let command = CloseEditorFlowDraftCommand {
        project_scope: scope.clone(),
        client_binding: EditorClientBinding {
            binding_ref: binding_ref.clone(),
            session_generation: session.session_generation,
            client_contract_revision: session.client_contract_revision.clone(),
            security_policy_revision: session.security_policy_revision.clone(),
        },
        challenge_binding: ProjectCommandChallengeBinding {
            project_scope: scope,
            client_session_binding_digest: binding_ref,
            client_session_generation: session.session_generation,
            client_contract_revision: session.client_contract_revision.clone(),
            security_policy_revision: session.security_policy_revision.clone(),
            limit_profile_revision: contracts::LIMIT_PROFILE_REVISION.to_owned(),
            challenge_rate_policy_revision:
                storyos_application::PROJECT_COMMAND_CHALLENGE_RATE_POLICY_REVISION.to_owned(),
            method: "POST".to_owned(),
            route_template: contracts::CLOSE_EDITOR_FLOW_DRAFT_PATH.to_owned(),
            command_schema: request.command_schema.clone(),
            command_kind: "closeEditorFlowDraft".to_owned(),
            canonical_command_digest: format!(
                "sha256:storyos.command.closeEditorFlowDraft.jcs.v1:{digest}"
            ),
            idempotency_key: key.to_owned(),
        },
        nonce_digest: plain_digest(nonce.as_bytes()),
        canonical_command_bytes,
        ids: AuthorCommandAdmissionIds {
            command_id: Uuid::now_v7().to_string(),
            author_command_admission_id: Uuid::now_v7().to_string(),
            receipt_id: Uuid::now_v7().to_string(),
        },
        draft_id,
        input: request.close_editor_flow_draft_input,
    };
    let store = project_reader(&state).await?;
    let settled = storyos_application::close_editor_flow_draft(&store, &command)
        .await
        .map_err(|error| match error {
            DraftCloseError::MissingDraft => resource_unavailable(),
            DraftCloseError::InvalidChallenge => problem(
                StatusCode::UNPROCESSABLE_ENTITY,
                "challenge_invalid",
                "The Draft Discard challenge is invalid.",
            ),
            DraftCloseError::BindingConflict => problem(
                StatusCode::CONFLICT,
                "draft_binding_conflict",
                "The Draft Discard binding conflicts.",
            ),
            DraftCloseError::Unavailable(_) => problem(
                StatusCode::SERVICE_UNAVAILABLE,
                "project_store_unavailable",
                "The Project store is unavailable.",
            ),
        })?;
    super::acknowledgement_hold::hold_first_acknowledgement_if_requested(key).await;
    close_response(&command, settled)
}

fn close_response(
    command: &CloseEditorFlowDraftCommand,
    settled: storyos_application::DraftCloseSettlement,
) -> Result<Json<contracts::CloseEditorFlowDraftResponse>, ApiError> {
    use contracts::{CloseEditorFlowDraftEffect as Effect, DomainReceiptResult as ResultKind};
    use storyos_core::CloseEditorFlowDraftResult;
    let scope = contract_scope(&command.project_scope);
    let digest = contracts::DigestValue {
        algorithm: contracts::DigestAlgorithm::Sha256,
        profile: contracts::CLOSE_EDITOR_FLOW_DRAFT_DIGEST_PROFILE.to_owned(),
        value_hex_lowercase: storyos_core::hex_sha256(&command.canonical_command_bytes),
    };
    let (result, effect) = match settled.result {
        CloseEditorFlowDraftResult::DraftClosureChanged => (
            ResultKind::DraftClosureChanged,
            Effect::DraftClosureChanged {
                event: Box::new(closed_event(
                    &command.project_scope,
                    &command.draft_id,
                    &settled.draft_revision_id,
                    &settled.payload_digest,
                    &storyos_application::RefusedEditDraftClosure {
                        event_id: settled.event_id.clone().ok_or_else(invalid_request)?,
                        source: settled.ids.clone(),
                        command_digest: command.challenge_binding.canonical_command_digest.clone(),
                        idempotency_key: command.challenge_binding.idempotency_key.clone(),
                        author_action_sequence: settled
                            .author_action_sequence
                            .clone()
                            .ok_or_else(invalid_request)?,
                        created_at: settled.created_at.clone(),
                    },
                )?),
            },
        ),
        CloseEditorFlowDraftResult::Conflicted => (
            ResultKind::Conflicted,
            Effect::Conflicted {
                current_revision_id: settled.draft_revision_id,
                current_digest: settled.payload_digest,
                current_closure: settled.observed_closure,
            },
        ),
        CloseEditorFlowDraftResult::SourceDraftNotOpen => (
            ResultKind::Refused,
            Effect::Refused {
                reason: contracts::DraftCloseRefusal::SourceDraftNotOpen,
                current_closure: settled.observed_closure,
            },
        ),
        CloseEditorFlowDraftResult::SourceUnavailable => (
            ResultKind::Refused,
            Effect::Refused {
                reason: contracts::DraftCloseRefusal::SourceUnavailable,
                current_closure: settled.observed_closure,
            },
        ),
    };
    Ok(Json(contracts::CloseEditorFlowDraftResponse {
        schema_id: contracts::CLOSE_EDITOR_FLOW_DRAFT_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: settled.correlation_id,
        project_scope: scope.clone(),
        command_id: settled.ids.command_id,
        author_command_admission_id: settled.ids.author_command_admission_id.clone(),
        receipt: contracts::DomainReceipt {
            receipt_id: settled.ids.receipt_id,
            project_scope: scope,
            command_kind: contracts::DomainReceiptCommandKind::CloseEditorFlowDraft,
            command_digest: digest,
            idempotency_key: command.challenge_binding.idempotency_key.clone(),
            producer_cause: contracts::DomainReceiptProducerCause::AuthorCommandAdmission,
            author_command_admission_id: settled.ids.author_command_admission_id,
            expected_heads: vec![],
            prior_heads: vec![],
            resulting_heads: vec![],
            authoritative_revision_ids: vec![],
            proposal_revision_ids: vec![],
            authoritative_commit_ids: vec![],
            author_action_sequence: settled.author_action_sequence,
            draft_artifact_refs: vec![command.draft_id.clone()],
            artifact_lifecycle_event_refs: settled.event_id.into_iter().collect(),
            condition_refs: vec![],
            result,
            created_at: settled.created_at,
        },
        effect,
    }))
}

pub(super) fn closed_event(
    scope: &ProjectScope,
    draft_id: &str,
    draft_revision_id: &str,
    payload_digest: &str,
    closed: &storyos_application::RefusedEditDraftClosure,
) -> Result<contracts::EditorFlowDraftClosed, ApiError> {
    let value_hex_lowercase = closed
        .command_digest
        .strip_prefix("sha256:storyos.command.closeEditorFlowDraft.jcs.v1:")
        .filter(|digest| {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
        .ok_or_else(challenge_store_unavailable)?
        .to_owned();
    Ok(contracts::EditorFlowDraftClosed {
        schema_id: contracts::EDITOR_FLOW_DRAFT_CLOSED_SCHEMA_ID.to_owned(),
        event_kind: "editor_flow_draft_closed".to_owned(),
        event_id: closed.event_id.clone(),
        project_scope: contract_scope(scope),
        draft_id: draft_id.to_owned(),
        draft_revision_id: draft_revision_id.to_owned(),
        payload_digest: payload_digest.to_owned(),
        prior_closure: "open".to_owned(),
        closure: "closed".to_owned(),
        close_reason: "abandoned".to_owned(),
        source: contracts::RefusedEditDraftSource {
            command_id: closed.source.command_id.clone(),
            author_command_admission_id: closed.source.author_command_admission_id.clone(),
            receipt_id: closed.source.receipt_id.clone(),
            idempotency_key: closed.idempotency_key.clone(),
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: contracts::CLOSE_EDITOR_FLOW_DRAFT_DIGEST_PROFILE.to_owned(),
                value_hex_lowercase,
            },
        },
        author_action_sequence: closed.author_action_sequence.clone(),
        created_at: closed.created_at.clone(),
    })
}
