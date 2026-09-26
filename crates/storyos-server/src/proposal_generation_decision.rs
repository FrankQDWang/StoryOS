use axum::body::to_bytes;
use sha2::{Digest, Sha256};
use storyos_application::{
    AuthorCommandAdmissionIds, CompleteReadyPartialProposalCommand,
    CompleteReadyPartialProposalEffect, ContinueProposalGenerationCommand,
    ContinueProposalGenerationEffect, EditorClientBinding, EditorSessionId,
    ProjectCommandChallengeBinding, ProposalGenerationDecisionError,
};

use super::editor_session::{exact_header, session_binding_ref};
use super::project_command_challenge::{
    hex_bytes, plain_digest, valid_uuid_v7, validate_json_content_type,
};
use super::*;

pub(super) async fn complete_ready_partial_proposal(
    State(state): State<Arc<ServerState>>,
    Path((project_id, proposal_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::CompleteReadyPartialProposalResponse>, ApiError> {
    let prepared = prepare_request(
        &state,
        &project_id,
        &proposal_id,
        request,
        contracts::COMPLETE_READY_PARTIAL_PROPOSAL_REQUEST_SCHEMA_ID,
        contracts::COMPLETE_READY_PARTIAL_PROPOSAL_DIGEST_PROFILE,
        contracts::COMPLETE_READY_PARTIAL_PROPOSAL_METHOD,
        contracts::COMPLETE_READY_PARTIAL_PROPOSAL_PATH,
        "completeReadyPartialProposal",
    )
    .await?;
    let body =
        serde_json::from_slice::<contracts::CompleteReadyPartialProposalRequest>(&prepared.bytes)
            .map_err(|_| invalid_request_shape())?;
    let input = &body.complete_ready_partial_proposal_input;
    if body.command_schema != contracts::COMPLETE_READY_PARTIAL_PROPOSAL_REQUEST_SCHEMA_ID
        || input.client_contract_revision != prepared.client_binding.client_contract_revision
        || input.security_policy_revision != prepared.client_binding.security_policy_revision
        || input.expected_target_revisions.len() != 1
        || input.expected_candidate_digest.len() != 64
    {
        return Err(invalid_request());
    }
    valid_uuid(&input.proposal_revision_id)?;
    valid_uuid(&input.generation_id)?;
    valid_uuid(&input.editor_session_id)?;
    valid_uuid(&input.correlation_id)?;
    valid_uuid(&input.expected_target_revisions[0])?;
    let last_applied_stream_seq = input
        .last_applied_stream_seq
        .parse::<u64>()
        .map_err(|_| invalid_request())?;
    let canonical_command_bytes = canonical_body_bytes(&body)?;
    let digest_hex = hex_bytes(&Sha256::digest(&canonical_command_bytes));
    let mut challenge = challenge_binding(
        &prepared,
        &body.command_schema,
        "completeReadyPartialProposal",
        contracts::COMPLETE_READY_PARTIAL_PROPOSAL_METHOD,
        contracts::COMPLETE_READY_PARTIAL_PROPOSAL_PATH,
    );
    challenge.canonical_command_digest = format!(
        "sha256:{}:{digest_hex}",
        contracts::COMPLETE_READY_PARTIAL_PROPOSAL_DIGEST_PROFILE
    );
    let command = CompleteReadyPartialProposalCommand {
        project_scope: prepared.scope.clone(),
        client_binding: prepared.client_binding.clone(),
        challenge_binding: challenge,
        nonce_digest: prepared.nonce_digest.clone(),
        canonical_command_bytes,
        correlation_id: input.correlation_id.clone(),
        ids: fresh_ids(),
        editor_session_id: EditorSessionId::new(input.editor_session_id.clone()),
        proposal_id: prepared.proposal_id.clone(),
        proposal_revision_id: input.proposal_revision_id.clone(),
        generation_id: input.generation_id.clone(),
        expected_candidate_digest: input.expected_candidate_digest.clone(),
        last_applied_stream_seq,
        expected_authoritative_revision_id: input.expected_target_revisions[0].clone(),
    };
    let store = project_reader(&state).await?;
    let settlement = storyos_application::complete_ready_partial_proposal(&store, &command)
        .await
        .map_err(decision_error)?;
    Ok(Json(complete_response(&command, &digest_hex, settlement)))
}

pub(super) async fn continue_proposal_generation(
    State(state): State<Arc<ServerState>>,
    Path((project_id, proposal_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::ContinueProposalGenerationResponse>, ApiError> {
    let prepared = prepare_request(
        &state,
        &project_id,
        &proposal_id,
        request,
        contracts::CONTINUE_PROPOSAL_GENERATION_REQUEST_SCHEMA_ID,
        contracts::CONTINUE_PROPOSAL_GENERATION_DIGEST_PROFILE,
        contracts::CONTINUE_PROPOSAL_GENERATION_METHOD,
        contracts::CONTINUE_PROPOSAL_GENERATION_PATH,
        "continueProposalGeneration",
    )
    .await?;
    let body =
        serde_json::from_slice::<contracts::ContinueProposalGenerationRequest>(&prepared.bytes)
            .map_err(|_| invalid_request_shape())?;
    let input = &body.continue_proposal_generation_input;
    if body.command_schema != contracts::CONTINUE_PROPOSAL_GENERATION_REQUEST_SCHEMA_ID
        || input.client_contract_revision != prepared.client_binding.client_contract_revision
        || input.security_policy_revision != prepared.client_binding.security_policy_revision
        || input.expected_target_revisions.len() != 1
        || input.selected_pending_operation_ids.is_empty()
        || input.expected_candidate_digest.len() != 64
        || !matches!(
            input.expected_generation_state.as_str(),
            "ready_partial" | "ready"
        )
    {
        return Err(invalid_request());
    }
    valid_uuid(&input.proposal_revision_id)?;
    valid_uuid(&input.prior_generation_id)?;
    valid_uuid(&input.editor_session_id)?;
    valid_uuid(&input.correlation_id)?;
    valid_uuid(&input.expected_target_revisions[0])?;
    for operation_id in &input.selected_pending_operation_ids {
        valid_uuid(operation_id)?;
    }
    let canonical_command_bytes = canonical_body_bytes(&body)?;
    let digest_hex = hex_bytes(&Sha256::digest(&canonical_command_bytes));
    let mut challenge = challenge_binding(
        &prepared,
        &body.command_schema,
        "continueProposalGeneration",
        contracts::CONTINUE_PROPOSAL_GENERATION_METHOD,
        contracts::CONTINUE_PROPOSAL_GENERATION_PATH,
    );
    challenge.canonical_command_digest = format!(
        "sha256:{}:{digest_hex}",
        contracts::CONTINUE_PROPOSAL_GENERATION_DIGEST_PROFILE
    );
    let command = ContinueProposalGenerationCommand {
        project_scope: prepared.scope.clone(),
        client_binding: prepared.client_binding.clone(),
        challenge_binding: challenge,
        nonce_digest: prepared.nonce_digest.clone(),
        canonical_command_bytes,
        correlation_id: input.correlation_id.clone(),
        ids: fresh_ids(),
        editor_session_id: EditorSessionId::new(input.editor_session_id.clone()),
        proposal_id: prepared.proposal_id.clone(),
        proposal_revision_id: input.proposal_revision_id.clone(),
        prior_generation_id: input.prior_generation_id.clone(),
        expected_generation_state: input.expected_generation_state.clone(),
        expected_candidate_digest: input.expected_candidate_digest.clone(),
        selected_pending_operation_ids: input.selected_pending_operation_ids.clone(),
        expected_authoritative_revision_id: input.expected_target_revisions[0].clone(),
    };
    let store = project_reader(&state).await?;
    let settlement = storyos_application::continue_proposal_generation(&store, &command)
        .await
        .map_err(decision_error)?;
    Ok(Json(continue_response(&command, &digest_hex, settlement)))
}

struct PreparedRequest {
    bytes: axum::body::Bytes,
    scope: storyos_application::ProjectScope,
    client_binding: EditorClientBinding,
    nonce_digest: String,
    idempotency_key: String,
    proposal_id: String,
}

#[allow(clippy::too_many_arguments)]
async fn prepare_request(
    state: &ServerState,
    project_id: &str,
    proposal_id: &str,
    request: Request,
    _schema: &str,
    _profile: &str,
    _method: &str,
    _path: &str,
    _kind: &str,
) -> Result<PreparedRequest, ApiError> {
    let (parts, body_stream) = request.into_parts();
    let headers = parts.headers;
    let scope = authenticate_scope(
        state,
        &headers,
        project_id,
        RequestOriginPolicy::StateChanging,
    )?;
    validate_json_content_type(&headers)?;
    valid_uuid(proposal_id)?;
    let bytes = to_bytes(body_stream, contracts::AUTHOR_EDIT_MAX_WIRE_BODY_BYTES)
        .await
        .map_err(|_| payload_too_large())?;
    let session_handle = session_cookie(&headers).ok_or_else(authentication_required)?;
    let session = state
        .client_session_binding(session_handle)
        .ok_or_else(authentication_required)?;
    let idempotency_key = exact_header(&headers, "idempotency-key")?.to_owned();
    let nonce = exact_header(&headers, "x-storyos-anti-forgery")?;
    if !valid_uuid_v7(&idempotency_key)
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
    Ok(PreparedRequest {
        bytes,
        scope,
        client_binding: EditorClientBinding {
            binding_ref,
            session_generation: session.session_generation,
            client_contract_revision: session.client_contract_revision.clone(),
            security_policy_revision: session.security_policy_revision.clone(),
        },
        nonce_digest: plain_digest(nonce.as_bytes()),
        idempotency_key,
        proposal_id: proposal_id.to_owned(),
    })
}

fn challenge_binding(
    prepared: &PreparedRequest,
    schema: &str,
    kind: &str,
    method: &str,
    path: &str,
) -> ProjectCommandChallengeBinding {
    ProjectCommandChallengeBinding {
        project_scope: prepared.scope.clone(),
        client_session_binding_digest: prepared.client_binding.binding_ref.clone(),
        client_session_generation: prepared.client_binding.session_generation,
        client_contract_revision: prepared.client_binding.client_contract_revision.clone(),
        security_policy_revision: prepared.client_binding.security_policy_revision.clone(),
        limit_profile_revision: contracts::LIMIT_PROFILE_REVISION.to_owned(),
        challenge_rate_policy_revision:
            storyos_application::PROJECT_COMMAND_CHALLENGE_RATE_POLICY_REVISION.to_owned(),
        method: method.to_owned(),
        route_template: path.to_owned(),
        command_schema: schema.to_owned(),
        command_kind: kind.to_owned(),
        canonical_command_digest: String::new(),
        idempotency_key: prepared.idempotency_key.clone(),
    }
}

fn fresh_ids() -> AuthorCommandAdmissionIds {
    AuthorCommandAdmissionIds {
        command_id: Uuid::now_v7().to_string(),
        author_command_admission_id: Uuid::now_v7().to_string(),
        receipt_id: Uuid::now_v7().to_string(),
    }
}

fn complete_response(
    command: &CompleteReadyPartialProposalCommand,
    digest_hex: &str,
    settlement: storyos_application::ProposalGenerationSettlement<
        CompleteReadyPartialProposalEffect,
    >,
) -> contracts::CompleteReadyPartialProposalResponse {
    let (result, effect) = match settlement.effect {
        CompleteReadyPartialProposalEffect::Completed {
            author_action_sequence,
            generation_id,
            preserved_validation,
            preserved_closure,
            preserved_operation_resolution,
            generation_event_id,
        } => (
            contracts::ProposalGenerationReceiptResult::ProposalGenerationCompleted,
            contracts::CompleteReadyPartialProposalEffect::Completed {
                author_action_sequence: author_action_sequence.to_string(),
                undo_disposition: contracts::ProposalGenerationUndoDisposition::Forward,
                generation_id,
                prior_generation_state: "ready_partial".to_owned(),
                resulting_generation_state: "ready".to_owned(),
                preserved_validation,
                preserved_closure,
                preserved_operation_resolution,
                generation_event_ref: generation_event_id,
            },
        ),
        CompleteReadyPartialProposalEffect::Conflicted { .. } => (
            contracts::ProposalGenerationReceiptResult::Conflicted,
            contracts::CompleteReadyPartialProposalEffect::Conflicted {
                reason: contracts::ProposalGenerationConflictReason::ChangedHead,
            },
        ),
        CompleteReadyPartialProposalEffect::Refused { reason } => (
            contracts::ProposalGenerationReceiptResult::Refused,
            contracts::CompleteReadyPartialProposalEffect::Refused {
                reason: complete_reason(reason),
            },
        ),
    };
    contracts::CompleteReadyPartialProposalResponse {
        schema_id: contracts::COMPLETE_READY_PARTIAL_PROPOSAL_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: command.correlation_id.clone(),
        project_scope: contract_scope(&command.project_scope),
        command_id: settlement.ids.command_id.clone(),
        author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
        receipt: receipt(
            command.project_scope.clone(),
            &settlement.ids,
            &command.challenge_binding.idempotency_key,
            &command.proposal_id,
            &command.proposal_revision_id,
            &command.expected_authoritative_revision_id,
            contracts::COMPLETE_READY_PARTIAL_PROPOSAL_DIGEST_PROFILE,
            digest_hex,
            result,
            &settlement.receipt_created_at,
        ),
        project: project_body(&settlement.response_project),
        effect,
    }
}

fn continue_response(
    command: &ContinueProposalGenerationCommand,
    digest_hex: &str,
    settlement: storyos_application::ProposalGenerationSettlement<ContinueProposalGenerationEffect>,
) -> contracts::ContinueProposalGenerationResponse {
    let (result, effect) = match settlement.effect {
        ContinueProposalGenerationEffect::Started {
            author_action_sequence,
            prior_generation_id,
            new_generation_id,
            prior_generation_state,
            prior_run_id,
            resulting_run_id,
            preserved_validation,
            preserved_closure,
            preserved_operation_resolution,
            generation_event_id,
        } => (
            contracts::ProposalGenerationReceiptResult::ProposalGenerationStarted,
            contracts::ContinueProposalGenerationEffect::Started {
                author_action_sequence: author_action_sequence.to_string(),
                undo_disposition: contracts::ProposalGenerationUndoDisposition::Forward,
                prior_generation_id,
                new_generation_id,
                prior_generation_state,
                resulting_generation_state: "generating".to_owned(),
                prior_run_id,
                resulting_run_id,
                preserved_validation,
                preserved_closure,
                preserved_operation_resolution,
                generation_event_ref: generation_event_id,
            },
        ),
        ContinueProposalGenerationEffect::Conflicted { .. } => (
            contracts::ProposalGenerationReceiptResult::Conflicted,
            contracts::ContinueProposalGenerationEffect::Conflicted {
                reason: contracts::ProposalGenerationConflictReason::ChangedHead,
            },
        ),
        ContinueProposalGenerationEffect::Refused { reason } => (
            contracts::ProposalGenerationReceiptResult::Refused,
            contracts::ContinueProposalGenerationEffect::Refused {
                reason: continue_reason(reason),
            },
        ),
    };
    contracts::ContinueProposalGenerationResponse {
        schema_id: contracts::CONTINUE_PROPOSAL_GENERATION_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: command.correlation_id.clone(),
        project_scope: contract_scope(&command.project_scope),
        command_id: settlement.ids.command_id.clone(),
        author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
        receipt: receipt(
            command.project_scope.clone(),
            &settlement.ids,
            &command.challenge_binding.idempotency_key,
            &command.proposal_id,
            &command.proposal_revision_id,
            &command.expected_authoritative_revision_id,
            contracts::CONTINUE_PROPOSAL_GENERATION_DIGEST_PROFILE,
            digest_hex,
            result,
            &settlement.receipt_created_at,
        ),
        project: project_body(&settlement.response_project),
        effect,
    }
}

#[allow(clippy::too_many_arguments)]
fn receipt(
    scope: storyos_application::ProjectScope,
    ids: &AuthorCommandAdmissionIds,
    idempotency_key: &str,
    proposal_id: &str,
    proposal_revision_id: &str,
    head: &str,
    profile: &str,
    digest_hex: &str,
    result: contracts::ProposalGenerationReceiptResult,
    created_at: &str,
) -> contracts::ProposalGenerationReceipt {
    let project_scope = contract_scope(&scope);
    contracts::ProposalGenerationReceipt {
        receipt_id: ids.receipt_id.clone(),
        project_scope: project_scope.clone(),
        command_digest: contracts::DigestValue {
            algorithm: contracts::DigestAlgorithm::Sha256,
            profile: profile.to_owned(),
            value_hex_lowercase: digest_hex.to_owned(),
        },
        idempotency_key: idempotency_key.to_owned(),
        author_command_admission_id: ids.author_command_admission_id.clone(),
        proposal_id: proposal_id.to_owned(),
        proposal_revision_id: proposal_revision_id.to_owned(),
        expected_target_revisions: vec![head.to_owned()],
        prior_authoritative_revision_ids: vec![head.to_owned()],
        resulting_authoritative_revision_ids: vec![head.to_owned()],
        authoritative_commit_ids: Vec::new(),
        result,
        created_at: created_at.to_owned(),
    }
}

fn project_body(project: &storyos_application::Project) -> contracts::ControlledProject {
    contracts::ControlledProject {
        project_id: project.project_id.as_ref().to_owned(),
        title: project.title.clone(),
        open: match &project.current_chapter_id {
            Some(chapter_id) => contracts::ProjectOpenState::CurrentChapter {
                current_chapter_id: chapter_id.as_ref().to_owned(),
            },
            None => contracts::ProjectOpenState::Empty,
        },
    }
}

fn canonical_body_bytes<T: serde::Serialize>(body: &T) -> Result<Vec<u8>, ApiError> {
    let canonical =
        canonical_json(serde_json::to_value(body).map_err(|_| invalid_request_shape())?);
    let bytes = serde_json::to_vec(&canonical).map_err(|_| invalid_request_shape())?;
    Ok(bytes)
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

fn complete_reason(
    reason: storyos_core::CompleteReadyPartialProposalRefusal,
) -> contracts::CompleteReadyPartialProposalRefusalReason {
    match reason {
        storyos_core::CompleteReadyPartialProposalRefusal::StaleProposalRevision => {
            contracts::CompleteReadyPartialProposalRefusalReason::StaleProposalRevision
        }
        storyos_core::CompleteReadyPartialProposalRefusal::NotEligible => {
            contracts::CompleteReadyPartialProposalRefusalReason::NotEligible
        }
        storyos_core::CompleteReadyPartialProposalRefusal::NotReadyPartial => {
            contracts::CompleteReadyPartialProposalRefusalReason::NotReadyPartial
        }
        storyos_core::CompleteReadyPartialProposalRefusal::StaleGeneration => {
            contracts::CompleteReadyPartialProposalRefusalReason::StaleGeneration
        }
        storyos_core::CompleteReadyPartialProposalRefusal::StaleCandidate => {
            contracts::CompleteReadyPartialProposalRefusalReason::StaleCandidate
        }
    }
}

fn continue_reason(
    reason: storyos_core::ContinueProposalGenerationRefusal,
) -> contracts::ContinueProposalGenerationRefusalReason {
    match reason {
        storyos_core::ContinueProposalGenerationRefusal::StaleProposalRevision => {
            contracts::ContinueProposalGenerationRefusalReason::StaleProposalRevision
        }
        storyos_core::ContinueProposalGenerationRefusal::NotEligible => {
            contracts::ContinueProposalGenerationRefusalReason::NotEligible
        }
        storyos_core::ContinueProposalGenerationRefusal::NotContinuable => {
            contracts::ContinueProposalGenerationRefusalReason::NotContinuable
        }
        storyos_core::ContinueProposalGenerationRefusal::StaleGeneration => {
            contracts::ContinueProposalGenerationRefusalReason::StaleGeneration
        }
        storyos_core::ContinueProposalGenerationRefusal::StaleCandidate => {
            contracts::ContinueProposalGenerationRefusalReason::StaleCandidate
        }
        storyos_core::ContinueProposalGenerationRefusal::OperationNotPending => {
            contracts::ContinueProposalGenerationRefusalReason::OperationNotPending
        }
        storyos_core::ContinueProposalGenerationRefusal::DuplicateIdentities => {
            contracts::ContinueProposalGenerationRefusalReason::DuplicateIdentities
        }
    }
}

fn decision_error(error: ProposalGenerationDecisionError) -> ApiError {
    match error {
        ProposalGenerationDecisionError::BindingConflict => problem(
            StatusCode::CONFLICT,
            "idempotency_binding_conflict",
            "The generation decision binding conflicts.",
        ),
        ProposalGenerationDecisionError::HistoricalAcknowledgementUnavailable => problem(
            StatusCode::CONFLICT,
            "historical_acknowledgement_unavailable",
            "The original generation decision acknowledgement cannot be recovered. Refresh to inspect the current Project.",
        ),
        ProposalGenerationDecisionError::InvalidChallenge => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "challenge_invalid",
            "The generation decision challenge is invalid.",
        ),
        ProposalGenerationDecisionError::MissingProject => resource_unavailable(),
        ProposalGenerationDecisionError::Unavailable(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "project_store_unavailable",
            "The Project store is unavailable.",
        ),
    }
}
