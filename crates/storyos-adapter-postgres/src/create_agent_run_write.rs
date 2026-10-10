use storyos_application::{
    ConversationSelection, CreateAgentRunApplied, CreateAgentRunError, CreateAgentRunInput,
    ProjectCommandEnvelope,
};

use super::agent_run_database_error;

pub(super) async fn persist_conversation_and_run(
    client: &tokio_postgres::Client,
    envelope: &ProjectCommandEnvelope,
    command: &CreateAgentRunInput,
    grant_id: &str,
    project_model_use_binding_revision: &str,
) -> Result<CreateAgentRunApplied, CreateAgentRunError> {
    match &command.conversation {
        ConversationSelection::New => {
            insert_new_conversation(
                client,
                envelope,
                command,
                grant_id,
                project_model_use_binding_revision,
            )
            .await
        }
        ConversationSelection::Existing { conversation_id } => {
            insert_existing_conversation_run(
                client,
                envelope,
                command,
                conversation_id,
                grant_id,
                project_model_use_binding_revision,
            )
            .await
        }
    }
}

async fn insert_new_conversation(
    client: &tokio_postgres::Client,
    envelope: &ProjectCommandEnvelope,
    command: &CreateAgentRunInput,
    grant_id: &str,
    project_model_use_binding_revision: &str,
) -> Result<CreateAgentRunApplied, CreateAgentRunError> {
    client
        .execute(
            "INSERT INTO storyos.project_agents
               (owner_user_id, project_id, project_agent_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid)
             ON CONFLICT (owner_user_id, project_id) DO NOTHING",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &command.project_agent_id,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    let project_agent_id = client
        .query_one(
            "SELECT project_agent_id::text
               FROM storyos.project_agents
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(agent_run_database_error)?
        .get::<_, String>(0);
    client
        .execute(
            "INSERT INTO storyos.project_conversations
               (owner_user_id, project_id, conversation_id, project_agent_id, created_receipt_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5::text::uuid)",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &command.conversation_id,
                &project_agent_id,
                &envelope.ids.receipt_id,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    let memory_settings_revision = uuid::Uuid::now_v7().to_string();
    client
        .execute(
            "INSERT INTO storyos.conversation_memory_settings
               (owner_user_id, project_id, conversation_id, memory_settings_revision,
                use_enabled, contribution_enabled, receipt_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     TRUE, TRUE, $5::text::uuid)",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &command.conversation_id,
                &memory_settings_revision,
                &envelope.ids.receipt_id,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    insert_queued_run(
        client,
        envelope,
        command,
        CreateAgentRunApplied {
            project_agent_id,
            conversation_id: command.conversation_id.clone(),
            memory_settings_revision,
            run_id: command.run_id.clone(),
        },
        grant_id,
        project_model_use_binding_revision,
    )
    .await
}

async fn insert_existing_conversation_run(
    client: &tokio_postgres::Client,
    envelope: &ProjectCommandEnvelope,
    command: &CreateAgentRunInput,
    conversation_id: &str,
    grant_id: &str,
    project_model_use_binding_revision: &str,
) -> Result<CreateAgentRunApplied, CreateAgentRunError> {
    let row = client
        .query_one(
            "SELECT conversation.project_agent_id::text,
                    settings.memory_settings_revision::text
               FROM storyos.project_conversations AS conversation
               JOIN storyos.conversation_memory_settings AS settings
                 ON (settings.owner_user_id, settings.project_id, settings.conversation_id) =
                    (conversation.owner_user_id, conversation.project_id,
                     conversation.conversation_id)
              WHERE conversation.owner_user_id = $1::text::uuid
                AND conversation.project_id = $2::text::uuid
                AND conversation.conversation_id = $3::text::uuid
                AND COALESCE(settings.is_current, TRUE)",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &conversation_id,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    insert_queued_run(
        client,
        envelope,
        command,
        CreateAgentRunApplied {
            project_agent_id: row.get(/*idx*/ 0),
            conversation_id: conversation_id.to_owned(),
            memory_settings_revision: row.get(/*idx*/ 1),
            run_id: command.run_id.clone(),
        },
        grant_id,
        project_model_use_binding_revision,
    )
    .await
}

async fn insert_queued_run(
    client: &tokio_postgres::Client,
    envelope: &ProjectCommandEnvelope,
    command: &CreateAgentRunInput,
    run: CreateAgentRunApplied,
    grant_id: &str,
    project_model_use_binding_revision: &str,
) -> Result<CreateAgentRunApplied, CreateAgentRunError> {
    client
        .execute(
            "INSERT INTO storyos.agent_runs
               (owner_user_id, project_id, run_id, project_agent_id, conversation_id,
                memory_settings_revision, grant_id, project_model_use_binding_revision,
                chapter_id, author_message, status, receipt_id, model_registration_revision)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::uuid, $7::text::uuid, $8::text::uuid,
                     $9::text::uuid, $10, 'queued', $11::text::uuid,
                     (SELECT binding.model_registration_revision
                        FROM storyos.project_external_use_binding_revisions AS binding
                       WHERE binding.owner_user_id = $1::text::uuid
                         AND binding.project_id = $2::text::uuid
                         AND binding.project_model_use_binding_revision = $8::text::uuid))",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &run.run_id,
                &run.project_agent_id,
                &run.conversation_id,
                &run.memory_settings_revision,
                &grant_id,
                &project_model_use_binding_revision,
                &command.chapter_id,
                &command.author_message,
                &envelope.ids.receipt_id,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    Ok(run)
}
