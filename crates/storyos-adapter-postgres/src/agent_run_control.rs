use storyos_application::{
    AgentRunControlCommand, AgentRunControlConflict, AgentRunControlEffect, AgentRunControlError,
    AgentRunControlIntent, AgentRunControlNoEffect, AgentRunControlSettlement,
    AgentRunControlStatus, AgentRunControlStore, ChapterId, Project, ProjectCommandChallengeError,
    ProjectCommandChallengeUse,
};
use storyos_core::{
    AgentRunLifecycle, CancelAgentRunResult, PauseAgentRunResult, classify_cancel_agent_run,
    classify_pause_agent_run,
};
use uuid::Uuid;

use crate::command_response_project::{
    COMMAND_RESPONSE_PROJECT_FORMAT, encode_command_response_project,
};

use super::*;

#[path = "agent_run_control_read.rs"]
mod read;

impl AgentRunControlStore for PostgresProjectReader {
    async fn control_agent_run(
        &self,
        command: &AgentRunControlCommand,
    ) -> Result<AgentRunControlSettlement, AgentRunControlError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(control_challenge_error)?;
        let challenge_use = transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(control_challenge_error)?;
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction
                    .rollback()
                    .await
                    .map_err(control_challenge_error)?;
                read::read_control_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(control_challenge_error)?;
                Err(AgentRunControlError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_control(&transaction.client, command).await {
                    Ok(settlement) => {
                        transaction
                            .commit_sql()
                            .await
                            .map_err(control_database_error)?;
                        Ok(settlement)
                    }
                    Err(error) => {
                        let _rollback = transaction.rollback().await;
                        Err(error)
                    }
                }
            }
        }
    }
}

