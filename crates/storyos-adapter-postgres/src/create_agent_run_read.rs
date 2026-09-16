use storyos_application::{
    AgentRunDecisionInspect, AgentRunEvidence, AgentRunModelInspect, AgentRunRecord,
    AgentRunStatus, AgentRunStreamItem, AuthorCommandAdmissionIds, CreateAgentRunAdmission,
    CreateAgentRunCommand, CreateAgentRunError, EvidenceAvailability, ProjectScope,
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
            "SELECT run.project_agent_id::text, run.conversation_id::text,
                    run.memory_settings_revision::text, run.run_id::text, run.status,
                    run.settlement::text,
                    attempt.model_attempt_id::text, attempt.destination_attempt_id::text,
                    attempt.outbound_disclosure_event_id::text,
                    attempt.model_invocation_id::text, attempt.dispatch_state,
                    attempt.decision_id::text, attempt.continuation_binding_id::text,
                    attempt.payload::text
               FROM storyos.agent_runs AS run
               LEFT JOIN storyos.model_attempts AS attempt
                 ON (attempt.owner_user_id, attempt.project_id, attempt.run_id) =
                    (run.owner_user_id, run.project_id, run.run_id)
              WHERE run.owner_user_id = $1::text::uuid
                AND run.project_id = $2::text::uuid
                AND run.run_id = $3::text::uuid",
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
    let status = parse_run_status(&row.get::<_, String>(4))?;
    let settlement = row
        .get::<_, Option<String>>(5)
        .as_deref()
        .and_then(|value| serde_json::from_str(value).ok());
    let payload = row
        .get::<_, Option<String>>(13)
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(agent_run_parse_error)?;
    let model = match row.get::<_, Option<String>>(6) {
        Some(model_attempt_id) => Some(AgentRunModelInspect {
            model_attempt_id,
            destination_attempt_id: row.get(7),
            outbound_disclosure_event_id: row.get(8),
            model_invocation_id: row.get(9),
            dispatch_state: row.get(10),
            evidence: payload
                .as_ref()
                .and_then(|value: &serde_json::Value| value.get("evidence"))
                .and_then(serde_json::Value::as_array)
                .map(Vec::as_slice)
                .map(parse_evidence)
                .transpose()?
                .unwrap_or_default(),
            items: payload
                .as_ref()
                .and_then(|value: &serde_json::Value| value.get("items"))
                .and_then(serde_json::Value::as_array)
                .map(Vec::as_slice)
                .map(parse_items)
                .unwrap_or_default(),
            usage_kind: payload
                .as_ref()
                .and_then(|value| value.pointer("/usage/kind"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown")
                .to_owned(),
        }),
        None => None,
    };
    Ok(Some(AgentRunRecord {
        project_agent_id: row.get(0),
        conversation_id: row.get(1),
        memory_settings_revision: row.get(2),
        run_id: row.get(3),
        status,
        context: super::context::load_assembled_context(client, scope, run_id).await?,
        decision: inspect_decision(
            settlement.as_ref(),
            payload.as_ref(),
            row.get::<_, Option<String>>(12),
        ),
        model,
    }))
}

fn parse_run_status(status: &str) -> Result<AgentRunStatus, CreateAgentRunError> {
    match status {
        "queued" => Ok(AgentRunStatus::Queued),
        "claimed" => Ok(AgentRunStatus::Claimed),
        "waiting" => Ok(AgentRunStatus::Waiting),
        "completed" => Ok(AgentRunStatus::Completed),
        "refused" => Ok(AgentRunStatus::Refused),
        _ => Err(CreateAgentRunError::Unavailable(Box::new(
            std::io::Error::other("The AgentRun status is unknown"),
        ))),
    }
}

fn inspect_decision(
    settlement: Option<&serde_json::Value>,
    payload: Option<&serde_json::Value>,
    continuation_binding_id: Option<String>,
) -> AgentRunDecisionInspect {
    if let Some(capability) = settlement
        .and_then(|value| value.get("capability"))
        .and_then(serde_json::Value::as_str)
    {
        return AgentRunDecisionInspect::ExecutionRefused {
            capability: capability.to_owned(),
        };
    }
    let Some(decision) = payload.and_then(|value| value.get("decision")) else {
        return AgentRunDecisionInspect::Absent;
    };
    if decision.is_null() {
        return AgentRunDecisionInspect::Absent;
    }
    let selected = decision
        .get("selected")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if !selected {
        return AgentRunDecisionInspect::Absent;
    }
    let decision_id = decision
        .get("decision_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    match decision.get("kind").and_then(serde_json::Value::as_str) {
        Some("advisory") => AgentRunDecisionInspect::Advisory {
            decision_id,
            selected,
            text: decision
                .get("text")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            continuation_binding_id,
        },
        Some("prose_change") => AgentRunDecisionInspect::ProseChange {
            decision_id,
            selected,
            text: decision
                .get("text")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            producer_input: decision
                .get("producer_input")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            continuation_binding_id,
        },
        Some("clarification") => {
            let question = decision
                .get("question")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned();
            AgentRunDecisionInspect::Clarification {
                decision_id,
                selected,
                required_reply: decision
                    .get("required_reply")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(&question)
                    .to_owned(),
                question,
                continuation_binding_id,
            }
        }
        _ => AgentRunDecisionInspect::Absent,
    }
}

fn parse_evidence(
    values: &[serde_json::Value],
) -> Result<Vec<AgentRunEvidence>, CreateAgentRunError> {
    values
        .iter()
        .map(|value| {
            let kind = value.get("kind").and_then(serde_json::Value::as_str);
            let attempt_id = required_string(value, "attempt_id")?;
            let availability = match value
                .get("availability")
                .and_then(serde_json::Value::as_str)
            {
                Some("current") => EvidenceAvailability::Current,
                Some("unknown") => EvidenceAvailability::Unknown,
                _ => {
                    return Err(CreateAgentRunError::Unavailable(Box::new(
                        std::io::Error::other("Attempt evidence availability is unknown"),
                    )));
                }
            };
            Ok(match kind {
                Some("sent_content") => AgentRunEvidence::SentContent {
                    attempt_id,
                    availability,
                    content: required_string(value, "content")?,
                },
                Some("stored_reference") => AgentRunEvidence::StoredReference {
                    attempt_id,
                    availability,
                    reference_id: required_string(value, "reference_id")?,
                },
                Some("provider_report") => AgentRunEvidence::ProviderReport {
                    attempt_id,
                    availability,
                    report: required_string(value, "report")?,
                },
                Some("provider_opaque") => AgentRunEvidence::ProviderOpaque {
                    attempt_id,
                    availability,
                    unknown_facts: value
                        .get("unknown_facts")
                        .and_then(serde_json::Value::as_array)
                        .map(|facts| {
                            facts
                                .iter()
                                .filter_map(serde_json::Value::as_str)
                                .map(str::to_owned)
                                .collect()
                        })
                        .unwrap_or_default(),
                },
                _ => {
                    return Err(CreateAgentRunError::Unavailable(Box::new(
                        std::io::Error::other("Attempt evidence kind is unknown"),
                    )));
                }
            })
        })
        .collect()
}

fn parse_items(values: &[serde_json::Value]) -> Vec<AgentRunStreamItem> {
    values
        .iter()
        .map(|value| {
            let state = value
                .get("state")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned();
            AgentRunStreamItem {
                item_id: value
                    .get("item_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                role: value
                    .get("role")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                phase: value
                    .get("phase")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(&state)
                    .to_owned(),
                state,
                text: optional_string(value, "text"),
                summary: optional_string(value, "summary"),
                call_id: optional_string(value, "call_id"),
                arguments: optional_string(value, "arguments"),
                refusal: optional_string(value, "refusal"),
                hosted_report: optional_string(value, "hosted_report"),
            }
        })
        .collect()
}

fn optional_string(value: &serde_json::Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

fn required_string(value: &serde_json::Value, field: &str) -> Result<String, CreateAgentRunError> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            CreateAgentRunError::Unavailable(Box::new(std::io::Error::other(
                "Attempt evidence is damaged",
            )))
        })
}
