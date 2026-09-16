use storyos_application::{
    AgentRunRecord, ChapterId, ConversationSelection, CreateAgentRunAdmission,
    CreateAgentRunCommand, CreateAgentRunError, CreateAgentRunStore, Project,
    ProjectCommandChallengeError, ProjectCommandChallengeUse, ProjectScope,
};
use storyos_core::{
    AssistanceAdmission, AssistanceAvailability, ChapterAdmission, ConversationAdmission,
    CreateAgentRun as CoreCreateAgentRun, CreateAgentRunRefusal, CreateAgentRunResult,
    ProjectLifecycle, ProjectPresence, create_agent_run,
};

use crate::command_response_project::{
    COMMAND_RESPONSE_PROJECT_FORMAT, encode_command_response_project,
};
use crate::update_project_assistance::read_assistance_record;

use super::*;

#[path = "create_agent_run_read.rs"]
mod read;
#[path = "create_agent_run_write.rs"]
mod write;

impl CreateAgentRunStore for PostgresProjectReader {
    async fn create_agent_run(
        &self,
        command: &CreateAgentRunCommand,
    ) -> Result<CreateAgentRunAdmission, CreateAgentRunError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(agent_run_challenge_error)?;
        let challenge_use = transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(agent_run_challenge_error)?;
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction
                    .rollback()
                    .await
                    .map_err(agent_run_challenge_error)?;
                read::read_create_agent_run_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(agent_run_challenge_error)?;
                Err(CreateAgentRunError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_create_agent_run(&transaction.client, command).await {
                    Ok(admission) => {
                        transaction
                            .commit()
                            .await
                            .map_err(agent_run_challenge_error)?;
                        Ok(admission)
                    }
                    Err(error) => {
                        let _rollback = transaction.rollback().await;
                        Err(error)
                    }
                }
            }
        }
    }

    async fn read_agent_run(
        &self,
        scope: &ProjectScope,
        run_id: &str,
    ) -> Result<Option<AgentRunRecord>, CreateAgentRunError> {
        let client = self
            .connect_challenge()
            .await
            .map_err(agent_run_challenge_error)?;
        client
            .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .await
            .map_err(agent_run_database_error)?;
        let result = async {
            set_challenge_scope_on_client(&client, scope)
                .await
                .map_err(agent_run_challenge_error)?;
            read::load_agent_run(&client, scope, run_id).await
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
}

async fn persist_create_agent_run(
    client: &tokio_postgres::Client,
    command: &CreateAgentRunCommand,
) -> Result<CreateAgentRunAdmission, CreateAgentRunError> {
    let row = client
        .query_opt(
            "SELECT title, lifecycle_state, current_chapter_id::text
               FROM storyos.projects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
              FOR UPDATE",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    let Some(row) = row else {
        return Err(CreateAgentRunError::MissingProject);
    };
    let current_title = row.get::<_, String>(0);
    let lifecycle = if row.get::<_, String>(1) == "archived" {
        ProjectLifecycle::Archived
    } else {
        ProjectLifecycle::Active
    };
    let current_chapter_id = row.get::<_, Option<String>>(2);
    let assistance_record = read_assistance_record(client, &command.project_scope)
        .await
        .map_err(|error| CreateAgentRunError::Unavailable(Box::new(error)))?;
    let assistance = match &assistance_record {
        Some(record) if record.availability == AssistanceAvailability::Available => {
            AssistanceAdmission::Available
        }
        Some(_) => AssistanceAdmission::Unavailable,
        None => AssistanceAdmission::Missing,
    };
    let conversation = match &command.conversation {
        ConversationSelection::New => ConversationAdmission::New,
        ConversationSelection::Existing { conversation_id } => {
            conversation_admission(client, command, conversation_id).await?
        }
    };
    let chapter = if current_chapter_id.as_deref() == Some(command.chapter_id.as_str()) {
        ChapterAdmission::Current
    } else {
        ChapterAdmission::Invalid
    };
    match create_agent_run(&CoreCreateAgentRun {
        presence: ProjectPresence::Present,
        lifecycle,
        assistance,
        conversation,
        chapter,
    }) {
        CreateAgentRunResult::Admitted => {}
        CreateAgentRunResult::Refused { reason } => {
            return Err(match reason {
                CreateAgentRunRefusal::MissingProject => CreateAgentRunError::MissingProject,
                CreateAgentRunRefusal::ArchivedProject => CreateAgentRunError::ArchivedProject,
                CreateAgentRunRefusal::AssistanceUnavailable => {
                    CreateAgentRunError::AssistanceUnavailable
                }
                CreateAgentRunRefusal::InaccessibleConversation => {
                    CreateAgentRunError::InaccessibleConversation
                }
                CreateAgentRunRefusal::ConversationBusy => CreateAgentRunError::ConversationBusy,
                CreateAgentRunRefusal::InvalidChapterJoin => {
                    CreateAgentRunError::InvalidChapterJoin
                }
            });
        }
    }
    hold_conversation_if_requested(&command.challenge_binding.idempotency_key).await;
    write::insert_create_agent_run_admission(client, command).await?;
    client
        .execute(
            "INSERT INTO storyos.domain_receipts
               (owner_user_id, project_id, receipt_id, author_command_admission_id,
                command_id, command_kind, command_digest, idempotency_key, producer_cause,
                expected_heads, prior_heads, resulting_heads, authoritative_revision_ids,
                proposal_revision_ids, authoritative_commit_ids, draft_artifact_refs,
                artifact_lifecycle_event_refs, condition_refs, result_kind, result_payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, 'createAgentRun', $6, $7::text::uuid,
                     'author_command_admission', '{}'::uuid[], '{}'::uuid[], '{}'::uuid[],
                     '{}'::uuid[], '{}'::uuid[], '{}'::uuid[], '{}'::text[], '{}'::text[],
                     '{}'::text[], 'authoritative_applied', '{}'::jsonb)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.ids.author_command_admission_id,
                &command.ids.command_id,
                &command.challenge_binding.canonical_command_digest,
                &command.challenge_binding.idempotency_key,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
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
        .map_err(agent_run_database_error)?;
    let assistance_record = assistance_record.ok_or(CreateAgentRunError::AssistanceUnavailable)?;
    let identities = write::persist_conversation_and_run(
        client,
        command,
        &assistance_record.grant_id,
        &assistance_record.project_model_use_binding_revision,
    )
    .await?;
    let project_activity_position =
        write::write_agent_run_activity(client, command, &identities).await?;
    let response_project = Project {
        project_id: command.project_scope.project_id.clone(),
        title: current_title,
        current_chapter_id: current_chapter_id.map(ChapterId::new),
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
                AND command_kind = 'createAgentRun' AND idempotency_key = $4::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.challenge_binding.idempotency_key,
                &COMMAND_RESPONSE_PROJECT_FORMAT,
                &encoded_project,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    Ok(CreateAgentRunAdmission {
        ids: command.ids.clone(),
        project_agent_id: identities.project_agent_id,
        conversation_id: identities.conversation_id,
        memory_settings_revision: identities.memory_settings_revision,
        run_id: identities.run_id,
        project_activity_position,
        response_project,
    })
}

async fn conversation_admission(
    client: &tokio_postgres::Client,
    command: &CreateAgentRunCommand,
    conversation_id: &str,
) -> Result<ConversationAdmission, CreateAgentRunError> {
    let found = client
        .query_opt(
            "SELECT 1
               FROM storyos.project_conversations
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND conversation_id = $3::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &conversation_id,
            ],
        )
        .await
        .map_err(agent_run_database_error)?
        .is_some();
    if !found {
        return Ok(ConversationAdmission::ExistingMissing);
    }
    let busy = client
        .query_opt(
            "SELECT 1
               FROM storyos.agent_runs
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND conversation_id = $3::text::uuid
                AND status = 'queued'",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &conversation_id,
            ],
        )
        .await
        .map_err(agent_run_database_error)?
        .is_some();
    Ok(if busy {
        ConversationAdmission::ExistingBusy
    } else {
        ConversationAdmission::ExistingIdle
    })
}

async fn hold_conversation_if_requested(idempotency_key: &str) {
    let Ok(expected) = std::env::var("STORYOS_TEST_CONVERSATION_HOLD_IDEMPOTENCY_KEY") else {
        return;
    };
    if expected != idempotency_key {
        return;
    }
    let Ok(path) = std::env::var("STORYOS_TEST_CONVERSATION_HOLD_PATH") else {
        return;
    };
    if let Ok(reached) = std::env::var("STORYOS_TEST_CONVERSATION_HOLD_REACHED_PATH") {
        let _write = std::fs::write(reached, "held");
    }
    let path = std::path::PathBuf::from(path);
    while path.exists() {
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
}

pub(super) fn agent_run_challenge_error(
    error: ProjectCommandChallengeError,
) -> CreateAgentRunError {
    match error {
        ProjectCommandChallengeError::BindingConflict => CreateAgentRunError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired => CreateAgentRunError::InvalidChallenge,
        ProjectCommandChallengeError::RateLimited { .. }
        | ProjectCommandChallengeError::Unavailable(_) => {
            CreateAgentRunError::Unavailable(Box::new(error))
        }
    }
}

pub(super) fn agent_run_database_error(error: tokio_postgres::Error) -> CreateAgentRunError {
    CreateAgentRunError::Unavailable(Box::new(error))
}

pub(super) fn agent_run_write_error(error: tokio_postgres::Error) -> CreateAgentRunError {
    if error.code() == Some(&tokio_postgres::error::SqlState::UNIQUE_VIOLATION) {
        return CreateAgentRunError::ConversationBusy;
    }
    CreateAgentRunError::Unavailable(Box::new(error))
}

pub(super) fn agent_run_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> CreateAgentRunError {
    CreateAgentRunError::Unavailable(Box::new(error))
}
