use axum::body::to_bytes;
use sha2::{Digest, Sha256};
use storyos_application::{
    AuthorCommandAdmissionIds, EditorClientBinding, EditorSessionId,
    ProjectCommandChallengeBinding, RejectProposalOperationsCommand, RejectProposalOperationsError,
    RejectProposalOperationsSettlementEffect, RejectionNote,
};

use super::editor_session::{exact_header, session_binding_ref};
use super::project_command_challenge::{
    hex_bytes, plain_digest, valid_uuid_v7, validate_json_content_type,
};
use super::*;

pub(super) async fn reject_proposal_operations(
    State(state): State<Arc<ServerState>>,
    Path((project_id, proposal_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::RejectProposalOperationsResponse>, ApiError> {
    let (parts, body_stream) = request.into_parts();
    let headers = parts.headers;
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::StateChanging,
    )?;
    validate_json_content_type(&headers)?;
    valid_uuid(&proposal_id)?;
    let bytes = to_bytes(body_stream, contracts::AUTHOR_EDIT_MAX_WIRE_BODY_BYTES)
        .await
        .map_err(|_| payload_too_large())?;
    let body = serde_json::from_slice::<contracts::RejectProposalOperationsRequest>(&bytes)
        .map_err(|_| invalid_request_shape())?;
    let input = &body.reject_proposal_operations_input;
    let session_handle = session_cookie(&headers).ok_or_else(authentication_required)?;
    let session = state
        .client_session_binding(session_handle)
        .ok_or_else(authentication_required)?;
    if body.command_schema != contracts::REJECT_PROPOSAL_OPERATIONS_REQUEST_SCHEMA_ID
        || input.client_contract_revision != session.client_contract_revision
        || input.security_policy_revision != session.security_policy_revision
        || input.selected_pending_operation_ids.len() != 1
        || input.expected_target_revisions.len() != 1
    {
        return Err(invalid_request());
    }
    let selected_pending_operation_id = input.selected_pending_operation_ids[0].clone();
    let expected_authoritative_revision_id = input.expected_target_revisions[0].clone();
    valid_uuid(&input.correlation_id)?;
    valid_uuid(&input.proposal_revision_id)?;
    valid_uuid(&selected_pending_operation_id)?;
    valid_uuid(&expected_authoritative_revision_id)?;
    valid_uuid(&input.editor_session_id)?;
    let rejection_note = match &input.rejection_reason {
        contracts::ProposalRejectionReason::AuthorDeclined { note } => match note {
            contracts::BoundedAuthorNote::Omitted => RejectionNote::Omitted,
            contracts::BoundedAuthorNote::Present { text } => {
                if text.is_empty() {
                    return Err(invalid_request());
                }
                RejectionNote::Present { text: text.clone() }
            }
        },
    };
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
        contracts::REJECT_PROPOSAL_OPERATIONS_DIGEST_PROFILE
    );
    let store = project_reader(&state).await?;
    let command = RejectProposalOperationsCommand {
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
            method: contracts::REJECT_PROPOSAL_OPERATIONS_METHOD.to_owned(),
            route_template: contracts::REJECT_PROPOSAL_OPERATIONS_PATH.to_owned(),
            command_schema: body.command_schema.clone(),
            command_kind: "rejectProposalOperations".to_owned(),
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
        proposal_id,
        proposal_revision_id: input.proposal_revision_id.clone(),
        selected_pending_operation_id,
        expected_authoritative_revision_id,
        rejection_note,
    };
    let settlement = storyos_application::reject_proposal_operations(&store, &command)
        .await
        .map_err(reject_error)?;
    super::acknowledgement_hold::hold_first_acknowledgement_if_requested(idempotency_key).await;
    reject_response(&command, &digest_hex, settlement)
}

fn reject_response(
    command: &RejectProposalOperationsCommand,
    digest_hex: &str,
    settlement: storyos_application::RejectProposalOperationsSettlement,
) -> Result<Json<contracts::RejectProposalOperationsResponse>, ApiError> {
    let project = settlement.response_project;
    let contract_project_scope = contract_scope(&command.project_scope);
    let (result, effect) = match settlement.effect {
        RejectProposalOperationsSettlementEffect::Resolved {
            author_action_sequence,
            operation_id,
            rejection_note,
            preserved_generation,
            preserved_validation,
            preserved_closure,
            resolution_event_id,
        } => (
            contracts::RejectionReceiptResult::Resolved,
            contracts::RejectProposalOperationsEffect::Resolved {
                author_action_sequence: author_action_sequence.to_string(),
                undo_disposition: contracts::AuthorUndoDisposition::Forward,
                operation_ids: vec![operation_id],
                prior_resolution: "pending".to_owned(),
                resulting_resolution: "rejected".to_owned(),
                rejection_reason: contracts::ProposalRejectionReason::AuthorDeclined {
                    note: match rejection_note {
                        RejectionNote::Omitted => contracts::BoundedAuthorNote::Omitted,
                        RejectionNote::Present { text } => {
                            contracts::BoundedAuthorNote::Present { text }
                        }
                    },
                },
                preserved_generation,
                preserved_validation,
                preserved_closure,
                resolution_event_refs: vec![resolution_event_id],
            },
        ),
        RejectProposalOperationsSettlementEffect::Conflicted { reason } => (
            contracts::RejectionReceiptResult::Conflicted,
            contracts::RejectProposalOperationsEffect::Conflicted {
                reason: match reason {
                    storyos_core::RejectProposalOperationsConflict::ChangedHead => {
                        contracts::RejectProposalOperationsConflictReason::ChangedHead
                    }
                },
            },
        ),
        RejectProposalOperationsSettlementEffect::Refused { reason } => (
            contracts::RejectionReceiptResult::Refused,
            contracts::RejectProposalOperationsEffect::Refused {
                reason: match reason {
                    storyos_core::RejectProposalOperationsRefusal::WrongScope => {
                        contracts::RejectProposalOperationsRefusalReason::WrongScope
                    }
                    storyos_core::RejectProposalOperationsRefusal::WrongAdmission => {
                        contracts::RejectProposalOperationsRefusalReason::WrongAdmission
                    }
                    storyos_core::RejectProposalOperationsRefusal::StaleProposalRevision => {
                        contracts::RejectProposalOperationsRefusalReason::StaleProposalRevision
                    }
                    storyos_core::RejectProposalOperationsRefusal::NotEligible => {
                        contracts::RejectProposalOperationsRefusalReason::NotEligible
                    }
                    storyos_core::RejectProposalOperationsRefusal::OperationNotPending => {
                        contracts::RejectProposalOperationsRefusalReason::OperationNotPending
                    }
                },
            },
        ),
    };
    Ok(Json(contracts::RejectProposalOperationsResponse {
        schema_id: contracts::REJECT_PROPOSAL_OPERATIONS_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: command.correlation_id.clone(),
        project_scope: contract_project_scope.clone(),
        command_id: settlement.ids.command_id,
        author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
        receipt: contracts::RejectionReceipt {
            receipt_id: settlement.ids.receipt_id,
            project_scope: contract_project_scope,
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: contracts::REJECT_PROPOSAL_OPERATIONS_DIGEST_PROFILE.to_owned(),
                value_hex_lowercase: digest_hex.to_owned(),
            },
            idempotency_key: command.challenge_binding.idempotency_key.clone(),
            author_command_admission_id: settlement.ids.author_command_admission_id,
            proposal_id: command.proposal_id.clone(),
            proposal_revision_id: command.proposal_revision_id.clone(),
            selected_pending_operation_ids: vec![command.selected_pending_operation_id.clone()],
            expected_target_revisions: vec![command.expected_authoritative_revision_id.clone()],
            prior_authoritative_revision_ids: vec![
                command.expected_authoritative_revision_id.clone(),
            ],
            resulting_authoritative_revision_ids: vec![
                command.expected_authoritative_revision_id.clone(),
            ],
            authoritative_commit_ids: Vec::new(),
            result,
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
    body: &contracts::RejectProposalOperationsRequest,
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

fn reject_error(error: RejectProposalOperationsError) -> ApiError {
    match error {
        RejectProposalOperationsError::BindingConflict => problem(
            StatusCode::CONFLICT,
            "idempotency_binding_conflict",
            "The Rejection binding conflicts.",
        ),
        RejectProposalOperationsError::HistoricalAcknowledgementUnavailable => problem(
            StatusCode::CONFLICT,
            "historical_acknowledgement_unavailable",
            "The original Rejection acknowledgement cannot be recovered. Refresh to inspect the current Project.",
        ),
        RejectProposalOperationsError::InvalidChallenge => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "challenge_invalid",
            "The Rejection challenge is invalid.",
        ),
        RejectProposalOperationsError::MissingProject => resource_unavailable(),
        RejectProposalOperationsError::Unavailable(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "project_store_unavailable",
            "The Project store is unavailable.",
        ),
    }
}
