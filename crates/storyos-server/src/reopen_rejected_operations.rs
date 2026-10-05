use storyos_application::{EditorSessionId, ReopenRejectedOperationsInput};
use storyos_core::TransitionOutcome;

use super::command_admission::{
    BodyValidation, ProblemMapping, ProjectCommandRoute, RevisionMismatch, SchemaMismatch,
    TargetValidation, admit, controlled_project,
};
use super::contract_reason::contract_reason;
use super::*;

const REOPEN_REJECTED_OPERATIONS: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "Reopen",
    command_kind: "reopenRejectedOperations",
    method: contracts::REOPEN_REJECTED_OPERATIONS_METHOD,
    path: contracts::REOPEN_REJECTED_OPERATIONS_PATH,
    schema_id: contracts::REOPEN_REJECTED_OPERATIONS_REQUEST_SCHEMA_ID,
    digest_profile: contracts::REOPEN_REJECTED_OPERATIONS_DIGEST_PROFILE,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    body_validation: BodyValidation::AfterRevisionCheck,
    target_validation: TargetValidation::AfterContentType,
    problem_mapping: ProblemMapping::Standard,
};

pub(super) async fn reopen_rejected_operations(
    State(state): State<Arc<ServerState>>,
    Path((project_id, proposal_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::ReopenRejectedOperationsResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[&proposal_id],
        request,
        &REOPEN_REJECTED_OPERATIONS,
        |body: &contracts::ReopenRejectedOperationsRequest| {
            let input = &body.reopen_rejected_operations_input;
            let (
                [selected_rejected_operation_id],
                [rejection_event_id],
                [expected_authoritative_revision_id],
            ) = (
                input.selected_rejected_operation_ids.as_slice(),
                input.rejection_event_refs.as_slice(),
                input.expected_target_revisions.as_slice(),
            )
            else {
                return Err(invalid_request());
            };
            valid_uuid(&input.proposal_revision_id)?;
            valid_uuid(selected_rejected_operation_id)?;
            valid_uuid(rejection_event_id)?;
            valid_uuid(expected_authoritative_revision_id)?;
            valid_uuid(&input.editor_session_id)?;
            Ok(ReopenRejectedOperationsInput {
                editor_session_id: EditorSessionId::new(input.editor_session_id.clone()),
                proposal_id: proposal_id.clone(),
                proposal_revision_id: input.proposal_revision_id.clone(),
                selected_rejected_operation_id: selected_rejected_operation_id.clone(),
                rejection_event_id: rejection_event_id.clone(),
                expected_authoritative_revision_id: expected_authoritative_revision_id.clone(),
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .reopen_rejected_operations(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| REOPEN_REJECTED_OPERATIONS.problem(error))?;
    admitted.hold_first_acknowledgement().await;
    let input = &admitted.input;
    let (result, resulting_proposal_revision_id, effect) = match settlement.outcome {
        TransitionOutcome::Applied(applied) => (
            contracts::ReopenReceiptResult::Resolved,
            applied.effect.resulting_proposal_revision_id.clone(),
            contracts::ReopenRejectedOperationsEffect::Resolved {
                author_action_sequence: applied.author_action_sequence.to_string(),
                undo_disposition: contracts::AuthorUndoDisposition::Forward,
                operation_ids: vec![input.selected_rejected_operation_id.clone()],
                rejection_event_refs: vec![input.rejection_event_id.clone()],
                prior_resolution: "rejected".to_owned(),
                resulting_resolution: "pending".to_owned(),
                resulting_proposal_revision_id: applied.effect.resulting_proposal_revision_id,
                resulting_validation: "pending".to_owned(),
                preserved_generation: applied.effect.preserved_generation,
                preserved_closure: applied.effect.preserved_closure,
                state_event_refs: vec![applied.effect.state_event_id],
            },
        ),
        TransitionOutcome::NoEffect(reason) => match reason {},
        TransitionOutcome::Conflicted(reason) => (
            contracts::ReopenReceiptResult::Conflicted,
            input.proposal_revision_id.clone(),
            contracts::ReopenRejectedOperationsEffect::Conflicted {
                reason: contract_reason(&reason)?,
            },
        ),
        TransitionOutcome::Refused(reason) => (
            contracts::ReopenReceiptResult::Refused,
            input.proposal_revision_id.clone(),
            contracts::ReopenRejectedOperationsEffect::Refused {
                reason: contract_reason(&reason)?,
            },
        ),
    };
    let envelope = &admitted.envelope;
    let project_scope = contract_scope(&envelope.project_scope);
    let head = vec![input.expected_authoritative_revision_id.clone()];
    Ok(Json(contracts::ReopenRejectedOperationsResponse {
        schema_id: contracts::REOPEN_REJECTED_OPERATIONS_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: envelope.correlation_id.clone(),
        project_scope: project_scope.clone(),
        command_id: settlement.ids.command_id,
        author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
        receipt: contracts::ReopenReceipt {
            receipt_id: settlement.ids.receipt_id,
            project_scope,
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: contracts::REOPEN_REJECTED_OPERATIONS_DIGEST_PROFILE.to_owned(),
                value_hex_lowercase: admitted.digest_hex.clone(),
            },
            idempotency_key: envelope.challenge_binding.idempotency_key.clone(),
            author_command_admission_id: settlement.ids.author_command_admission_id,
            proposal_id: input.proposal_id.clone(),
            source_proposal_revision_id: input.proposal_revision_id.clone(),
            resulting_proposal_revision_id,
            selected_rejected_operation_ids: vec![input.selected_rejected_operation_id.clone()],
            rejection_event_refs: vec![input.rejection_event_id.clone()],
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
