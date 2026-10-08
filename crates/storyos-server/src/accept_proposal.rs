use storyos_application::{
    AcceptProposalInput, AcceptProposalSettlement, AcceptanceRefusalReason, EditorSessionId,
    RefusableCommandError,
};
use storyos_core::TransitionOutcome;

use super::command_admission::{
    Admitted, BodyValidation, ProblemMapping, ProjectCommandRoute, RevisionMismatch,
    SchemaMismatch, TargetValidation, admit, controlled_project,
};
use super::contract_reason::contract_reason;
use super::*;

const ACCEPT_PROPOSAL: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "Acceptance",
    command_kind: "acceptProposal",
    method: contracts::ACCEPT_PROPOSAL_METHOD,
    path: contracts::ACCEPT_PROPOSAL_PATH,
    schema_id: contracts::ACCEPT_PROPOSAL_REQUEST_SCHEMA_ID,
    digest_profile: contracts::ACCEPT_PROPOSAL_DIGEST_PROFILE,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    body_validation: BodyValidation::AfterRevisionCheck,
    target_validation: TargetValidation::AfterContentType,
    problem_mapping: ProblemMapping::Standard,
};

pub(super) async fn accept_proposal(
    State(state): State<Arc<ServerState>>,
    Path((project_id, proposal_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::AcceptProposalResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[&proposal_id],
        request,
        &ACCEPT_PROPOSAL,
        |body: &contracts::AcceptProposalRequest| {
            let input = &body.accept_proposal_input;
            // The correlation identity precedes the other identities in the problem order.
            valid_uuid(&input.correlation_id)?;
            valid_uuid(&input.proposal_revision_id)?;
            valid_uuid(&input.validation_receipt_id)?;
            if input.selected_operation_ids.is_empty() {
                return Err(invalid_request());
            }
            for operation_id in &input.selected_operation_ids {
                valid_uuid(operation_id)?;
            }
            valid_uuid(&input.expected_authoritative_revision_id)?;
            valid_uuid(&input.editor_session_id)?;
            Ok(AcceptProposalInput {
                editor_session_id: EditorSessionId::new(input.editor_session_id.clone()),
                proposal_id: proposal_id.clone(),
                proposal_revision_id: input.proposal_revision_id.clone(),
                validation_receipt_id: input.validation_receipt_id.clone(),
                selected_operation_ids: input.selected_operation_ids.clone(),
                expected_authoritative_revision_id: input
                    .expected_authoritative_revision_id
                    .clone(),
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .accept_proposal(&admitted.envelope, &admitted.input)
        .await
        .map_err(accept_problem)?;
    admitted.hold_first_acknowledgement().await;
    accept_response(&admitted, settlement)
}

fn accept_response(
    admitted: &Admitted<AcceptProposalInput>,
    settlement: AcceptProposalSettlement,
) -> Result<Json<contracts::AcceptProposalResponse>, ApiError> {
    let envelope = &admitted.envelope;
    let input = &admitted.input;
    let expected = vec![input.expected_authoritative_revision_id.clone()];
    let (result, effect, resulting, commits) = match settlement.outcome {
        TransitionOutcome::Applied(applied) => (
            contracts::AcceptanceReceiptResult::Applied,
            contracts::AcceptProposalEffect::Applied {
                author_action_sequence: applied.author_action_sequence.to_string(),
                authoritative_commit_id: applied.ids.authoritative_commit_id.clone(),
                authoritative_revision: contract_chapter_revision(
                    applied.ids.revision_id.clone(),
                    applied.body,
                    &applied.blocks,
                ),
                project_activity_position: applied.project_activity_position.to_string(),
            },
            vec![applied.ids.revision_id],
            vec![applied.ids.authoritative_commit_id],
        ),
        TransitionOutcome::NoEffect(reason) => (
            contracts::AcceptanceReceiptResult::Invalid,
            contracts::AcceptProposalEffect::Invalid {
                reason: contract_reason(&reason)?,
            },
            expected.clone(),
            Vec::new(),
        ),
        TransitionOutcome::Conflicted(reason) => (
            contracts::AcceptanceReceiptResult::Conflicted,
            contracts::AcceptProposalEffect::Conflicted {
                reason: contract_reason(&reason)?,
            },
            expected.clone(),
            Vec::new(),
        ),
        TransitionOutcome::Refused(reason) => (
            contracts::AcceptanceReceiptResult::Refused,
            contracts::AcceptProposalEffect::Refused {
                reason: contract_reason(&reason)?,
            },
            expected.clone(),
            Vec::new(),
        ),
    };
    let project_scope = contract_scope(&envelope.project_scope);
    Ok(Json(contracts::AcceptProposalResponse {
        schema_id: contracts::ACCEPT_PROPOSAL_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: envelope.correlation_id.clone(),
        project_scope: project_scope.clone(),
        command_id: settlement.ids.command_id,
        author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
        receipt: contracts::AcceptanceReceipt {
            receipt_id: settlement.ids.receipt_id,
            project_scope,
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: contracts::ACCEPT_PROPOSAL_DIGEST_PROFILE.to_owned(),
                value_hex_lowercase: admitted.digest_hex.clone(),
            },
            idempotency_key: envelope.challenge_binding.idempotency_key.clone(),
            author_command_admission_id: settlement.ids.author_command_admission_id,
            proposal_id: input.proposal_id.clone(),
            proposal_revision_id: input.proposal_revision_id.clone(),
            validation_receipt_id: input.validation_receipt_id.clone(),
            selected_operation_ids: input.selected_operation_ids.clone(),
            prior_authoritative_revision_ids: expected,
            resulting_authoritative_revision_ids: resulting,
            authoritative_commit_ids: commits,
            condition_refs: settlement.zero_authority_effect.unwrap_or_default(),
            result,
            created_at: settlement.receipt_created_at,
        },
        project: controlled_project(settlement.response),
        effect,
    }))
}

fn accept_problem(error: RefusableCommandError<AcceptanceRefusalReason>) -> ApiError {
    match error {
        RefusableCommandError::RefusedBeforeAdmission(
            AcceptanceRefusalReason::InvalidChallenge,
        ) => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "challenge_invalid",
            "The Acceptance challenge is invalid.",
        ),
        RefusableCommandError::RefusedBeforeAdmission(
            AcceptanceRefusalReason::StaleWriter | AcceptanceRefusalReason::SessionChanged,
        ) => problem(
            StatusCode::CONFLICT,
            "acceptance_session_ineligible",
            "This Acceptance session is no longer eligible. Inspect the Proposal and restore writer access before a new attempt.",
        ),
        RefusableCommandError::Command(error) => ACCEPT_PROPOSAL.problem(error),
    }
}
