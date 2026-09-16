use storyos_application::{ConversationSelection, CreateAgentRunCommand, CreateAgentRunError};

use super::{agent_run_database_error, agent_run_parse_error, agent_run_write_error};

pub(super) struct RunIdentities {
    pub(super) project_agent_id: String,
    pub(super) conversation_id: String,
    pub(super) memory_settings_revision: String,
    pub(super) run_id: String,
}

pub(super) async fn persist_conversation_and_run(
    client: &tokio_postgres::Client,
    command: &CreateAgentRunCommand,
    grant_id: &str,
    project_model_use_binding_revision: &str,
) -> Result<RunIdentities, CreateAgentRunError> {
    match &command.conversation {
        ConversationSelection::New => {
            insert_new_conversation(
                client,
                command,
                grant_id,
                project_model_use_binding_revision,
            )
            .await
        }
        ConversationSelection::Existing { conversation_id } => {
            insert_existing_conversation_run(
                client,
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
    command: &CreateAgentRunCommand,
    grant_id: &str,
    project_model_use_binding_revision: &str,
) -> Result<RunIdentities, CreateAgentRunError> {
    client
        .execute(
            "INSERT INTO storyos.project_agents
               (owner_user_id, project_id, project_agent_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid)
             ON CONFLICT (owner_user_id, project_id) DO NOTHING",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
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
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
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
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.conversation_id,
                &project_agent_id,
                &command.ids.receipt_id,
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
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.conversation_id,
                &memory_settings_revision,
                &command.ids.receipt_id,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    insert_queued_run(
        client,
        command,
        &project_agent_id,
        &command.conversation_id,
        &memory_settings_revision,
        grant_id,
        project_model_use_binding_revision,
    )
    .await?;
    Ok(RunIdentities {
        project_agent_id,
        conversation_id: command.conversation_id.clone(),
        memory_settings_revision,
        run_id: command.run_id.clone(),
    })
}

async fn insert_existing_conversation_run(
    client: &tokio_postgres::Client,
    command: &CreateAgentRunCommand,
    conversation_id: &str,
    grant_id: &str,
    project_model_use_binding_revision: &str,
) -> Result<RunIdentities, CreateAgentRunError> {
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
                AND conversation.conversation_id = $3::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &conversation_id,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    let project_agent_id = row.get::<_, String>(0);
    let memory_settings_revision = row.get::<_, String>(1);
    insert_queued_run(
        client,
        command,
        &project_agent_id,
        conversation_id,
        &memory_settings_revision,
        grant_id,
        project_model_use_binding_revision,
    )
    .await?;
    Ok(RunIdentities {
        project_agent_id,
        conversation_id: conversation_id.to_owned(),
        memory_settings_revision,
        run_id: command.run_id.clone(),
    })
}

async fn insert_queued_run(
    client: &tokio_postgres::Client,
    command: &CreateAgentRunCommand,
    project_agent_id: &str,
    conversation_id: &str,
    memory_settings_revision: &str,
    grant_id: &str,
    project_model_use_binding_revision: &str,
) -> Result<(), CreateAgentRunError> {
    client
        .execute(
            "INSERT INTO storyos.agent_runs
               (owner_user_id, project_id, run_id, project_agent_id, conversation_id,
                memory_settings_revision, grant_id, project_model_use_binding_revision,
                chapter_id, author_message, status, receipt_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::uuid, $7::text::uuid, $8::text::uuid,
                     $9::text::uuid, $10, 'queued', $11::text::uuid)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.run_id,
                &project_agent_id,
                &conversation_id,
                &memory_settings_revision,
                &grant_id,
                &project_model_use_binding_revision,
                &command.chapter_id,
                &command.author_message,
                &command.ids.receipt_id,
            ],
        )
        .await
        .map_err(agent_run_write_error)?;
    Ok(())
}

pub(super) async fn insert_create_agent_run_admission(
    client: &tokio_postgres::Client,
    command: &CreateAgentRunCommand,
) -> Result<(), CreateAgentRunError> {
    let client_session_generation = command.client_binding.session_generation.to_string();
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
                    'agent_run_start', $9, $10, $11, 'createAgentRun',
                    $12, $13::text::uuid, challenge.consumed_at, challenge.expires_at,
                    $14::text::uuid, NULL, NULL, '{}'::uuid[], '{}'::text[], NULL, $7,
                    NULL, NULL, NULL, convert_from($15::bytea, 'UTF8')::jsonb
               FROM storyos.project_command_challenges AS challenge
              WHERE challenge.owner_user_id = $1::text::uuid
                AND challenge.project_id = $2::text::uuid
                AND challenge.command_kind = 'createAgentRun'
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
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    if inserted != 1 {
        return Err(CreateAgentRunError::InvalidChallenge);
    }
    Ok(())
}

pub(super) async fn write_agent_run_activity(
    client: &tokio_postgres::Client,
    command: &CreateAgentRunCommand,
    identities: &RunIdentities,
) -> Result<u64, CreateAgentRunError> {
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
        .map_err(agent_run_database_error)?
        .get::<_, String>(0)
        .parse::<u64>()
        .map_err(agent_run_parse_error)?;
    let project_activity_event_id = uuid::Uuid::now_v7().to_string();
    let payload = serde_json::json!({
        "kind": "agent_run_created",
        "project_agent_id": identities.project_agent_id,
        "conversation_id": identities.conversation_id,
        "memory_settings_revision": identities.memory_settings_revision,
        "run_id": identities.run_id,
    })
    .to_string();
    client
        .execute(
            "INSERT INTO storyos.project_activity_event_payloads
               (owner_user_id, project_id, project_activity_position, project_activity_event_id,
                event_kind, receipt_id, receipt_result_kind, payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4::text::uuid,
                     'agent_run_created', $5::text::uuid, 'authoritative_applied',
                     $6::text::jsonb)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &project_activity_position.to_string(),
                &project_activity_event_id,
                &command.ids.receipt_id,
                &payload,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    Ok(project_activity_position)
}
