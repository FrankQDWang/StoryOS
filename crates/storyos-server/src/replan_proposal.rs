use storyos_application::{EditorSessionId, ReplanProposalInput};
use storyos_core::TransitionOutcome;

use super::command_admission::{
    BodyValidation, ProblemMapping, ProjectCommandRoute, RevisionMismatch, SchemaMismatch,
    TargetValidation, admit, controlled_project,
};
use super::contract_reason::contract_reason;
use super::*;

const REPLAN_PROPOSAL: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "Replan",
    command_kind: "replanProposal",
    method: contracts::REPLAN_PROPOSAL_METHOD,
    path: contracts::REPLAN_PROPOSAL_PATH,
    schema_id: contracts::REPLAN_PROPOSAL_REQUEST_SCHEMA_ID,
    digest_profile: contracts::REPLAN_PROPOSAL_DIGEST_PROFILE,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    body_validation: BodyValidation::AfterRevisionCheck,
    target_validation: TargetValidation::AfterContentType,
    problem_mapping: ProblemMapping::Standard,
};

pub(super) async fn replan_proposal(
    State(state): State<Arc<ServerState>>,
    Path((project_id, proposal_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::ReplanProposalResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[&proposal_id],
        request,
        &REPLAN_PROPOSAL,
        |body: &contracts::ReplanProposalRequest| {
            let input = &body.replan_proposal_input;
            let ([expected_authoritative_revision_id], [replacement_operation_id]) = (
                input.expected_current_target_revisions.as_slice(),
                input.replacement_operations.as_slice(),
            ) else {
                return Err(invalid_request());
            };
            valid_uuid(&input.conflicted_proposal_revision_id)?;
            valid_uuid(&input.expected_current_proposal_head)?;
            valid_uuid(expected_authoritative_revision_id)?;
            valid_uuid(replacement_operation_id)?;
            valid_uuid(&input.editor_session_id)?;
            match &input.source_condition {
                contracts::ReplanSourceCondition::ProposalConflict {
                    proposal_conflict_ref,
                } => valid_uuid(proposal_conflict_ref)?,
                contracts::ReplanSourceCondition::ProposalRecoveryConflict {
                    proposal_recovery_conflict_ref,
                } => valid_uuid(proposal_recovery_conflict_ref)?,
            }
            Ok(ReplanProposalInput {
                editor_session_id: EditorSessionId::new(input.editor_session_id.clone()),
                proposal_id: proposal_id.clone(),
                conflicted_proposal_revision_id: input.conflicted_proposal_revision_id.clone(),
                expected_current_proposal_head: input.expected_current_proposal_head.clone(),
                expected_authoritative_revision_id: expected_authoritative_revision_id.clone(),
                replacement_operation_id: replacement_operation_id.clone(),
                source_condition: input.source_condition.clone(),
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .replan_proposal(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| REPLAN_PROPOSAL.problem(error))?;
    admitted.hold_first_acknowledgement().await;
    let input = &admitted.input;
    let (result, resulting_proposal_revision_id, effect) = match settlement.outcome {
        TransitionOutcome::Applied(applied) => (
            contracts::ReplanReceiptResult::Resolved,
            applied.effect.resulting_proposal_revision_id.clone(),
            contracts::ReplanProposalEffect::Resolved {
                author_action_sequence: applied.author_action_sequence.to_string(),
                undo_disposition: contracts::AuthorUndoDisposition::Forward,
                resulting_proposal_revision_id: applied.effect.resulting_proposal_revision_id,
                resulting_validation: "pending".to_owned(),
                preserved_generation: applied.effect.preserved_generation,
                preserved_closure: applied.effect.preserved_closure,
                source_condition: input.source_condition.clone(),
                state_event_refs: vec![applied.effect.state_event_id],
            },
        ),
        TransitionOutcome::NoEffect(reason) => match reason {},
        TransitionOutcome::Conflicted(reason) => (
            contracts::ReplanReceiptResult::Conflicted,
            input.conflicted_proposal_revision_id.clone(),
            contracts::ReplanProposalEffect::Conflicted {
                reason: contract_reason(&reason)?,
            },
        ),
        TransitionOutcome::Refused(reason) => (
            contracts::ReplanReceiptResult::Refused,
            input.conflicted_proposal_revision_id.clone(),
            contracts::ReplanProposalEffect::Refused {
                reason: contract_reason(&reason)?,
            },
        ),
    };
    let envelope = &admitted.envelope;
    let project_scope = contract_scope(&envelope.project_scope);
    let head = vec![input.expected_authoritative_revision_id.clone()];
    Ok(Json(contracts::ReplanProposalResponse {
        schema_id: contracts::REPLAN_PROPOSAL_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: envelope.correlation_id.clone(),
        project_scope: project_scope.clone(),
        command_id: settlement.ids.command_id,
        author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
        receipt: contracts::ReplanReceipt {
            receipt_id: settlement.ids.receipt_id,
            project_scope,
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: contracts::REPLAN_PROPOSAL_DIGEST_PROFILE.to_owned(),
                value_hex_lowercase: admitted.digest_hex.clone(),
            },
            idempotency_key: envelope.challenge_binding.idempotency_key.clone(),
            author_command_admission_id: settlement.ids.author_command_admission_id,
            proposal_id: input.proposal_id.clone(),
            source_proposal_revision_id: input.conflicted_proposal_revision_id.clone(),
            resulting_proposal_revision_id,
            expected_current_target_revisions: head.clone(),
            prior_authoritative_revision_ids: head.clone(),
            resulting_authoritative_revision_ids: head,
            authoritative_commit_ids: Vec::new(),
            result,
            created_at: settlement.receipt_created_at,
        },
        project: controlled_project(settlement.response),
        effect,
    }))
}
