use storyos_application::{
    AgentRunRecord, AgentRunStatus, AuthorCommandAdmissionIds, CreateAgentRunAdmission,
    CreateAgentRunCommand, CreateAgentRunError, ProjectScope,
};

use crate::command_response_project::{
    CommandResponseProjectEvidence, read_command_response_project,
};
use crate::{PostgresProjectReader, set_challenge_scope_on_client};

use super::{agent_run_challenge_error, agent_run_database_error, agent_run_parse_error};

pub(super) async fn read_create_agent_run_settlement(
    store: &PostgresProjectReader,
    command: &CreateAgentRunCommand,
    receipt_id: &str,
) -> Result<CreateAgentRunAdmission, CreateAgentRunError> {
    let client = store
        .connect_challenge()
        .await
        .map_err(agent_run_challenge_error)?;
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .map_err(agent_run_database_error)?;
    let result = async {
        set_challenge_scope_on_client(&client, &command.project_scope)
            .await
            .map_err(agent_run_challenge_error)?;
        let row = client
            .query_opt(
                "SELECT receipt.command_id::text,
                        receipt.author_command_admission_id::text,
                        receipt.receipt_id::text,
                        run.project_agent_id::text,
                        run.conversation_id::text,
                        run.memory_settings_revision::text,
                        run.run_id::text,
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
                   JOIN storyos.agent_runs AS run
                     ON (run.owner_user_id, run.project_id, run.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
                   JOIN storyos.project_activity_event_payloads AS payload
                     ON (payload.owner_user_id, payload.project_id, payload.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
                  WHERE receipt.owner_user_id = $1::text::uuid
                    AND receipt.project_id = $2::text::uuid
                    AND receipt.receipt_id = $3::text::uuid
                    AND receipt.command_kind = 'createAgentRun'
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
                ],
            )
            .await
            .map_err(agent_run_database_error)?
            .ok_or(CreateAgentRunError::BindingConflict)?;
        let response_project = match read_command_response_project(
            row.get::<_, Option<String>>(8).as_deref(),
            row.get::<_, Option<String>>(9).as_deref(),
        ) {
            Ok(CommandResponseProjectEvidence::Captured(project)) => project,
            Ok(CommandResponseProjectEvidence::HistoricalUnavailable) => {
                return Err(CreateAgentRunError::HistoricalAcknowledgementUnavailable);
            }
            Err(()) => {
                return Err(CreateAgentRunError::Unavailable(Box::new(
                    std::io::Error::other("createAgentRun acknowledgement evidence is damaged"),
                )));
            }
        };
        Ok(CreateAgentRunAdmission {
            ids: AuthorCommandAdmissionIds {
                command_id: row.get(0),
                author_command_admission_id: row.get(1),
                receipt_id: row.get(2),
            },
            project_agent_id: row.get(3),
            conversation_id: row.get(4),
            memory_settings_revision: row.get(5),
            run_id: row.get(6),
            project_activity_position: row
                .get::<_, String>(7)
                .parse::<u64>()
                .map_err(agent_run_parse_error)?,
            response_project,
        })
    }
    .await;
    match &result {
        Ok(_) => client
            .batch_execute("COMMIT")
            .await
            .map_err(agent_run_database_error)?,
        Err(_) => {
            let _rollback = client.batch_execute("ROLLBACK").await;
        }
    }
    result
}

pub(super) async fn load_agent_run(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<Option<AgentRunRecord>, CreateAgentRunError> {
    let row = client
        .query_opt(
            "SELECT project_agent_id::text, conversation_id::text,
                    memory_settings_revision::text, run_id::text, status
               FROM storyos.agent_runs
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    if row.get::<_, String>(4) != "queued" {
        return Err(CreateAgentRunError::Unavailable(Box::new(
            std::io::Error::other("The AgentRun status is not queued"),
        )));
    }
    Ok(Some(AgentRunRecord {
        project_agent_id: row.get(0),
        conversation_id: row.get(1),
        memory_settings_revision: row.get(2),
        run_id: row.get(3),
        status: AgentRunStatus::Queued,
        context: super::context::load_assembled_context(client, scope, run_id).await?,
    }))
}
