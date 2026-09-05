use storyos_application::{CreateChapterCommand, CreateChapterError};
use storyos_core::CreateChapterCurrent;
use uuid::Uuid;

use super::{create_chapter_database_error, create_chapter_parse_error};

pub(super) async fn next_chapter_order(
    client: &tokio_postgres::Client,
    command: &CreateChapterCommand,
) -> Result<u64, CreateChapterError> {
    client
        .query_one(
            "SELECT (COALESCE(MAX(tree_order), 0) + 1)::text
               FROM storyos.manuscript_objects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND object_kind = 'chapter' AND parent_volume_id = $3::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.volume_id,
            ],
        )
        .await
        .map_err(create_chapter_database_error)?
        .get::<_, String>(0)
        .parse::<u64>()
        .map_err(create_chapter_parse_error)
}

pub(super) async fn insert_created_chapter_object(
    client: &tokio_postgres::Client,
    command: &CreateChapterCommand,
    chapter_id: &str,
    tree_order: u64,
) -> Result<(), CreateChapterError> {
    let tree_order_text = tree_order.to_string();
    client
        .execute(
            "INSERT INTO storyos.manuscript_objects
               (owner_user_id, project_id, manuscript_object_id, object_kind, title, tree_order,
                parent_volume_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 'chapter', $4, $5::text::bigint,
                     $6::text::uuid)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &chapter_id,
                &command.title,
                &tree_order_text,
                &command.volume_id,
            ],
        )
        .await
        .map_err(create_chapter_database_error)?;
    Ok(())
}

pub(super) async fn persist_created_chapter(
    client: &tokio_postgres::Client,
    command: &CreateChapterCommand,
    tree_revision: &u64,
    chapter_id: &str,
    current: &CreateChapterCurrent,
) -> Result<(), CreateChapterError> {
    let payload_id = Uuid::now_v7().to_string();
    let revision_id = Uuid::now_v7().to_string();
    let empty: &[u8] = &[];
    client
        .execute(
            "INSERT INTO storyos.authoritative_payloads
               (owner_user_id, project_id, payload_id, canonical_bytes)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &payload_id,
                &empty,
            ],
        )
        .await
        .map_err(create_chapter_database_error)?;
    client
        .execute(
            "INSERT INTO storyos.authoritative_revisions
               (owner_user_id, project_id, manuscript_object_id, revision_id, payload_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5::text::uuid)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &chapter_id,
                &revision_id,
                &payload_id,
            ],
        )
        .await
        .map_err(create_chapter_database_error)?;
    client
        .execute(
            "INSERT INTO storyos.authoritative_heads
               (owner_user_id, project_id, manuscript_object_id, current_revision_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &chapter_id,
                &revision_id,
            ],
        )
        .await
        .map_err(create_chapter_database_error)?;
    let manuscript_block_id = Uuid::now_v7().to_string();
    crate::manuscript_block::insert_paragraph_block(
        client,
        command.project_scope.owner_user_id.as_ref(),
        command.project_scope.project_id.as_ref(),
        chapter_id,
        &revision_id,
        &manuscript_block_id,
    )
    .await
    .map_err(create_chapter_database_error)?;
    let updated = match current {
        CreateChapterCurrent::SelectCreated => client
            .execute(
                "UPDATE storyos.projects
                    SET tree_revision = $3::text::bigint, current_chapter_id = $5::text::uuid
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND tree_revision = $4::text::bigint AND lifecycle_state = 'active'
                    AND current_chapter_id IS NULL",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &tree_revision.to_string(),
                    &command.expected_tree_revision.to_string(),
                    &chapter_id,
                ],
            )
            .await
            .map_err(create_chapter_database_error)?,
        CreateChapterCurrent::PreserveExisting => client
            .execute(
                "UPDATE storyos.projects
                    SET tree_revision = $3::text::bigint
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND tree_revision = $4::text::bigint AND lifecycle_state = 'active'
                    AND current_chapter_id IS NOT NULL",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &tree_revision.to_string(),
                    &command.expected_tree_revision.to_string(),
                ],
            )
            .await
            .map_err(create_chapter_database_error)?,
    };
    if updated != 1 {
        return Err(CreateChapterError::Unavailable(Box::new(
            std::io::Error::other("tree revision or current Chapter changed under FOR UPDATE"),
        )));
    }
    Ok(())
}

pub(super) async fn insert_create_chapter_admission(
    client: &tokio_postgres::Client,
    command: &CreateChapterCommand,
) -> Result<(), CreateChapterError> {
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
                    'explicit_project_command', $9, $10, $11, 'createChapter',
                    $12, $13::text::uuid, challenge.consumed_at, challenge.expires_at,
                    $14::text::uuid, NULL, NULL, '{}'::uuid[], '{}'::text[], NULL, $7,
                    NULL, NULL, NULL, convert_from($15::bytea, 'UTF8')::jsonb
               FROM storyos.project_command_challenges AS challenge
              WHERE challenge.owner_user_id = $1::text::uuid
                AND challenge.project_id = $2::text::uuid
                AND challenge.command_kind = 'createChapter'
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
        .map_err(create_chapter_database_error)?;
    if inserted != 1 {
        return Err(CreateChapterError::InvalidChallenge);
    }
    Ok(())
}
