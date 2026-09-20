use storyos_application::{
    AgentRunControlCommand, AgentRunControlConflict, AgentRunControlEffect, AgentRunControlError,
    AgentRunControlIntent, AgentRunControlNoEffect, AgentRunControlSettlement,
    AgentRunControlStatus, AuthorCommandAdmissionIds,
};

use crate::command_response_project::{
    CommandResponseProjectEvidence, read_command_response_project,
};
use crate::{PostgresProjectReader, set_challenge_scope_on_client};

use super::{command_kind, control_challenge_error, control_database_error, control_parse_error};

pub(super) async fn read_control_settlement(
    store: &PostgresProjectReader,
    command: &AgentRunControlCommand,
    receipt_id: &str,
) -> Result<AgentRunControlSettlement, AgentRunControlError> {
    let client = store
        .connect_challenge()
        .await
        .map_err(control_challenge_error)?;
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .map_err(control_database_error)?;
    let result = async {
        set_challenge_scope_on_client(&client, &command.project_scope)
            .await
            .map_err(control_challenge_error)?;
        let row = client
            .query_opt(
                "SELECT receipt.command_id::text,
                        receipt.author_command_admission_id::text,
                        receipt.receipt_id::text,
                        to_char(receipt.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                        receipt.result_kind,
                        receipt.result_payload->>'reason',
                        payload.payload->>'run_id',
                        payload.payload->>'fence_generation',
                        payload.project_activity_position::text,
                        idempotency.acknowledgement_format,
                        idempotency.response_project::text
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
              LEFT JOIN storyos.project_activity_event_payloads AS payload
                     ON (payload.owner_user_id, payload.project_id, payload.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
                  WHERE receipt.owner_user_id = $1::text::uuid
                    AND receipt.project_id = $2::text::uuid
                    AND receipt.receipt_id = $3::text::uuid
                    AND receipt.command_kind = $6
                    AND receipt.command_digest = $4
                    AND receipt.idempotency_key = $5::text::uuid
                    AND settlement.settlement_kind = 'receipt_settled'
                    AND idempotency.outcome_kind = 'settled'",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &receipt_id,
                    &command.challenge_binding.canonical_command_digest,
                    &command.challenge_binding.idempotency_key,
                    &command_kind(command.intent),
                ],
            )
            .await
            .map_err(control_database_error)?
            .ok_or(AgentRunControlError::BindingConflict)?;
        let result_kind = row.get::<_, String>(4);
        let reason = row.get::<_, Option<String>>(5);
        let effect = match (result_kind.as_str(), reason.as_deref(), command.intent) {
            ("authoritative_applied", None, intent) => AgentRunControlEffect::Applied {
                run_id: row
                    .get::<_, Option<String>>(6)
                    .ok_or(AgentRunControlError::BindingConflict)?,
                status: match intent {
                    AgentRunControlIntent::Pause => AgentRunControlStatus::Paused,
                    AgentRunControlIntent::Cancel => AgentRunControlStatus::Cancelled,
                },
                fence_generation: row
                    .get::<_, Option<String>>(7)
                    .ok_or(AgentRunControlError::BindingConflict)?
                    .parse::<u64>()
                    .map_err(control_parse_error)?,
            },
            ("no_effect", Some("already_paused"), AgentRunControlIntent::Pause) => {
                AgentRunControlEffect::NoEffect {
                    reason: AgentRunControlNoEffect::AlreadyPaused,
                }
            }
            ("no_effect", Some("already_cancelled"), AgentRunControlIntent::Cancel) => {
                AgentRunControlEffect::NoEffect {
                    reason: AgentRunControlNoEffect::AlreadyCancelled,
                }
            }
            ("conflicted", Some("terminal_run"), _) => AgentRunControlEffect::Conflicted {
                reason: AgentRunControlConflict::TerminalRun,
            },
            _ => return Err(AgentRunControlError::BindingConflict),
        };
        let response_project = match read_command_response_project(
            row.get::<_, Option<String>>(9).as_deref(),
            row.get::<_, Option<String>>(10).as_deref(),
        ) {
            Ok(CommandResponseProjectEvidence::Captured(project)) => project,
            Ok(CommandResponseProjectEvidence::HistoricalUnavailable) => {
                return Err(AgentRunControlError::HistoricalAcknowledgementUnavailable);
            }
            Err(()) => return Err(AgentRunControlError::BindingConflict),
        };
        Ok(AgentRunControlSettlement {
            ids: AuthorCommandAdmissionIds {
                command_id: row.get(0),
                author_command_admission_id: row.get(1),
                receipt_id: row.get(2),
            },
            receipt_created_at: row.get(3),
            project_activity_position: row
                .get::<_, Option<String>>(8)
                .unwrap_or_else(|| "0".to_owned())
                .parse::<u64>()
                .map_err(control_parse_error)?,
            response_project,
            effect,
        })
    }
    .await;
    match &result {
        Ok(_) => client
            .batch_execute("COMMIT")
            .await
            .map_err(control_database_error)?,
        Err(_) => {
            let _rollback = client.batch_execute("ROLLBACK").await;
        }
    }
    result
}
