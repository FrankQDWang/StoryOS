//! The Author Command Admission row of the sequence, for project, AgentRun, and editor commands.

use storyos_application::{ProjectCommandEnvelope, ProjectCommandError};
use tokio_postgres::Client;

use super::{
    Admission, CommandSpec, EditorAdmission, EditorWriter, MissingAdmission, ProjectActionClass,
    TakeoverAdmission, unavailable,
};

/// Inserts the Admission after the consumed Command Challenge. An insert without a row gives the command's error.
pub(super) async fn insert_admission(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    spec: &CommandSpec,
    admission: &Admission,
) -> Result<(), ProjectCommandError> {
    let binding = &envelope.client_binding;
    let challenge = &envelope.challenge_binding;
    let session_generation = binding.session_generation.to_string();
    let owner_user_id = envelope.project_scope.owner_user_id.as_ref();
    let project_id = envelope.project_scope.project_id.as_ref();
    let command_bytes = envelope.canonical_command_bytes.as_slice();
    let inserted = match admission {
        Admission::Project(action_class) => {
            let action_class = match action_class {
                ProjectActionClass::ExplicitProjectCommand => "explicit_project_command",
                ProjectActionClass::AgentRunStart => "agent_run_start",
                ProjectActionClass::AgentRunControl => "agent_run_control",
            };
            client
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
                    $17, $9, $10, $11, $16,
                    $12, $13::text::uuid, challenge.consumed_at, challenge.expires_at,
                    $14::text::uuid, NULL, NULL, '{}'::uuid[], '{}'::text[], NULL, $7,
                    NULL, NULL, NULL, convert_from($15::bytea, 'UTF8')::jsonb
               FROM storyos.project_command_challenges AS challenge
              WHERE challenge.owner_user_id = $1::text::uuid
                AND challenge.project_id = $2::text::uuid
                AND challenge.command_kind = $16
                AND challenge.idempotency_key = $13::text::uuid",
                    &[
                        &owner_user_id,
                        &project_id,
                        &envelope.ids.author_command_admission_id,
                        &envelope.ids.command_id,
                        &binding.binding_ref,
                        &session_generation,
                        &binding.client_contract_revision,
                        &binding.security_policy_revision,
                        &challenge.method,
                        &challenge.route_template,
                        &challenge.command_schema,
                        &challenge.canonical_command_digest,
                        &challenge.idempotency_key,
                        &envelope.correlation_id,
                        &command_bytes,
                        &spec.kind,
                        &action_class,
                    ],
                )
                .await
        }
        Admission::ExplicitEditorCommand(EditorAdmission {
            editor_session_id,
            chapter_object_id,
            expected_authoritative_revision_id,
            target_refs,
            writer,
        }) => {
            let client_writer_generation = match writer {
                EditorWriter::Current => None,
                EditorWriter::ClientGeneration(generation) => Some(generation.to_string()),
            };
            client
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
                    session.editor_session_id, writer.writer_generation,
                    session.client_session_binding_ref, $6::text::numeric,
                    session.client_contract_revision, session.security_policy_revision,
                    'explicit_editor_command', $7, $8, $9, $19,
                    $10, $11::text::uuid, challenge.consumed_at, challenge.expires_at,
                    $12::text::uuid, $13::text::uuid, $14::text::uuid, '{}'::uuid[], $21::text[],
                    NULL, session.client_contract_revision, NULL, NULL, NULL,
                    convert_from($15::bytea, 'UTF8')::jsonb
               FROM storyos.editor_sessions AS session
               JOIN storyos.project_writer_generations AS writer
                 ON (writer.owner_user_id, writer.project_id,
                     writer.current_editor_session_id) =
                    (session.owner_user_id, session.project_id, session.editor_session_id)
                AND writer.writer_generation = (
                  SELECT max(current_writer.writer_generation)
                    FROM storyos.project_writer_generations AS current_writer
                   WHERE current_writer.owner_user_id = session.owner_user_id
                     AND current_writer.project_id = session.project_id
                )
               JOIN storyos.project_command_challenges AS challenge
                 ON (challenge.owner_user_id, challenge.project_id,
                     challenge.command_kind, challenge.idempotency_key) =
                    (session.owner_user_id, session.project_id, $19, $11::text::uuid)
              WHERE session.owner_user_id = $1::text::uuid
                AND session.project_id = $2::text::uuid
                AND session.editor_session_id = $5::text::uuid
                AND session.client_session_binding_ref = $16
                AND session.client_session_generation = $6::text::numeric
                AND session.client_contract_revision = $17
                AND session.security_policy_revision = $18
                AND challenge.consumed_at IS NOT NULL
                AND ($20::text IS NULL OR writer.writer_generation = $20::text::numeric)",
                    &[
                        &owner_user_id,
                        &project_id,
                        &envelope.ids.author_command_admission_id,
                        &envelope.ids.command_id,
                        editor_session_id,
                        &session_generation,
                        &challenge.method,
                        &challenge.route_template,
                        &challenge.command_schema,
                        &challenge.canonical_command_digest,
                        &challenge.idempotency_key,
                        &envelope.correlation_id,
                        chapter_object_id,
                        expected_authoritative_revision_id,
                        &command_bytes,
                        &binding.binding_ref,
                        &binding.client_contract_revision,
                        &binding.security_policy_revision,
                        &spec.kind,
                        &client_writer_generation,
                        target_refs,
                    ],
                )
                .await
        }
        Admission::WriterTakeover(TakeoverAdmission {
            editor_session_id,
            observed_writer_generation,
            editor_contract_revision,
        }) => {
            client
                .execute(
                    "WITH current_writer AS MATERIALIZED (
                       SELECT writer.writer_generation, writer.current_editor_session_id
                         FROM storyos.project_writer_generations AS writer
                        WHERE writer.owner_user_id = $1::text::uuid
                          AND writer.project_id = $2::text::uuid
                        ORDER BY writer.writer_generation DESC
                        LIMIT 1
                     )
                     INSERT INTO storyos.author_command_admissions
                       (owner_user_id, project_id, author_command_admission_id, command_id,
                        editor_session_id, writer_generation, client_session_binding_ref,
                        client_session_generation, client_contract_revision,
                        security_policy_revision, action_class, method, route_template,
                        command_schema, command_kind, canonical_command_digest, idempotency_key,
                        challenge_consumed_at, challenge_expires_at, correlation_id,
                        chapter_object_id, expected_authoritative_revision_id,
                        expected_proposal_head_revision_ids, target_refs,
                        observed_ownership_partition, editor_contract_revision, undo_group_id,
                        completed_intent_record_id, local_intent_sequence, command_payload)
                     SELECT $1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                            session.editor_session_id, current_writer.writer_generation,
                            session.client_session_binding_ref, $7::text::numeric,
                            session.client_contract_revision, session.security_policy_revision,
                            'explicit_editor_command', $8, $9, $10, $19,
                            $11, $12::text::uuid, challenge.consumed_at, challenge.expires_at,
                            $13::text::uuid, NULL, NULL, '{}'::uuid[], '{}'::text[], NULL, $14,
                            NULL, NULL, NULL, convert_from($15::bytea, 'UTF8')::jsonb
                       FROM storyos.editor_sessions AS session
                       JOIN current_writer
                         ON current_writer.writer_generation = $6::text::numeric
                        AND current_writer.current_editor_session_id <> session.editor_session_id
                       JOIN storyos.project_command_challenges AS challenge
                         ON (challenge.owner_user_id, challenge.project_id,
                             challenge.command_kind, challenge.idempotency_key) =
                            (session.owner_user_id, session.project_id, $19, $12::text::uuid)
                      WHERE session.owner_user_id = $1::text::uuid
                        AND session.project_id = $2::text::uuid
                        AND session.editor_session_id = $5::text::uuid
                        AND session.client_session_binding_ref = $16
                        AND session.client_session_generation = $7::text::numeric
                        AND session.client_contract_revision = $17
                        AND session.security_policy_revision = $18
                        AND challenge.consumed_at IS NOT NULL",
                    &[
                        &owner_user_id,
                        &project_id,
                        &envelope.ids.author_command_admission_id,
                        &envelope.ids.command_id,
                        editor_session_id,
                        &observed_writer_generation.to_string(),
                        &session_generation,
                        &challenge.method,
                        &challenge.route_template,
                        &challenge.command_schema,
                        &challenge.canonical_command_digest,
                        &challenge.idempotency_key,
                        &envelope.correlation_id,
                        editor_contract_revision,
                        &command_bytes,
                        &binding.binding_ref,
                        &binding.client_contract_revision,
                        &binding.security_policy_revision,
                        &spec.kind,
                    ],
                )
                .await
        }
    }
    .map_err(unavailable)?;
    if inserted != 1 {
        return Err(match spec.missing_admission {
            MissingAdmission::InvalidChallenge => ProjectCommandError::InvalidChallenge,
            MissingAdmission::BindingConflict => ProjectCommandError::BindingConflict,
            MissingAdmission::InvalidWriter => ProjectCommandError::WriterIneligible,
        });
    }
    Ok(())
}