async fn persist_control(
    client: &tokio_postgres::Client,
    command: &AgentRunControlCommand,
) -> Result<AgentRunControlSettlement, AgentRunControlError> {
    let project = client
        .query_opt(
            "SELECT title, current_chapter_id::text
               FROM storyos.projects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
              FOR UPDATE",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(control_database_error)?;
    let Some(project) = project else {
        return Err(AgentRunControlError::MissingProject);
    };
    let response_project = Project {
        project_id: command.project_scope.project_id.clone(),
        title: project.get(0),
        current_chapter_id: project.get::<_, Option<String>>(1).map(ChapterId::new),
    };
    let run = client
        .query_opt(
            "SELECT status
               FROM storyos.agent_runs
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid
              FOR UPDATE",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.run_id,
            ],
        )
        .await
        .map_err(control_database_error)?;
    let Some(run) = run else {
        return Err(AgentRunControlError::MissingRun);
    };
    let lifecycle = AgentRunLifecycle::parse(&run.get::<_, String>(0)).ok_or_else(|| {
        AgentRunControlError::Unavailable(Box::new(std::io::Error::other(
            "The AgentRun status is unknown",
        )))
    })?;
    let effect = match command.intent {
        AgentRunControlIntent::Pause => match classify_pause_agent_run(lifecycle) {
            PauseAgentRunResult::Applied => {
                apply_control(client, command, AgentRunControlStatus::Paused).await?
            }
            PauseAgentRunResult::AlreadyPaused => AgentRunControlEffect::NoEffect {
                reason: AgentRunControlNoEffect::AlreadyPaused,
            },
            PauseAgentRunResult::Terminal => AgentRunControlEffect::Conflicted {
                reason: AgentRunControlConflict::TerminalRun,
            },
        },
        AgentRunControlIntent::Cancel => match classify_cancel_agent_run(lifecycle) {
            CancelAgentRunResult::Applied => {
                apply_control(client, command, AgentRunControlStatus::Cancelled).await?
            }
            CancelAgentRunResult::AlreadyCancelled => AgentRunControlEffect::NoEffect {
                reason: AgentRunControlNoEffect::AlreadyCancelled,
            },
            CancelAgentRunResult::Terminal => AgentRunControlEffect::Conflicted {
                reason: AgentRunControlConflict::TerminalRun,
            },
        },
    };
    insert_control_admission(client, command).await?;
    let (result_kind, result_payload) = match &effect {
        AgentRunControlEffect::Applied { .. } => ("authoritative_applied", "{}".to_owned()),
        AgentRunControlEffect::NoEffect { reason } => (
            "no_effect",
            match reason {
                AgentRunControlNoEffect::AlreadyPaused => {
                    r#"{"reason":"already_paused"}"#.to_owned()
                }
                AgentRunControlNoEffect::AlreadyCancelled => {
                    r#"{"reason":"already_cancelled"}"#.to_owned()
                }
            },
        ),
        AgentRunControlEffect::Conflicted { .. } => {
            ("conflicted", r#"{"reason":"terminal_run"}"#.to_owned())
        }
    };
    let receipt_created_at = client
        .query_one(
            "INSERT INTO storyos.domain_receipts
               (owner_user_id, project_id, receipt_id, author_command_admission_id,
                command_id, command_kind, command_digest, idempotency_key, producer_cause,
                expected_heads, prior_heads, resulting_heads, authoritative_revision_ids,
                proposal_revision_ids, authoritative_commit_ids, draft_artifact_refs,
                artifact_lifecycle_event_refs, condition_refs, result_kind, result_payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6, $7, $8::text::uuid,
                     'author_command_admission', '{}'::uuid[], '{}'::uuid[], '{}'::uuid[],
                     '{}'::uuid[], '{}'::uuid[], '{}'::uuid[], '{}'::text[], '{}'::text[],
                     '{}'::text[], $9, $10::text::jsonb)
          RETURNING to_char(created_at AT TIME ZONE 'UTC',
                            'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.ids.author_command_admission_id,
                &command.ids.command_id,
                &command_kind(command.intent),
                &command.challenge_binding.canonical_command_digest,
                &command.challenge_binding.idempotency_key,
                &result_kind,
                &result_payload,
            ],
        )
        .await
        .map_err(control_database_error)?
        .get::<_, String>(0);
    client
        .execute(
            "INSERT INTO storyos.author_command_admission_settlements
               (owner_user_id, project_id, author_command_admission_id, settlement_kind, receipt_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid,
                     'receipt_settled', $4::text::uuid)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.author_command_admission_id,
                &command.ids.receipt_id,
            ],
        )
        .await
        .map_err(control_database_error)?;
    let project_activity_position = match &effect {
        AgentRunControlEffect::Applied {
            status,
            fence_generation,
            ..
        } => write_control_activity(client, command, *status, *fence_generation).await?,
        AgentRunControlEffect::NoEffect { .. } | AgentRunControlEffect::Conflicted { .. } => 0,
    };
    let encoded_project = encode_command_response_project(&response_project);
    client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled',
                    result_reference = $3,
                    acknowledgement_format = $5,
                    response_project = $6::text::jsonb
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = $7 AND idempotency_key = $4::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.challenge_binding.idempotency_key,
                &COMMAND_RESPONSE_PROJECT_FORMAT,
                &encoded_project,
                &command_kind(command.intent),
            ],
        )
        .await
        .map_err(control_database_error)?;
    Ok(AgentRunControlSettlement {
        ids: command.ids.clone(),
        receipt_created_at,
        project_activity_position,
        response_project,
        effect,
    })
}

async fn apply_control(
    client: &tokio_postgres::Client,
    command: &AgentRunControlCommand,
    control_status: AgentRunControlStatus,
) -> Result<AgentRunControlEffect, AgentRunControlError> {
    let status = match control_status {
        AgentRunControlStatus::Paused => "paused",
        AgentRunControlStatus::Cancelled => "cancelled",
    };
    let fence_generation = client
        .query_one(
            "UPDATE storyos.agent_runs
                SET status = $4,
                    fence_token = fence_token + 1,
                    lease_expires_at = NULL,
                    wakeup_pending = false
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid
          RETURNING fence_token",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.run_id,
                &status,
            ],
        )
        .await
        .map_err(control_database_error)?
        .get::<_, i64>(0);
    Ok(AgentRunControlEffect::Applied {
        run_id: command.run_id.clone(),
        status: control_status,
        fence_generation: u64::try_from(fence_generation).map_err(control_parse_error)?,
    })
}

