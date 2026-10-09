use storyos_application::{
    EditorSessionId, UndoLatestAuthorActionInput, UndoLatestAuthorActionSettlement, UndoRecords,
};
use storyos_core::{TransitionOutcome, UndoLatestAuthorActionConflict};

use super::command_admission::{
    Admitted, BodyValidation, ProblemMapping, ProjectCommandRoute, RevisionMismatch,
    SchemaMismatch, TargetValidation, admit, controlled_project,
};
use super::contract_reason::contract_reason;
use super::*;

const UNDO_LATEST_AUTHOR_ACTION: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "Undo Latest Author Action",
    command_kind: "undoLatestAuthorAction",
    method: contracts::UNDO_LATEST_AUTHOR_ACTION_METHOD,
    path: contracts::UNDO_LATEST_AUTHOR_ACTION_PATH,
    schema_id: contracts::UNDO_LATEST_AUTHOR_ACTION_REQUEST_SCHEMA_ID,
    digest_profile: contracts::UNDO_LATEST_AUTHOR_ACTION_DIGEST_PROFILE,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    body_validation: BodyValidation::AfterRevisionCheck,
    target_validation: TargetValidation::AfterContentType,
    problem_mapping: ProblemMapping::Standard,
};

pub(super) async fn undo_latest_author_action(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    request: Request,
) -> Result<Json<contracts::UndoLatestAuthorActionResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[],
        request,
        &UNDO_LATEST_AUTHOR_ACTION,
        |body: &contracts::UndoLatestAuthorActionRequest| {
            let input = &body.undo_latest_author_action_input;
            let Some(expected_frontier) = input
                .expected_author_undo_frontier_sequence
                .parse::<u64>()
                .ok()
                .filter(|sequence| *sequence >= 1)
            else {
                return Err(invalid_request());
            };
            // The correlation identity precedes the other identities in the problem order.
            valid_uuid(&input.correlation_id)?;
            valid_uuid(&input.expected_authoritative_revision_id)?;
            valid_uuid(&input.editor_session_id)?;
            Ok(UndoLatestAuthorActionInput {
                editor_session_id: EditorSessionId::new(input.editor_session_id.clone()),
                expected_author_undo_frontier_sequence: expected_frontier,
                expected_authoritative_revision_id: input
                    .expected_authoritative_revision_id
                    .clone(),
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .undo_latest_author_action(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| UNDO_LATEST_AUTHOR_ACTION.problem(error))?;
    admitted.hold_first_acknowledgement().await;
    undo_response(&admitted, settlement)
}

fn undo_response(
    admitted: &Admitted<UndoLatestAuthorActionInput>,
    settlement: UndoLatestAuthorActionSettlement,
) -> Result<Json<contracts::UndoLatestAuthorActionResponse>, ApiError> {
    let envelope = &admitted.envelope;
    let expected = admitted.input.expected_authoritative_revision_id.clone();
    let mut source_reopen_event = None;
    let mut acknowledgement = UndoAcknowledgement {
        proposal_id: None,
        proposal_revision_id: None,
        draft_refs: Vec::new(),
        lifecycle_refs: Vec::new(),
        revision_ids: Vec::new(),
        commit_ids: Vec::new(),
        resulting_head: expected.clone(),
        action_sequence: None,
    };
    let (receipt_result, effect) = match settlement.outcome {
        TransitionOutcome::Applied(applied) => {
            source_reopen_event = applied.source_reopen_event;
            acknowledgement.action_sequence = Some(applied.author_action_sequence.to_string());
            let frontier = applied
                .author_undo_frontier_sequence
                .map(|sequence| sequence.to_string());
            let compensated = |commit_id: Option<String>, revision, position: u64| {
                contracts::UndoLatestAuthorActionEffect::Compensated {
                    source_sequence: applied.source_sequence.to_string(),
                    author_action_sequence: applied.author_action_sequence.to_string(),
                    authoritative_commit_id: commit_id,
                    authoritative_revision: revision,
                    project_activity_position: position.to_string(),
                    author_undo_frontier_sequence: frontier.clone(),
                }
            };
            match applied.records {
                UndoRecords::Revision {
                    authoritative_commit_id,
                    revision_id,
                    body,
                    blocks,
                    project_activity_position,
                    proposal_id,
                    proposal_revision_id,
                } => {
                    acknowledgement.proposal_id = proposal_id;
                    acknowledgement.proposal_revision_id = proposal_revision_id;
                    acknowledgement.revision_ids = vec![revision_id.clone()];
                    acknowledgement.commit_ids = vec![authoritative_commit_id.clone()];
                    acknowledgement.resulting_head.clone_from(&revision_id);
                    (
                        contracts::DomainReceiptResult::AuthoritativeApplied,
                        compensated(
                            Some(authoritative_commit_id),
                            Some(contract_chapter_revision(revision_id, body, &blocks)),
                            project_activity_position,
                        ),
                    )
                }
                UndoRecords::Structure {
                    authoritative_commit_id,
                    snapshot_id: _,
                    project_activity_position,
                } => {
                    acknowledgement.commit_ids = vec![authoritative_commit_id.clone()];
                    (
                        contracts::DomainReceiptResult::AuthoritativeApplied,
                        compensated(
                            Some(authoritative_commit_id),
                            /*revision*/ None,
                            project_activity_position,
                        ),
                    )
                }
                UndoRecords::CurrentChapter {
                    snapshot_id: _,
                    project_activity_position,
                } => (
                    contracts::DomainReceiptResult::AuthoritativeApplied,
                    compensated(
                        /*commit_id*/ None,
                        /*revision*/ None,
                        project_activity_position,
                    ),
                ),
                UndoRecords::Proposal {
                    proposal_revision_id,
                    project_activity_position,
                } => {
                    acknowledgement.proposal_revision_id = proposal_revision_id;
                    (
                        contracts::DomainReceiptResult::AuthoritativeApplied,
                        compensated(
                            /*commit_id*/ None,
                            /*revision*/ None,
                            project_activity_position,
                        ),
                    )
                }
                UndoRecords::Draft { event } => {
                    acknowledgement.draft_refs = vec![event.draft_id.clone()];
                    acknowledgement.lifecycle_refs = vec![event.event_id.clone()];
                    acknowledgement.action_sequence = Some(event.author_action_sequence.clone());
                    (
                        contracts::DomainReceiptResult::DraftClosureChanged,
                        contracts::UndoLatestAuthorActionEffect::DraftCompensated {
                            event,
                            author_undo_frontier_sequence: frontier,
                        },
                    )
                }
                UndoRecords::ReversalRequired {
                    proposal_id,
                    proposal_revision_id,
                } => {
                    acknowledgement.proposal_id = Some(proposal_id.clone());
                    acknowledgement.proposal_revision_id = Some(proposal_revision_id.clone());
                    (
                        // The wire result names the reversal. Storage keeps the zero-commit undo shape.
                        contracts::DomainReceiptResult::ProposalRevised,
                        contracts::UndoLatestAuthorActionEffect::ReversalRequired {
                            proposal_id,
                            proposal_revision_id,
                            author_action_sequence: applied.author_action_sequence.to_string(),
                            source_sequence: applied.source_sequence.to_string(),
                        },
                    )
                }
            }
        }
        TransitionOutcome::NoEffect(reason) => match reason {},
        TransitionOutcome::Conflicted(reason) => (
            contracts::DomainReceiptResult::Conflicted,
            contracts::UndoLatestAuthorActionEffect::Conflicted {
                current_author_undo_frontier_sequence: match reason {
                    UndoLatestAuthorActionConflict::FrontierMismatch => settlement
                        .zero_authority_effect
                        .and_then(|frontier| frontier.current_author_undo_frontier_sequence)
                        .map(|sequence| sequence.to_string()),
                    UndoLatestAuthorActionConflict::WrongTargetHead
                    | UndoLatestAuthorActionConflict::SourceBindingChanged => None,
                },
                reason: contract_reason(&reason)?,
            },
        ),
        TransitionOutcome::Refused(reason) => (
            contracts::DomainReceiptResult::Refused,
            contracts::UndoLatestAuthorActionEffect::Unavailable {
                reason: contract_reason(&reason)?,
            },
        ),
    };
    if let Some(event) = &source_reopen_event {
        acknowledgement.draft_refs.push(event.draft_id.clone());
        acknowledgement.lifecycle_refs.push(event.event_id.clone());
    }
    let project_scope = contract_scope(&envelope.project_scope);
    let expected_heads = vec![expected];
    Ok(Json(contracts::UndoLatestAuthorActionResponse {
        proposal_id: acknowledgement.proposal_id,
        proposal_revision_id: acknowledgement.proposal_revision_id,
        source_reopen_event,
        schema_id: contracts::UNDO_LATEST_AUTHOR_ACTION_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: envelope.correlation_id.clone(),
        project_scope: project_scope.clone(),
        command_id: settlement.ids.command_id,
        author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
        receipt: contracts::DomainReceipt {
            receipt_id: settlement.ids.receipt_id,
            project_scope,
            command_kind: contracts::DomainReceiptCommandKind::UndoLatestAuthorAction,
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: contracts::UNDO_LATEST_AUTHOR_ACTION_DIGEST_PROFILE.to_owned(),
                value_hex_lowercase: admitted.digest_hex.clone(),
            },
            idempotency_key: envelope.challenge_binding.idempotency_key.clone(),
            producer_cause: contracts::DomainReceiptProducerCause::AuthorCommandAdmission,
            author_command_admission_id: settlement.ids.author_command_admission_id,
            expected_heads: expected_heads.clone(),
            prior_heads: expected_heads,
            resulting_heads: vec![acknowledgement.resulting_head],
            authoritative_revision_ids: acknowledgement.revision_ids,
            proposal_revision_ids: Vec::new(),
            authoritative_commit_ids: acknowledgement.commit_ids,
            author_action_sequence: acknowledgement.action_sequence,
            draft_artifact_refs: acknowledgement.draft_refs,
            artifact_lifecycle_event_refs: acknowledgement.lifecycle_refs,
            condition_refs: Vec::new(),
            result: receipt_result,
            created_at: settlement.receipt_created_at,
        },
        project: controlled_project(settlement.response),
        effect,
    }))
}

/// The receipt fields of an Undo response that depend on the outcome.
struct UndoAcknowledgement {
    proposal_id: Option<String>,
    proposal_revision_id: Option<String>,
    draft_refs: Vec<String>,
    lifecycle_refs: Vec<String>,
    revision_ids: Vec<String>,
    commit_ids: Vec<String>,
    resulting_head: String,
    action_sequence: Option<String>,
}
