use super::{PostgresProjectReader, challenge_error, database_error, parse_error};
use crate::author_edit::parse_u64;
use crate::command_response_project::{
    CommandResponseProjectEvidence, read_command_response_project,
};
use storyos_application::{
    CompleteReadyPartialProposalCommand, CompleteReadyPartialProposalEffect, Project,
    ProposalGenerationDecisionError, ProposalGenerationSettlement,
};

pub(super) async fn read_complete_settlement(
    store: &PostgresProjectReader,
    command: &CompleteReadyPartialProposalCommand,
    receipt_id: &str,
) -> Result<
    ProposalGenerationSettlement<CompleteReadyPartialProposalEffect>,
    ProposalGenerationDecisionError,
> {
    let row = read_settlement_row(
        store,
        &command.project_scope,
        receipt_id,
        "completeReadyPartialProposal",
        &command.challenge_binding,
    )
    .await?;
    let effect = match (
        row.result_kind.as_str(),
        row.transition.as_deref(),
        row.reason.as_deref(),
    ) {
        ("proposal_generation_completed", Some("generation_completed"), _) => {
            CompleteReadyPartialProposalEffect::Completed {
                author_action_sequence: row.sequence.ok_or_else(historical)?,
                generation_id: row.resulting_generation_id.clone().ok_or_else(historical)?,
                preserved_validation: row.preserved_validation.clone().ok_or_else(historical)?,
                preserved_closure: row.preserved_closure.clone().ok_or_else(historical)?,
                preserved_operation_resolution: row
                    .preserved_operation_resolution
                    .clone()
                    .ok_or_else(historical)?,
                generation_event_id: row.transition_id.clone().ok_or_else(historical)?,
            }
        }
        ("conflicted", _, Some("changed_head")) => CompleteReadyPartialProposalEffect::Conflicted {
            reason: storyos_core::ProposalGenerationConflict::ChangedHead,
        },
        ("refused", _, Some(reason)) => CompleteReadyPartialProposalEffect::Refused {
            reason: parse_complete_reason(reason)?,
        },
        _ => return Err(historical()),
    };
    Ok(settlement_from_row(row, effect))
}

struct SettlementRow {
    ids: storyos_application::AuthorCommandAdmissionIds,
    result_kind: String,
    reason: Option<String>,
    transition: Option<String>,
    created_at: String,
    sequence: Option<u64>,
    transition_id: Option<String>,
    resulting_generation_id: Option<String>,
    preserved_validation: Option<String>,
    preserved_closure: Option<String>,
    preserved_operation_resolution: Option<String>,
    response_project: Project,
}

fn historical() -> ProposalGenerationDecisionError {
    ProposalGenerationDecisionError::HistoricalAcknowledgementUnavailable
}

fn settlement_from_row<T>(row: SettlementRow, effect: T) -> ProposalGenerationSettlement<T> {
    ProposalGenerationSettlement {
        ids: row.ids,
        effect,
        receipt_created_at: row.created_at,
        response_project: row.response_project,
    }
}

