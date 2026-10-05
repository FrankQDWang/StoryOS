use storyos_application::{
    CurrentProducerWithdrawal, EditorSessionId, ProjectCommandSettlement, ProposalWithdrawn,
    WithdrawProposalInput, WithdrawalNote,
};
use storyos_core::{
    TransitionOutcome, WithdrawProposalConflict, WithdrawProposalNoEffect, WithdrawProposalRefusal,
};

use super::command_admission::{
    Admitted, AntiForgery, BodyValidation, ProblemMapping, ProjectCommandRoute, RevisionMismatch,
    SchemaMismatch, TargetValidation, admit_body, controlled_project, read_body,
};
use super::contract_reason::contract_reason;
use super::*;

const WITHDRAW_PROPOSAL: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "Withdrawal",
    command_kind: "withdrawProposal",
    method: contracts::WITHDRAW_PROPOSAL_METHOD,
    path: contracts::WITHDRAW_PROPOSAL_PATH,
    schema_id: contracts::WITHDRAW_PROPOSAL_REQUEST_SCHEMA_ID,
    digest_profile: contracts::WITHDRAW_PROPOSAL_DIGEST_PROFILE,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    body_validation: BodyValidation::AfterRevisionCheck,
    target_validation: TargetValidation::AfterContentType,
    problem_mapping: ProblemMapping::Standard,
};

/// The author form settles through the command sequence. The current-producer form is an
/// AgentRun decision without a Command Challenge (ADR 0043).
enum WithdrawalForm {
    Author(WithdrawProposalInput),
    CurrentProducer(CurrentProducerWithdrawal),
}

