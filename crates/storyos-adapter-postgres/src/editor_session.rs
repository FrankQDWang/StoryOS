use sha2::{Digest, Sha256};
use storyos_application::{
    EditorSession, EditorSessionError, EditorSessionLookup, EditorSessionSnapshot,
    EditorSessionStore, EditorWriterState, OpenEditorSession, ProjectCommandChallengeError,
    ProjectCommandChallengeTransaction, ProjectCommandChallengeUse,
};

use super::*;

impl EditorSessionStore for PostgresProjectReader {
    async fn create_editor_session(
        &self,
        request: &OpenEditorSession,
    ) -> Result<EditorSession, EditorSessionError> {
        let mut transaction = self
            .begin_project_command_transaction(&request.project_scope)
            .await
            .map_err(session_challenge_error)?;
        let challenge_use = transaction
            .consume(&request.challenge_binding, &request.nonce_digest)
            .await
            .map_err(session_challenge_error)?;
        let editor_session_id = match challenge_use {
            ProjectCommandChallengeUse::FirstUse => {
                transaction.client.execute(
                    "INSERT INTO storyos.editor_sessions
                       (owner_user_id, project_id, editor_session_id, client_session_binding_ref,
                        client_session_generation, client_contract_revision, security_policy_revision)
                     VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4, $5::text::numeric, $6, $7)",
                    &[&request.project_scope.owner_user_id.as_ref(), &request.project_scope.project_id.as_ref(),
                      &request.editor_session_id.as_ref(), &request.client_binding.binding_ref,
                      &request.client_binding.session_generation.to_string(),
                      &request.client_binding.client_contract_revision, &request.client_binding.security_policy_revision],
                ).await.map_err(session_database_error)?;
                transaction
                    .client
                    .execute(
                        "INSERT INTO storyos.project_writer_generations
                       (owner_user_id, project_id, writer_generation, current_editor_session_id)
                     VALUES ($1::text::uuid, $2::text::uuid, 1, $3::text::uuid)
                     ON CONFLICT DO NOTHING",
                        &[
                            &request.project_scope.owner_user_id.as_ref(),
                            &request.project_scope.project_id.as_ref(),
                            &request.editor_session_id.as_ref(),
                        ],
                    )
                    .await
                    .map_err(session_database_error)?;
                transaction.client.execute(
                    "INSERT INTO storyos.editor_session_base_snapshots
                       (owner_user_id, project_id, snapshot_id, editor_session_id,
                        chapter_object_id, authoritative_revision_id)
                     SELECT project.owner_user_id, project.project_id, $3::text::uuid, $4::text::uuid,
                            project.current_chapter_id, head.current_revision_id
                     FROM storyos.projects AS project
                     JOIN storyos.authoritative_heads AS head
                       ON (head.owner_user_id, head.project_id, head.manuscript_object_id) =
                          (project.owner_user_id, project.project_id, project.current_chapter_id)
                     WHERE project.owner_user_id = $1::text::uuid AND project.project_id = $2::text::uuid",
                    &[&request.project_scope.owner_user_id.as_ref(), &request.project_scope.project_id.as_ref(),
                      &request.snapshot_id, &request.editor_session_id.as_ref()],
                ).await.map_err(session_database_error)?;
                crate::snapshot::persist_canonical_snapshot(
                    &transaction.client,
                    &request.project_scope,
                    &request.snapshot_id,
                    /*project_activity_position*/ 0,
                )
                .await
                .map_err(session_database_error)?;
                transaction
                    .client
                    .execute(
                        "UPDATE storyos.command_idempotency
                     SET outcome_kind = 'settled', result_reference = $5
                     WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                       AND command_kind = $3 AND idempotency_key = $4::text::uuid",
                        &[
                            &request.project_scope.owner_user_id.as_ref(),
                            &request.project_scope.project_id.as_ref(),
                            &request.challenge_binding.command_kind,
                            &request.challenge_binding.idempotency_key,
                            &request.editor_session_id.as_ref(),
                        ],
                    )
                    .await
                    .map_err(session_database_error)?;
                request.editor_session_id.as_ref().to_owned()
            }
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => result_reference,
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(session_challenge_error)?;
                return Err(EditorSessionError::BindingConflict);
            }
        };
        let result = read_session(
            &transaction.client,
            &editor_session_id,
            &request.client_binding,
        )
        .await?
        .ok_or(EditorSessionError::BindingConflict)?;
        transaction
            .commit()
            .await
            .map_err(session_challenge_error)?;
        Ok(result)
    }

    async fn read_editor_session(
        &self,
        request: &EditorSessionLookup,
    ) -> Result<Option<EditorSession>, EditorSessionError> {
        let client = self
            .connect_challenge()
            .await
            .map_err(session_challenge_error)?;
        client
            .batch_execute("BEGIN")
            .await
            .map_err(session_database_error)?;
        let result = async {
            set_challenge_scope_on_client(&client, &request.project_scope)
                .await
                .map_err(session_challenge_error)?;
            read_session(
                &client,
                request.editor_session_id.as_ref(),
                &request.client_binding,
            )
            .await
        }
        .await;
        match result {
            Ok(session) => {
                client
                    .batch_execute("COMMIT")
                    .await
                    .map_err(session_database_error)?;
                Ok(session)
            }
            Err(error) => {
                client
                    .batch_execute("ROLLBACK")
                    .await
                    .map_err(session_database_error)?;
                Err(error)
            }
        }
    }
}