async fn read_settlement_row(
    store: &PostgresProjectReader,
    scope: &storyos_application::ProjectScope,
    receipt_id: &str,
    command_kind: &str,
    challenge: &storyos_application::ProjectCommandChallengeBinding,
) -> Result<SettlementRow, ProposalGenerationDecisionError> {
    let client = store.connect_challenge().await.map_err(challenge_error)?;
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .map_err(database_error)?;
    let result = async {
        crate::set_challenge_scope_on_client(&client, scope)
            .await
            .map_err(challenge_error)?;
        let row = client
            .query_opt(
                "SELECT receipt.command_id::text, receipt.author_command_admission_id::text,
                        receipt.receipt_id::text, receipt.result_kind,
                        receipt.result_payload->>'reason', receipt.result_payload->>'transition',
                        to_char(receipt.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                        action.author_action_sequence::text, transition.transition_id::text,
                        transition.resulting_generation_id::text, transition.preserved_validation,
                        transition.preserved_closure, transition.preserved_operation_resolution,
                        idempotency.acknowledgement_format, idempotency.response_project::text
                   FROM storyos.domain_receipts AS receipt
                   JOIN storyos.author_command_admission_settlements AS settlement
                     ON (settlement.owner_user_id, settlement.project_id,
                         settlement.author_command_admission_id, settlement.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id,
                         receipt.author_command_admission_id, receipt.receipt_id)
                   JOIN storyos.command_idempotency AS idempotency
                     ON (idempotency.owner_user_id, idempotency.project_id,
                         idempotency.command_kind, idempotency.idempotency_key,
                         idempotency.result_reference) =
                        (receipt.owner_user_id, receipt.project_id, receipt.command_kind,
                         receipt.idempotency_key, receipt.receipt_id::text)
              LEFT JOIN storyos.author_action_entries AS action
                     ON (action.owner_user_id, action.project_id, action.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
              LEFT JOIN storyos.proposal_generation_transitions AS transition
                     ON (transition.owner_user_id, transition.project_id, transition.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
                  WHERE receipt.owner_user_id = $1::text::uuid
                    AND receipt.project_id = $2::text::uuid
                    AND receipt.receipt_id = $3::text::uuid
                    AND receipt.command_kind = $4
                    AND receipt.command_digest = $5
                    AND receipt.idempotency_key = $6::text::uuid
                    AND settlement.settlement_kind = 'receipt_settled'
                    AND idempotency.outcome_kind = 'settled'",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &receipt_id,
                    &command_kind,
                    &challenge.canonical_command_digest,
                    &challenge.idempotency_key,
                ],
            )
            .await
            .map_err(database_error)?;
        let Some(row) = row else {
            return Err(historical());
        };
        let response_project = match read_command_response_project(
            row.get::<_, Option<String>>(13).as_deref(),
            row.get::<_, Option<String>>(14).as_deref(),
        ) {
            Ok(CommandResponseProjectEvidence::Captured(project)) => project,
            Ok(CommandResponseProjectEvidence::HistoricalUnavailable) => return Err(historical()),
            Err(()) => {
                return Err(ProposalGenerationDecisionError::Unavailable(Box::new(
                    std::io::Error::other("Generation acknowledgement evidence is damaged"),
                )));
            }
        };
        let sequence = row
            .get::<_, Option<String>>(7)
            .map(parse_u64)
            .transpose()
            .map_err(parse_error)?;
        Ok(SettlementRow {
            ids: storyos_application::AuthorCommandAdmissionIds {
                command_id: row.get(0),
                author_command_admission_id: row.get(1),
                receipt_id: row.get(2),
            },
            result_kind: row.get(3),
            reason: row.get(4),
            transition: row.get(5),
            created_at: row.get(6),
            sequence,
            transition_id: row.get(8),
            resulting_generation_id: row.get(9),
            preserved_validation: row.get(10),
            preserved_closure: row.get(11),
            preserved_operation_resolution: row.get(12),
            response_project,
        })
    }
    .await;
    if result.is_ok() {
        client
            .batch_execute("COMMIT")
            .await
            .map_err(database_error)?;
    } else {
        client
            .batch_execute("ROLLBACK")
            .await
            .map_err(database_error)?;
    }
    result
}

fn parse_complete_reason(
    reason: &str,
) -> Result<storyos_core::CompleteReadyPartialProposalRefusal, ProposalGenerationDecisionError> {
    match reason {
        "stale_proposal_revision" => {
            Ok(storyos_core::CompleteReadyPartialProposalRefusal::StaleProposalRevision)
        }
        "not_eligible" => Ok(storyos_core::CompleteReadyPartialProposalRefusal::NotEligible),
        "not_ready_partial" => {
            Ok(storyos_core::CompleteReadyPartialProposalRefusal::NotReadyPartial)
        }
        "stale_generation" => {
            Ok(storyos_core::CompleteReadyPartialProposalRefusal::StaleGeneration)
        }
        "stale_candidate" => Ok(storyos_core::CompleteReadyPartialProposalRefusal::StaleCandidate),
        _ => Err(historical()),
    }
}
