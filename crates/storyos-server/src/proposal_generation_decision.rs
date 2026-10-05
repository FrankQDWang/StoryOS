use axum::body::to_bytes;
use storyos_application::{
    AuthorCommandAdmissionIds, CompleteReadyPartialProposalInput, ContinueProposalGenerationInput,
    EditorClientBinding, EditorSessionId, ProjectCommandChallengeBinding, ProjectCommandSettlement,
};
use storyos_core::TransitionOutcome;

use super::command_admission::{
    Admitted, BodyValidation, ProblemMapping, ProjectCommandRoute, RevisionMismatch,
    SchemaMismatch, TargetValidation, admit, controlled_project,
};
use super::contract_reason::contract_reason;
use super::editor_session::{exact_header, session_binding_ref};
use super::project_command_challenge::{plain_digest, valid_uuid_v7, validate_json_content_type};
use super::*;

const COMPLETE_READY_PARTIAL_PROPOSAL: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "generation decision",
    command_kind: "completeReadyPartialProposal",
    method: contracts::COMPLETE_READY_PARTIAL_PROPOSAL_METHOD,
    path: contracts::COMPLETE_READY_PARTIAL_PROPOSAL_PATH,
    schema_id: contracts::COMPLETE_READY_PARTIAL_PROPOSAL_REQUEST_SCHEMA_ID,
    digest_profile: contracts::COMPLETE_READY_PARTIAL_PROPOSAL_DIGEST_PROFILE,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    body_validation: BodyValidation::AfterChallengeHeaders,
    target_validation: TargetValidation::AfterContentType,
    problem_mapping: ProblemMapping::Standard,
};

const CONTINUE_PROPOSAL_GENERATION: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "generation decision",
    command_kind: "continueProposalGeneration",
    method: contracts::CONTINUE_PROPOSAL_GENERATION_METHOD,
    path: contracts::CONTINUE_PROPOSAL_GENERATION_PATH,
    schema_id: contracts::CONTINUE_PROPOSAL_GENERATION_REQUEST_SCHEMA_ID,
    digest_profile: contracts::CONTINUE_PROPOSAL_GENERATION_DIGEST_PROFILE,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    body_validation: BodyValidation::AfterChallengeHeaders,
    target_validation: TargetValidation::AfterContentType,
    problem_mapping: ProblemMapping::Standard,
};