async fn insert_control_admission(
    client: &tokio_postgres::Client,
    command: &AgentRunControlCommand,
) -> Result<(), AgentRunControlError> {
    let client_session_generation = command.client_binding.session_generation.to_string();
    let kind = command_kind(command.intent);
    let inserted = client
        .execute(
            "INSERT INTO storyos.author_command_admissions
               (owner_user_id, project_id, author_command_admission_id, command_id,
                editor_session_id, writer_generation, client_session_binding_ref,
                client_session_generation, client_contract_revision, security_policy_revision,
                action_class, method, route_template, command_schema, command_kind,
                canonical_command_digest, idempotency_key, challenge_consumed_at,
                challenge_expires_at, correlation_id, chapter_object_id,
                expected_authoritative_revision_id, expected_proposal_head_revision_ids,
                target_refs, observed_ownership_partition, editor_contract_revision,
                undo_group_id, completed_intent_record_id, local_intent_sequence, command_payload)
             SELECT $1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                    NULL, NULL, $5, $6::text::numeric, $7, $8,
                    'agent_run_control', $9, $10, $11, $16,
                    $12, $13::text::uuid, challenge.consumed_at, challenge.expires_at,
                    $14::text::uuid, NULL, NULL, '{}'::uuid[], '{}'::text[], NULL, $7,
                    NULL, NULL, NULL, convert_from($15::bytea, 'UTF8')::jsonb
               FROM storyos.project_command_challenges AS challenge
              WHERE challenge.owner_user_id = $1::text::uuid
                AND challenge.project_id = $2::text::uuid
                AND challenge.command_kind = $16
                AND challenge.idempotency_key = $13::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.author_command_admission_id,
                &command.ids.command_id,
                &command.client_binding.binding_ref,
                &client_session_generation,
                &command.client_binding.client_contract_revision,
                &command.client_binding.security_policy_revision,
                &command.challenge_binding.method,
                &command.challenge_binding.route_template,
                &command.challenge_binding.command_schema,
                &command.challenge_binding.canonical_command_digest,
                &command.challenge_binding.idempotency_key,
                &command.correlation_id,
                &command.canonical_command_bytes.as_slice(),
                &kind,
            ],
        )
        .await
        .map_err(control_database_error)?;
    if inserted != 1 {
        return Err(AgentRunControlError::InvalidChallenge);
    }
    Ok(())
}

async fn write_control_activity(
    client: &tokio_postgres::Client,
    command: &AgentRunControlCommand,
    status: AgentRunControlStatus,
    fence_generation: u64,
) -> Result<u64, AgentRunControlError> {
    let project_activity_position = client
        .query_one(
            "INSERT INTO storyos.scope_counters AS counters
               (owner_user_id, project_id, project_activity_position)
             VALUES ($1::text::uuid, $2::text::uuid, 1)
             ON CONFLICT (owner_user_id, project_id)
             DO UPDATE SET
               project_activity_position = counters.project_activity_position + 1
             RETURNING counters.project_activity_position::text",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(control_database_error)?
        .get::<_, String>(0)
        .parse::<u64>()
        .map_err(control_parse_error)?;
    let event_kind = match status {
        AgentRunControlStatus::Paused => "agent_run_paused",
        AgentRunControlStatus::Cancelled => "agent_run_cancelled",
    };
    let payload = serde_json::json!({
        "kind": event_kind,
        "run_id": command.run_id,
        "fence_generation": fence_generation.to_string(),
    })
    .to_string();
    client
        .execute(
            "INSERT INTO storyos.project_activity_event_payloads
               (owner_user_id, project_id, project_activity_position, project_activity_event_id,
                event_kind, receipt_id, receipt_result_kind, payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4::text::uuid,
                     $5, $6::text::uuid, 'authoritative_applied', $7::text::jsonb)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &project_activity_position.to_string(),
                &Uuid::now_v7().to_string(),
                &event_kind,
                &command.ids.receipt_id,
                &payload,
            ],
        )
        .await
        .map_err(control_database_error)?;
    Ok(project_activity_position)
}

pub(super) fn command_kind(intent: AgentRunControlIntent) -> &'static str {
    match intent {
        AgentRunControlIntent::Pause => "pauseAgentRun",
        AgentRunControlIntent::Cancel => "cancelAgentRun",
    }
}

pub(super) fn control_challenge_error(error: ProjectCommandChallengeError) -> AgentRunControlError {
    match error {
        ProjectCommandChallengeError::BindingConflict => AgentRunControlError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired => AgentRunControlError::InvalidChallenge,
        ProjectCommandChallengeError::RateLimited { .. }
        | ProjectCommandChallengeError::Unavailable(_) => {
            AgentRunControlError::Unavailable(Box::new(error))
        }
    }
}

pub(super) fn control_database_error(error: tokio_postgres::Error) -> AgentRunControlError {
    AgentRunControlError::Unavailable(Box::new(error))
}

pub(super) fn control_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> AgentRunControlError {
    AgentRunControlError::Unavailable(Box::new(error))
}
