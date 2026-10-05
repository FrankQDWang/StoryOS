use storyos_application::{EditorSessionId, RejectProposalOperationsInput, RejectionNote};
use storyos_core::TransitionOutcome;

use super::command_admission::{
    BodyValidation, ProblemMapping, ProjectCommandRoute, RevisionMismatch, SchemaMismatch,
    TargetValidation, admit, controlled_project,
};
use super::contract_reason::contract_reason;
use super::*;

const REJECT_PROPOSAL_OPERATIONS: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "Rejection",
    command_kind: "rejectProposalOperations",
    method: contracts::REJECT_PROPOSAL_OPERATIONS_METHOD,
    path: contracts::REJECT_PROPOSAL_OPERATIONS_PATH,
    schema_id: contracts::REJECT_PROPOSAL_OPERATIONS_REQUEST_SCHEMA_ID,
    digest_profile: contracts::REJECT_PROPOSAL_OPERATIONS_DIGEST_PROFILE,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    body_validation: BodyValidation::AfterRevisionCheck,
    target_validation: TargetValidation::AfterContentType,
    problem_mapping: ProblemMapping::Standard,
};

pub(super) async fn reject_proposal_operations(
    State(state): State<Arc<ServerState>>,
    Path((project_id, proposal_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::RejectProposalOperationsResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[&proposal_id],
        request,
        &REJECT_PROPOSAL_OPERATIONS,
        |body: &contracts::RejectProposalOperationsRequest| {
            let input = &body.reject_proposal_operations_input;
            let [expected_authoritative_revision_id] = input.expected_target_revisions.as_slice()
            else {
                return Err(invalid_request());
            };
            if input.selected_pending_operation_ids.is_empty() {
                return Err(invalid_request());
            }
            // The correlation identity precedes the other identities in the problem order.
            valid_uuid(&input.correlation_id)?;
            valid_uuid(&input.proposal_revision_id)?;
            for operation_id in &input.selected_pending_operation_ids {
                valid_uuid(operation_id)?;
            }
            valid_uuid(expected_authoritative_revision_id)?;
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
            Ok(RejectProposalOperationsInput {
                editor_session_id: EditorSessionId::new(input.editor_session_id.clone()),
                proposal_id: proposal_id.clone(),
                proposal_revision_id: input.proposal_revision_id.clone(),
                selected_pending_operation_ids: input.selected_pending_operation_ids.clone(),
                expected_authoritative_revision_id: expected_authoritative_revision_id.clone(),
                rejection_note,
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .reject_proposal_operations(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| REJECT_PROPOSAL_OPERATIONS.problem(error))?;
    admitted.hold_first_acknowledgement().await;
    let input = &admitted.input;
    let (result, effect) = match settlement.outcome {
        TransitionOutcome::Applied(applied) => (
            contracts::RejectionReceiptResult::Resolved,
            contracts::RejectProposalOperationsEffect::Resolved {
                author_action_sequence: applied.author_action_sequence.to_string(),
                undo_disposition: contracts::AuthorUndoDisposition::Forward,
                operation_ids: input.selected_pending_operation_ids.clone(),
                prior_resolution: "pending".to_owned(),
                resulting_resolution: "rejected".to_owned(),
                rejection_reason: contracts::ProposalRejectionReason::AuthorDeclined {
                    note: match &input.rejection_note {
                        RejectionNote::Omitted => contracts::BoundedAuthorNote::Omitted,
                        RejectionNote::Present { text } => {
                            contracts::BoundedAuthorNote::Present { text: text.clone() }
                        }
                    },
                },
                preserved_generation: applied.effect.preserved_generation,
                preserved_validation: applied.effect.preserved_validation,
                preserved_closure: applied.effect.preserved_closure,
                resolution_event_refs: vec![applied.effect.resolution_event_id],
            },
        ),
        TransitionOutcome::NoEffect(reason) => match reason {},
        TransitionOutcome::Conflicted(reason) => (
            contracts::RejectionReceiptResult::Conflicted,
            contracts::RejectProposalOperationsEffect::Conflicted {
                reason: contract_reason(&reason)?,
            },
        ),
        TransitionOutcome::Refused(reason) => (
            contracts::RejectionReceiptResult::Refused,
            contracts::RejectProposalOperationsEffect::Refused {
                reason: contract_reason(&reason)?,
            },
        ),
    };
    let envelope = &admitted.envelope;
    let project_scope = contract_scope(&envelope.project_scope);
    let head = vec![input.expected_authoritative_revision_id.clone()];
    Ok(Json(contracts::RejectProposalOperationsResponse {
        schema_id: contracts::REJECT_PROPOSAL_OPERATIONS_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: envelope.correlation_id.clone(),
        project_scope: project_scope.clone(),
        command_id: settlement.ids.command_id,
        author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
        receipt: contracts::RejectionReceipt {
            receipt_id: settlement.ids.receipt_id,
            project_scope,
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: contracts::REJECT_PROPOSAL_OPERATIONS_DIGEST_PROFILE.to_owned(),
                value_hex_lowercase: admitted.digest_hex.clone(),
            },
            idempotency_key: envelope.challenge_binding.idempotency_key.clone(),
            author_command_admission_id: settlement.ids.author_command_admission_id,
            proposal_id: input.proposal_id.clone(),
            proposal_revision_id: input.proposal_revision_id.clone(),
            selected_pending_operation_ids: input.selected_pending_operation_ids.clone(),
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