pub(super) async fn withdraw_proposal(
    State(state): State<Arc<ServerState>>,
    Path((project_id, proposal_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::WithdrawProposalResponse>, ApiError> {
    let read = read_body::<contracts::WithdrawProposalRequest>(
        &state,
        &project_id,
        &[&proposal_id],
        request,
        &WITHDRAW_PROPOSAL,
    )
    .await?;
    // The form fields are validated before the schema and revision checks, as on main.
    let anti_forgery = match &read.body.withdraw_proposal_input {
        contracts::WithdrawProposalInput::Author {
            withdrawal_reason: contracts::AuthorWithdrawalReason::AuthorWithdrew { note },
            editor_session_id,
            ..
        } => {
            if matches!(note, contracts::BoundedAuthorNote::Present { text } if text.is_empty()) {
                return Err(invalid_request());
            }
            valid_uuid(editor_session_id)?;
            AntiForgery::Required
        }
        contracts::WithdrawProposalInput::CurrentProducer {
            producer:
                contracts::AgentRunDecisionProducer {
                    kind: contracts::AgentRunDecisionKind::AgentRunDecision,
                    run_id,
                    decision_id,
                },
            withdrawal_reason: contracts::CurrentProducerWithdrawalReason::CurrentProducerWithdrew,
            ..
        } => {
            valid_uuid(run_id)?;
            valid_uuid(decision_id)?;
            AntiForgery::Absent
        }
    };
    let admitted = admit_body(
        &state,
        read,
        &WITHDRAW_PROPOSAL,
        anti_forgery,
        |body: &contracts::WithdrawProposalRequest| {
            let (proposal_revision_id, expected_closure, expected_target_revisions) =
                match &body.withdraw_proposal_input {
                    contracts::WithdrawProposalInput::Author {
                        proposal_revision_id,
                        expected_closure,
                        expected_target_revisions,
                        ..
                    }
                    | contracts::WithdrawProposalInput::CurrentProducer {
                        proposal_revision_id,
                        expected_closure,
                        expected_target_revisions,
                        ..
                    } => (
                        proposal_revision_id,
                        expected_closure,
                        expected_target_revisions,
                    ),
                };
            let [expected_authoritative_revision_id] = expected_target_revisions.as_slice() else {
                return Err(invalid_request());
            };
            if expected_closure != "open" {
                return Err(invalid_request());
            }
            valid_uuid(proposal_revision_id)?;
            valid_uuid(expected_authoritative_revision_id)?;
            Ok(match &body.withdraw_proposal_input {
                contracts::WithdrawProposalInput::Author {
                    withdrawal_reason: contracts::AuthorWithdrawalReason::AuthorWithdrew { note },
                    editor_session_id,
                    ..
                } => WithdrawalForm::Author(WithdrawProposalInput {
                    editor_session_id: EditorSessionId::new(editor_session_id.clone()),
                    proposal_id: proposal_id.clone(),
                    proposal_revision_id: proposal_revision_id.clone(),
                    expected_authoritative_revision_id: expected_authoritative_revision_id.clone(),
                    withdrawal_note: match note {
                        contracts::BoundedAuthorNote::Omitted => WithdrawalNote::Omitted,
                        contracts::BoundedAuthorNote::Present { text } => {
                            WithdrawalNote::Present { text: text.clone() }
                        }
                    },
                }),
                contracts::WithdrawProposalInput::CurrentProducer {
                    producer:
                        contracts::AgentRunDecisionProducer {
                            run_id,
                            decision_id,
                            ..
                        },
                    ..
                } => WithdrawalForm::CurrentProducer(CurrentProducerWithdrawal {
                    run_id: run_id.clone(),
                    decision_id: decision_id.clone(),
                    proposal_id: proposal_id.clone(),
                    proposal_revision_id: proposal_revision_id.clone(),
                    expected_authoritative_revision_id: expected_authoritative_revision_id.clone(),
                }),
            })
        },
    )
    .await?;
    match &admitted.input {
        WithdrawalForm::Author(input) => {
            let settlement = admitted
                .store
                .withdraw_proposal(&admitted.envelope, input)
                .await
                .map_err(|error| WITHDRAW_PROPOSAL.problem(error))?;
            admitted.hold_first_acknowledgement().await;
            let author_command_admission_id = settlement.ids.author_command_admission_id.clone();
            withdraw_response(
                &admitted,
                ReceiptTarget {
                    proposal_id: &input.proposal_id,
                    proposal_revision_id: &input.proposal_revision_id,
                    expected_authoritative_revision_id: &input.expected_authoritative_revision_id,
                },
                settlement,
                Some(author_command_admission_id),
                |applied| {
                    (
                        Some(applied.author_action_sequence),
                        contracts::ProposalWithdrawalReason::AuthorWithdrew {
                            note: match &input.withdrawal_note {
                                WithdrawalNote::Omitted => contracts::BoundedAuthorNote::Omitted,
                                WithdrawalNote::Present { text } => {
                                    contracts::BoundedAuthorNote::Present { text: text.clone() }
                                }
                            },
                        },
                        applied.effect,
                    )
                },
            )
        }
        WithdrawalForm::CurrentProducer(withdrawal) => {
            let settlement = admitted
                .store
                .withdraw_proposal_as_current_producer(&admitted.envelope, withdrawal)
                .await
                .map_err(|error| WITHDRAW_PROPOSAL.problem(error))?;
            admitted.hold_first_acknowledgement().await;
            withdraw_response(
                &admitted,
                ReceiptTarget {
                    proposal_id: &withdrawal.proposal_id,
                    proposal_revision_id: &withdrawal.proposal_revision_id,
                    expected_authoritative_revision_id: &withdrawal
                        .expected_authoritative_revision_id,
                },
                settlement,
                /*author_command_admission_id*/ None,
                |withdrawn| {
                    (
                        /*author_action_sequence*/ None,
                        contracts::ProposalWithdrawalReason::CurrentProducerWithdrew,
                        withdrawn,
                    )
                },
            )
        }
    }
}

/// The Proposal Revision and expected head that the Receipt of both forms shows.
struct ReceiptTarget<'a> {
    proposal_id: &'a str,
    proposal_revision_id: &'a str,
    expected_authoritative_revision_id: &'a str,
}

/// The public acknowledgement of one settled Withdrawal of either form.
///
/// `resolved` gives the Author Action, the withdrawal reason, and the closure change of an
/// applied outcome.
fn withdraw_response<A>(
    admitted: &Admitted<WithdrawalForm>,
    target: ReceiptTarget<'_>,
    settlement: ProjectCommandSettlement<
        A,
        WithdrawProposalNoEffect,
        WithdrawProposalConflict,
        WithdrawProposalRefusal,
    >,
    author_command_admission_id: Option<String>,
    resolved: impl FnOnce(
        A,
    ) -> (
        Option<u64>,
        contracts::ProposalWithdrawalReason,
        ProposalWithdrawn,
    ),
) -> Result<Json<contracts::WithdrawProposalResponse>, ApiError> {
    let (result, effect) = match settlement.outcome {
        TransitionOutcome::Applied(applied) => {
            let (author_action_sequence, withdrawal_reason, withdrawn) = resolved(applied);
            (
                contracts::WithdrawalReceiptResult::Resolved,
                contracts::WithdrawProposalEffect::Resolved {
                    undo_disposition: author_action_sequence
                        .map(|_| contracts::AuthorUndoDisposition::Forward),
                    author_action_sequence: author_action_sequence
                        .map(|sequence| sequence.to_string()),
                    preserved_generation: withdrawn.preserved_generation,
                    preserved_validation: withdrawn.preserved_validation,
                    prior_closure: "open".to_owned(),
                    resulting_closure: "withdrawn".to_owned(),
                    withdrawal_reason,
                    closure_event_refs: vec![withdrawn.withdrawal_event_id],
                },
            )
        }
        TransitionOutcome::NoEffect(reason) => (
            contracts::WithdrawalReceiptResult::NoEffect,
            contracts::WithdrawProposalEffect::NoEffect {
                reason: contract_reason(&reason)?,
            },
        ),
        TransitionOutcome::Conflicted(reason) => (
            contracts::WithdrawalReceiptResult::Conflicted,
            contracts::WithdrawProposalEffect::Conflicted {
                reason: contract_reason(&reason)?,
            },
        ),
        TransitionOutcome::Refused(reason) => (
            contracts::WithdrawalReceiptResult::Refused,
            contracts::WithdrawProposalEffect::Refused {
                reason: contract_reason(&reason)?,
            },
        ),
    };
    let envelope = &admitted.envelope;
    let project_scope = contract_scope(&envelope.project_scope);
    let head = vec![target.expected_authoritative_revision_id.to_owned()];
    Ok(Json(contracts::WithdrawProposalResponse {
        schema_id: contracts::WITHDRAW_PROPOSAL_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: envelope.correlation_id.clone(),
        project_scope: project_scope.clone(),
        command_id: settlement.ids.command_id,
        author_command_admission_id: author_command_admission_id.clone(),
        receipt: contracts::WithdrawalReceipt {
            receipt_id: settlement.ids.receipt_id,
            project_scope,
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: contracts::WITHDRAW_PROPOSAL_DIGEST_PROFILE.to_owned(),
                value_hex_lowercase: admitted.digest_hex.clone(),
            },
            idempotency_key: envelope.challenge_binding.idempotency_key.clone(),
            author_command_admission_id,
            proposal_id: target.proposal_id.to_owned(),
            proposal_revision_id: target.proposal_revision_id.to_owned(),
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