async fn read_session(
    client: &tokio_postgres::Client,
    editor_session_id: &str,
    binding: &storyos_application::EditorClientBinding,
) -> Result<Option<EditorSession>, EditorSessionError> {
    let generation = binding.session_generation.to_string();
    let row = client.query_opt(
        "SELECT session.client_session_binding_ref, session.client_session_generation::text,
                session.client_contract_revision, session.security_policy_revision,
                to_char(session.opened_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                writer.writer_generation::text, writer.current_editor_session_id::text,
                snapshot.snapshot_id::text, snapshot.chapter_object_id::text,
                snapshot.authoritative_revision_id::text,
                snapshot.project_activity_position::text,
                convert_from(payload.canonical_bytes, 'UTF8'),
                to_char(snapshot.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
         FROM storyos.editor_sessions AS session
         JOIN storyos.project_writer_generations AS writer USING (owner_user_id, project_id)
         JOIN storyos.editor_session_base_snapshots AS snapshot USING
           (owner_user_id, project_id, editor_session_id)
         JOIN storyos.authoritative_revisions AS revision
           ON (revision.owner_user_id, revision.project_id, revision.manuscript_object_id, revision.revision_id) =
              (snapshot.owner_user_id, snapshot.project_id, snapshot.chapter_object_id, snapshot.authoritative_revision_id)
         JOIN storyos.authoritative_payloads AS payload
           ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
              (revision.owner_user_id, revision.project_id, revision.payload_id)
         WHERE session.owner_user_id = current_setting('storyos.owner_user_id')::uuid
           AND session.project_id = current_setting('storyos.project_id')::uuid
           AND session.editor_session_id = $1::text::uuid
           AND session.client_session_binding_ref = $2
           AND session.client_session_generation = $3::text::numeric
           AND session.client_contract_revision = $4 AND session.security_policy_revision = $5",
        &[&editor_session_id, &binding.binding_ref, &generation,
          &binding.client_contract_revision, &binding.security_policy_revision],
    ).await.map_err(session_database_error)?;
    row.map(|row| {
        let writer_generation = row
            .get::<_, String>(5)
            .parse::<u64>()
            .map_err(|error| EditorSessionError::Unavailable(Box::new(error)))?;
        let project_activity_position = row
            .get::<_, String>(10)
            .parse::<u64>()
            .map_err(|error| EditorSessionError::Unavailable(Box::new(error)))?;
        let body = row.get::<_, String>(11);
        Ok(EditorSession {
            editor_session_id: storyos_application::EditorSessionId::new(editor_session_id),
            client_binding: binding.clone(),
            opened_at: row.get(4),
            writer: if row.get::<_, String>(6) == editor_session_id {
                EditorWriterState::CurrentWriter { writer_generation }
            } else {
                EditorWriterState::ReadOnly {
                    observed_writer_generation: writer_generation,
                }
            },
            base_snapshot: EditorSessionSnapshot {
                snapshot_id: row.get(7),
                chapter_id: row.get(8),
                authoritative_revision_id: row.get(9),
                project_activity_position,
                payload_digest_hex: Sha256::digest(body.as_bytes()).iter().fold(
                    String::with_capacity(64),
                    |mut encoded, byte| {
                        use std::fmt::Write as _;
                        write!(encoded, "{byte:02x}").expect("writing to String cannot fail");
                        encoded
                    },
                ),
                body,
                created_at: row.get(12),
            },
        })
    })
    .transpose()
}

fn session_challenge_error(error: ProjectCommandChallengeError) -> EditorSessionError {
    match error {
        ProjectCommandChallengeError::BindingConflict => EditorSessionError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired
        | ProjectCommandChallengeError::RateLimited { .. } => EditorSessionError::InvalidChallenge,
        ProjectCommandChallengeError::Unavailable(source) => {
            EditorSessionError::Unavailable(source)
        }
    }
}

fn session_database_error(error: tokio_postgres::Error) -> EditorSessionError {
    EditorSessionError::Unavailable(Box::new(error))
}
