use storyos_application::{EditorSessionId, ReopenWithdrawnProposalInput};
use storyos_core::TransitionOutcome;

use super::command_admission::{
    BodyValidation, ProblemMapping, ProjectCommandRoute, RevisionMismatch, SchemaMismatch,
    TargetValidation, admit, controlled_project,
};
use super::contract_reason::contract_reason;
use super::*;

const REOPEN_WITHDRAWN_PROPOSAL: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "Reopen",
    command_kind: "reopenWithdrawnProposal",
    method: contracts::REOPEN_WITHDRAWN_PROPOSAL_METHOD,
    path: contracts::REOPEN_WITHDRAWN_PROPOSAL_PATH,
    schema_id: contracts::REOPEN_WITHDRAWN_PROPOSAL_REQUEST_SCHEMA_ID,
    digest_profile: contracts::REOPEN_WITHDRAWN_PROPOSAL_DIGEST_PROFILE,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    body_validation: BodyValidation::AfterRevisionCheck,
    target_validation: TargetValidation::AfterContentType,
    problem_mapping: ProblemMapping::Standard,
};

pub(super) async fn reopen_withdrawn_proposal(
    State(state): State<Arc<ServerState>>,
    Path((project_id, proposal_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::ReopenWithdrawnProposalResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[&proposal_id],
        request,
        &REOPEN_WITHDRAWN_PROPOSAL,
        |body: &contracts::ReopenWithdrawnProposalRequest| {
            let input = &body.reopen_withdrawn_proposal_input;
            let [expected_authoritative_revision_id] = input.expected_target_revisions.as_slice()
            else {
                return Err(invalid_request());
            };
            if input.expected_closure != "withdrawn" {
                return Err(invalid_request());
            }
            valid_uuid(&input.proposal_revision_id)?;
            valid_uuid(&input.withdrawal_event_ref)?;
            valid_uuid(expected_authoritative_revision_id)?;
            valid_uuid(&input.editor_session_id)?;
            Ok(ReopenWithdrawnProposalInput {
                editor_session_id: EditorSessionId::new(input.editor_session_id.clone()),
                proposal_id: proposal_id.clone(),
                proposal_revision_id: input.proposal_revision_id.clone(),
                withdrawal_event_id: input.withdrawal_event_ref.clone(),
                expected_authoritative_revision_id: expected_authoritative_revision_id.clone(),
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .reopen_withdrawn_proposal(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| REOPEN_WITHDRAWN_PROPOSAL.problem(error))?;
    admitted.hold_first_acknowledgement().await;
    let input = &admitted.input;
    let (result, resulting_proposal_revision_id, effect) = match settlement.outcome {
        TransitionOutcome::Applied(applied) => (
            contracts::ReopenWithdrawnReceiptResult::Resolved,
            Some(applied.effect.resulting_proposal_revision_id.clone()),
            contracts::ReopenWithdrawnProposalEffect::Resolved {
                author_action_sequence: applied.author_action_sequence.to_string(),
                undo_disposition: contracts::AuthorUndoDisposition::Forward,
                resulting_proposal_revision_id: applied
                    .effect
                    .resulting_proposal_revision_id
                    .clone(),
                prior_closure: "withdrawn".to_owned(),
                resulting_closure: "open".to_owned(),
                resulting_validation: "pending".to_owned(),
                preserved_generation: applied.effect.preserved_generation,
                preserved_operation_resolution: applied.effect.preserved_operation_resolution,
                withdrawal_event_ref: input.withdrawal_event_id.clone(),
                state_event_refs: vec![applied.effect.resulting_proposal_revision_id],
            },
        ),
        TransitionOutcome::NoEffect(reason) => (
            contracts::ReopenWithdrawnReceiptResult::NoEffect,
            None,
            contracts::ReopenWithdrawnProposalEffect::NoEffect {
                reason: contract_reason(&reason)?,
            },
        ),
        TransitionOutcome::Conflicted(reason) => (
            contracts::ReopenWithdrawnReceiptResult::Conflicted,
            None,
            contracts::ReopenWithdrawnProposalEffect::Conflicted {
                reason: contract_reason(&reason)?,
            },
        ),
        TransitionOutcome::Refused(reason) => (
            contracts::ReopenWithdrawnReceiptResult::Refused,
            None,
            contracts::ReopenWithdrawnProposalEffect::Refused {
                reason: contract_reason(&reason)?,
            },
        ),
    };
    let envelope = &admitted.envelope;
    let project_scope = contract_scope(&envelope.project_scope);
    let head = vec![input.expected_authoritative_revision_id.clone()];
    Ok(Json(contracts::ReopenWithdrawnProposalResponse {
        schema_id: contracts::REOPEN_WITHDRAWN_PROPOSAL_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: envelope.correlation_id.clone(),
        project_scope: project_scope.clone(),
        command_id: settlement.ids.command_id,
        author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
        receipt: contracts::ReopenWithdrawnReceipt {
            receipt_id: settlement.ids.receipt_id,
            project_scope,
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: contracts::REOPEN_WITHDRAWN_PROPOSAL_DIGEST_PROFILE.to_owned(),
                value_hex_lowercase: admitted.digest_hex.clone(),
            },
            idempotency_key: envelope.challenge_binding.idempotency_key.clone(),
            author_command_admission_id: settlement.ids.author_command_admission_id,
            proposal_id: input.proposal_id.clone(),
            source_proposal_revision_id: input.proposal_revision_id.clone(),
            resulting_proposal_revision_id,
            withdrawal_event_ref: input.withdrawal_event_id.clone(),
            expected_target_revisions: head.clone(),
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