pub(super) async fn complete_ready_partial_proposal(
    State(state): State<Arc<ServerState>>,
    Path((project_id, proposal_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::CompleteReadyPartialProposalResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[&proposal_id],
        request,
        &COMPLETE_READY_PARTIAL_PROPOSAL,
        |body: &contracts::CompleteReadyPartialProposalRequest| {
            let input = &body.complete_ready_partial_proposal_input;
            let [expected_authoritative_revision_id] = input.expected_target_revisions.as_slice()
            else {
                return Err(invalid_request());
            };
            if input.expected_candidate_digest.len() != 64 {
                return Err(invalid_request());
            }
            valid_uuid(&input.proposal_revision_id)?;
            valid_uuid(&input.generation_id)?;
            valid_uuid(&input.editor_session_id)?;
            valid_uuid(&input.correlation_id)?;
            valid_uuid(expected_authoritative_revision_id)?;
            Ok(CompleteReadyPartialProposalInput {
                editor_session_id: EditorSessionId::new(input.editor_session_id.clone()),
                proposal_id: proposal_id.clone(),
                proposal_revision_id: input.proposal_revision_id.clone(),
                generation_id: input.generation_id.clone(),
                expected_candidate_digest: input.expected_candidate_digest.clone(),
                last_applied_stream_seq: input
                    .last_applied_stream_seq
                    .parse::<u64>()
                    .map_err(|_| invalid_request())?,
                expected_authoritative_revision_id: expected_authoritative_revision_id.clone(),
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .complete_ready_partial_proposal(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| COMPLETE_READY_PARTIAL_PROPOSAL.problem(error))?;
    let input = &admitted.input;
    let receipt = |result| {
        generation_receipt(
            &admitted,
            &COMPLETE_READY_PARTIAL_PROPOSAL,
            &settlement,
            result,
            &input.proposal_id,
            &input.proposal_revision_id,
            &input.expected_authoritative_revision_id,
        )
    };
    let (receipt, effect) = match &settlement.outcome {
        TransitionOutcome::Applied(applied) => (
            receipt(contracts::ProposalGenerationReceiptResult::ProposalGenerationCompleted),
            contracts::CompleteReadyPartialProposalEffect::Completed {
                author_action_sequence: applied.author_action_sequence.to_string(),
                undo_disposition: contracts::ProposalGenerationUndoDisposition::Forward,
                generation_id: applied.effect.generation_id.clone(),
                prior_generation_state: "ready_partial".to_owned(),
                resulting_generation_state: "ready".to_owned(),
                preserved_validation: applied.effect.preserved_validation.clone(),
                preserved_closure: applied.effect.preserved_closure.clone(),
                preserved_operation_resolution: applied
                    .effect
                    .preserved_operation_resolution
                    .clone(),
                generation_event_ref: applied.effect.generation_event_id.clone(),
            },
        ),
        TransitionOutcome::NoEffect(reason) => match *reason {},
        TransitionOutcome::Conflicted(reason) => (
            receipt(contracts::ProposalGenerationReceiptResult::Conflicted),
            contracts::CompleteReadyPartialProposalEffect::Conflicted {
                reason: contract_reason(reason)?,
            },
        ),
        TransitionOutcome::Refused(reason) => (
            receipt(contracts::ProposalGenerationReceiptResult::Refused),
            contracts::CompleteReadyPartialProposalEffect::Refused {
                reason: contract_reason(reason)?,
            },
        ),
    };
    let envelope = &admitted.envelope;
    Ok(Json(contracts::CompleteReadyPartialProposalResponse {
        schema_id: contracts::COMPLETE_READY_PARTIAL_PROPOSAL_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: envelope.correlation_id.clone(),
        project_scope: contract_scope(&envelope.project_scope),
        command_id: settlement.ids.command_id,
        author_command_admission_id: settlement.ids.author_command_admission_id,
        receipt,
        project: controlled_project(settlement.response),
        effect,
    }))
}

pub(super) async fn continue_proposal_generation(
    State(state): State<Arc<ServerState>>,
    Path((project_id, proposal_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::ContinueProposalGenerationResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[&proposal_id],
        request,
        &CONTINUE_PROPOSAL_GENERATION,
        |body: &contracts::ContinueProposalGenerationRequest| {
            let input = &body.continue_proposal_generation_input;
            let [expected_authoritative_revision_id] = input.expected_target_revisions.as_slice()
            else {
                return Err(invalid_request());
            };
            if input.selected_pending_operation_ids.is_empty()
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
            valid_uuid(expected_authoritative_revision_id)?;
            for operation_id in &input.selected_pending_operation_ids {
                valid_uuid(operation_id)?;
            }
            Ok(ContinueProposalGenerationInput {
                editor_session_id: EditorSessionId::new(input.editor_session_id.clone()),
                proposal_id: proposal_id.clone(),
                proposal_revision_id: input.proposal_revision_id.clone(),
                prior_generation_id: input.prior_generation_id.clone(),
                expected_generation_state: input.expected_generation_state.clone(),
                expected_candidate_digest: input.expected_candidate_digest.clone(),
                selected_pending_operation_ids: input.selected_pending_operation_ids.clone(),
                expected_authoritative_revision_id: expected_authoritative_revision_id.clone(),
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .continue_proposal_generation(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| CONTINUE_PROPOSAL_GENERATION.problem(error))?;
    let input = &admitted.input;
    let receipt = |result| {
        generation_receipt(
            &admitted,
            &CONTINUE_PROPOSAL_GENERATION,
            &settlement,
            result,
            &input.proposal_id,
            &input.proposal_revision_id,
            &input.expected_authoritative_revision_id,
        )
    };
    let (receipt, effect) = match &settlement.outcome {
        TransitionOutcome::Applied(applied) => (
            receipt(contracts::ProposalGenerationReceiptResult::ProposalGenerationStarted),
            contracts::ContinueProposalGenerationEffect::Started {
                author_action_sequence: applied.author_action_sequence.to_string(),
                undo_disposition: contracts::ProposalGenerationUndoDisposition::Forward,
                prior_generation_id: applied.effect.prior_generation_id.clone(),
                new_generation_id: applied.effect.new_generation_id.clone(),
                prior_generation_state: applied.effect.prior_generation_state.clone(),
                resulting_generation_state: "generating".to_owned(),
                prior_run_id: applied.effect.prior_run_id.clone(),
                resulting_run_id: applied.effect.resulting_run_id.clone(),
                preserved_validation: applied.effect.preserved_validation.clone(),
                preserved_closure: applied.effect.preserved_closure.clone(),
                preserved_operation_resolution: applied
                    .effect
                    .preserved_operation_resolution
                    .clone(),
                generation_event_ref: applied.effect.generation_event_id.clone(),
            },
        ),
        TransitionOutcome::NoEffect(reason) => match *reason {},
        TransitionOutcome::Conflicted(reason) => (
            receipt(contracts::ProposalGenerationReceiptResult::Conflicted),
            contracts::ContinueProposalGenerationEffect::Conflicted {
                reason: contract_reason(reason)?,
            },
        ),
        TransitionOutcome::Refused(reason) => (
            receipt(contracts::ProposalGenerationReceiptResult::Refused),
            contracts::ContinueProposalGenerationEffect::Refused {
                reason: contract_reason(reason)?,
            },
        ),
    };
    let envelope = &admitted.envelope;
    Ok(Json(contracts::ContinueProposalGenerationResponse {
        schema_id: contracts::CONTINUE_PROPOSAL_GENERATION_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: envelope.correlation_id.clone(),
        project_scope: contract_scope(&envelope.project_scope),
        command_id: settlement.ids.command_id,
        author_command_admission_id: settlement.ids.author_command_admission_id,
        receipt,
        project: controlled_project(settlement.response),
        effect,
    }))
}

/// The Domain Receipt of one generation decision. Its three head arrays show the one head.
fn generation_receipt<I, A, N, C, R>(
    admitted: &Admitted<I>,
    route: &ProjectCommandRoute,
    settlement: &ProjectCommandSettlement<A, N, C, R>,
    result: contracts::ProposalGenerationReceiptResult,
    proposal_id: &str,
    proposal_revision_id: &str,
    head: &str,
) -> contracts::ProposalGenerationReceipt {
    let envelope = &admitted.envelope;
    let head = vec![head.to_owned()];
    contracts::ProposalGenerationReceipt {
        receipt_id: settlement.ids.receipt_id.clone(),
        project_scope: contract_scope(&envelope.project_scope),
        command_digest: contracts::DigestValue {
            algorithm: contracts::DigestAlgorithm::Sha256,
            profile: route.digest_profile.to_owned(),
            value_hex_lowercase: admitted.digest_hex.clone(),
        },
        idempotency_key: envelope.challenge_binding.idempotency_key.clone(),
        author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
        proposal_id: proposal_id.to_owned(),
        proposal_revision_id: proposal_revision_id.to_owned(),
        expected_target_revisions: head.clone(),
        prior_authoritative_revision_ids: head.clone(),
        resulting_authoritative_revision_ids: head,
        authoritative_commit_ids: Vec::new(),
        result,
        created_at: settlement.receipt_created_at.clone(),
    }
}

pub(super) struct PreparedRequest {
    pub(super) bytes: axum::body::Bytes,
    pub(super) scope: storyos_application::ProjectScope,
    pub(super) client_binding: EditorClientBinding,
    pub(super) nonce_digest: String,
    idempotency_key: String,
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn prepare_request(
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
    })
}

pub(super) fn challenge_binding(
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

pub(super) fn fresh_ids() -> AuthorCommandAdmissionIds {
    AuthorCommandAdmissionIds {
        command_id: Uuid::now_v7().to_string(),
        author_command_admission_id: Uuid::now_v7().to_string(),
        receipt_id: Uuid::now_v7().to_string(),
    }
}
