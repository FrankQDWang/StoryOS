use axum::body::to_bytes;
use sha2::{Digest, Sha256};
use storyos_application::{
    AuthorCommandAdmissionIds, EditorClientBinding, EditorSessionId,
    ProjectCommandChallengeBinding, ReplanProposalCommand, ReplanProposalError,
    ReplanProposalSettlementEffect,
};

use super::editor_session::{exact_header, session_binding_ref};
use super::project_command_challenge::{
    hex_bytes, plain_digest, valid_uuid_v7, validate_json_content_type,
};
use super::*;

pub(super) async fn replan_proposal(
    State(state): State<Arc<ServerState>>,
    Path((project_id, proposal_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::ReplanProposalResponse>, ApiError> {
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
    let body = serde_json::from_slice::<contracts::ReplanProposalRequest>(&bytes)
        .map_err(|_| invalid_request_shape())?;
    let input = &body.replan_proposal_input;
    let session_handle = session_cookie(&headers).ok_or_else(authentication_required)?;
    let session = state
        .client_session_binding(session_handle)
        .ok_or_else(authentication_required)?;
    if body.command_schema != contracts::REPLAN_PROPOSAL_REQUEST_SCHEMA_ID
        || input.client_contract_revision != session.client_contract_revision
        || input.security_policy_revision != session.security_policy_revision
        || input.expected_current_target_revisions.len() != 1
        || input.replacement_operations.len() != 1
    {
        return Err(invalid_request());
    }
    let expected_authoritative_revision_id = input.expected_current_target_revisions[0].clone();
    let replacement_operation_id = input.replacement_operations[0].clone();
    valid_uuid(&input.correlation_id)?;
    valid_uuid(&input.conflicted_proposal_revision_id)?;
    valid_uuid(&input.expected_current_proposal_head)?;
    valid_uuid(&expected_authoritative_revision_id)?;
    valid_uuid(&replacement_operation_id)?;
    valid_uuid(&input.editor_session_id)?;
    match &input.source_condition {
        contracts::ReplanSourceCondition::ProposalConflict {
            proposal_conflict_ref,
        } => valid_uuid(proposal_conflict_ref)?,
        contracts::ReplanSourceCondition::ProposalRecoveryConflict {
            proposal_recovery_conflict_ref,
        } => valid_uuid(proposal_recovery_conflict_ref)?,
    }
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
        contracts::REPLAN_PROPOSAL_DIGEST_PROFILE
    );
    let store = project_reader(&state).await?;
    let command = ReplanProposalCommand {
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
            method: contracts::REPLAN_PROPOSAL_METHOD.to_owned(),
            route_template: contracts::REPLAN_PROPOSAL_PATH.to_owned(),
            command_schema: body.command_schema.clone(),
            command_kind: "replanProposal".to_owned(),
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
        conflicted_proposal_revision_id: input.conflicted_proposal_revision_id.clone(),
        expected_current_proposal_head: input.expected_current_proposal_head.clone(),
        expected_authoritative_revision_id,
        replacement_operation_id,
        source_condition: input.source_condition.clone(),
    };
    let settlement = storyos_application::replan_proposal(&store, &command)
        .await
        .map_err(replan_error)?;
    super::acknowledgement_hold::hold_first_acknowledgement_if_requested(idempotency_key).await;
    replan_response(&command, &digest_hex, settlement)
}

fn replan_response(
    command: &ReplanProposalCommand,
    digest_hex: &str,
    settlement: storyos_application::ReplanProposalSettlement,
) -> Result<Json<contracts::ReplanProposalResponse>, ApiError> {
    let project = settlement.response_project;
    let contract_project_scope = contract_scope(&command.project_scope);
    let resulting_proposal_revision_id = match &settlement.effect {
        ReplanProposalSettlementEffect::Resolved {
            resulting_proposal_revision_id,
            ..
        } => resulting_proposal_revision_id.clone(),
        ReplanProposalSettlementEffect::Conflicted { .. }
        | ReplanProposalSettlementEffect::Refused { .. } => {
            command.conflicted_proposal_revision_id.clone()
        }
    };
    let (result, effect) = match settlement.effect {
        ReplanProposalSettlementEffect::Resolved {
            author_action_sequence,
            resulting_proposal_revision_id,
            preserved_generation,
            preserved_closure,
            source_condition,
            state_event_id,
        } => (
            contracts::ReplanReceiptResult::Resolved,
            contracts::ReplanProposalEffect::Resolved {
                author_action_sequence: author_action_sequence.to_string(),
                undo_disposition: contracts::AuthorUndoDisposition::Forward,
                resulting_proposal_revision_id,
                resulting_validation: "pending".to_owned(),
                preserved_generation,
                preserved_closure,
                source_condition,
                state_event_refs: vec![state_event_id],
            },
        ),
        ReplanProposalSettlementEffect::Conflicted { reason } => (
            contracts::ReplanReceiptResult::Conflicted,
            contracts::ReplanProposalEffect::Conflicted {
                reason: match reason {
                    storyos_core::ReplanProposalConflict::ChangedHead => {
                        contracts::ReplanProposalConflictReason::ChangedHead
                    }
                },
            },
        ),
        ReplanProposalSettlementEffect::Refused { reason } => (
            contracts::ReplanReceiptResult::Refused,
            contracts::ReplanProposalEffect::Refused {
                reason: match reason {
                    storyos_core::ReplanProposalRefusal::WrongScope => {
                        contracts::ReplanProposalRefusalReason::WrongScope
                    }
                    storyos_core::ReplanProposalRefusal::WrongAdmission => {
                        contracts::ReplanProposalRefusalReason::WrongAdmission
                    }
                    storyos_core::ReplanProposalRefusal::StaleProposalRevision => {
                        contracts::ReplanProposalRefusalReason::StaleProposalRevision
                    }
                    storyos_core::ReplanProposalRefusal::NotEligible => {
                        contracts::ReplanProposalRefusalReason::NotEligible
                    }
                    storyos_core::ReplanProposalRefusal::UnavailableProof => {
                        contracts::ReplanProposalRefusalReason::UnavailableProof
                    }
                },
            },
        ),
    };
    Ok(Json(contracts::ReplanProposalResponse {
        schema_id: contracts::REPLAN_PROPOSAL_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: command.correlation_id.clone(),
        project_scope: contract_project_scope.clone(),
        command_id: settlement.ids.command_id,
        author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
        receipt: contracts::ReplanReceipt {
            receipt_id: settlement.ids.receipt_id,
            project_scope: contract_project_scope,
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: contracts::REPLAN_PROPOSAL_DIGEST_PROFILE.to_owned(),
                value_hex_lowercase: digest_hex.to_owned(),
            },
            idempotency_key: command.challenge_binding.idempotency_key.clone(),
            author_command_admission_id: settlement.ids.author_command_admission_id,
            proposal_id: command.proposal_id.clone(),
            source_proposal_revision_id: command.conflicted_proposal_revision_id.clone(),
            resulting_proposal_revision_id,
            expected_current_target_revisions: vec![
                command.expected_authoritative_revision_id.clone(),
            ],
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

fn canonical_body_bytes(body: &contracts::ReplanProposalRequest) -> Result<Vec<u8>, ApiError> {
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

fn replan_error(error: ReplanProposalError) -> ApiError {
    match error {
        ReplanProposalError::BindingConflict => problem(
            StatusCode::CONFLICT,
            "idempotency_binding_conflict",
            "The Replan binding conflicts.",
        ),
        ReplanProposalError::HistoricalAcknowledgementUnavailable => problem(
            StatusCode::CONFLICT,
            "historical_acknowledgement_unavailable",
            "The original Replan acknowledgement cannot be recovered. Refresh to inspect the current Project.",
        ),
        ReplanProposalError::InvalidChallenge => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "challenge_invalid",
            "The Replan challenge is invalid.",
        ),
        ReplanProposalError::MissingProject => resource_unavailable(),
        ReplanProposalError::Unavailable(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "project_store_unavailable",
            "The Project store is unavailable.",
        ),
    }
}
