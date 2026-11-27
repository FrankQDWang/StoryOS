//! The Author Command Admission row of the sequence, for project, AgentRun, and editor commands.

use crate::statement_cache::Client;
use storyos_application::{ProjectCommandEnvelope, ProjectCommandError};
use storyos_contracts::CommandKind;

use super::{Admission, DirectEditorAdmission, EditorAdmission, ProjectActionClass, unavailable};

/// Inserts the Admission after the consumed Command Challenge. Returns `false` when the insert
/// writes no row.
pub(super) async fn insert_admission(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    command_kind: CommandKind,
    admission: &Admission,
) -> Result<bool, ProjectCommandError> {
    let command_kind = command_kind.as_str();
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
                editor_session_id, client_session_binding_ref,
                client_session_generation, client_contract_revision, security_policy_revision,
                action_class, method, route_template, command_schema, command_kind,
                canonical_command_digest, idempotency_key, challenge_consumed_at,
                challenge_expires_at, correlation_id, chapter_object_id,
                expected_authoritative_revision_id, expected_proposal_head_revision_ids,
                target_refs, observed_ownership_partition, editor_contract_revision,
                undo_group_id, completed_intent_record_id, local_intent_sequence, command_payload)
             SELECT $1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                    NULL, $5, $6::text::numeric, $7, $8,
                    $17::text, $9, $10, $11, $16::text,
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
                        &command_kind,
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
        }) => {
            client
                .execute(
                    "INSERT INTO storyos.author_command_admissions
               (owner_user_id, project_id, author_command_admission_id, command_id,
                editor_session_id, client_session_binding_ref,
                client_session_generation, client_contract_revision, security_policy_revision,
                action_class, method, route_template, command_schema, command_kind,
                canonical_command_digest, idempotency_key, challenge_consumed_at,
                challenge_expires_at, correlation_id, chapter_object_id,
                expected_authoritative_revision_id, expected_proposal_head_revision_ids,
                target_refs, observed_ownership_partition, editor_contract_revision,
                undo_group_id, completed_intent_record_id, local_intent_sequence, command_payload)
             SELECT $1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                    session.editor_session_id,
                    session.client_session_binding_ref, $6::text::numeric,
                    session.client_contract_revision, session.security_policy_revision,
                    'explicit_editor_command', $7, $8, $9, $19::text,
                    $10, $11::text::uuid, challenge.consumed_at, challenge.expires_at,
                    $12::text::uuid, $13::text::uuid, $14::text::uuid, '{}'::uuid[], $20::text[],
                    NULL, session.client_contract_revision, NULL, NULL, NULL,
                    convert_from($15::bytea, 'UTF8')::jsonb
               FROM storyos.editor_sessions AS session
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
                AND challenge.consumed_at IS NOT NULL",
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
                        &command_kind,
                        target_refs,
                    ],
                )
                .await
        }
        Admission::DirectEditorAction(DirectEditorAdmission {
            editor_session_id,
            chapter_object_id,
            expected_authoritative_revision_id,
            expected_proposal_head_revision_ids,
            target_refs,
            observed_ownership_partition,
            editor_contract_revision,
            undo_group_id,
            completed_intent_record_id,
            local_intent_sequence,
        }) => {
            client
                .execute(
                    "INSERT INTO storyos.author_command_admissions
                       (owner_user_id, project_id, author_command_admission_id, command_id,
                        editor_session_id, client_session_binding_ref,
                        client_session_generation, client_contract_revision,
                        security_policy_revision, action_class, method, route_template,
                        command_schema, command_kind, canonical_command_digest, idempotency_key,
                        challenge_consumed_at, challenge_expires_at, correlation_id,
                        chapter_object_id, expected_authoritative_revision_id,
                        expected_proposal_head_revision_ids, target_refs,
                        observed_ownership_partition, editor_contract_revision, undo_group_id,
                        completed_intent_record_id, local_intent_sequence, command_payload)
                     SELECT $1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                            session.editor_session_id,
                            session.client_session_binding_ref, $6::text::numeric,
                            session.client_contract_revision, session.security_policy_revision,
                            'direct_editor_action', $7, $8, $9, $26::text, $10,
                            $11::text::uuid, challenge.consumed_at, challenge.expires_at,
                            $24::text::uuid, $12::text::uuid, $13::text::uuid,
                            $14::text[]::uuid[], $15::text[], $16, $17, $18::text::uuid,
                            $19::text::uuid, $20::text::numeric,
                            convert_from($25::bytea, 'UTF8')::jsonb
                       FROM storyos.editor_sessions AS session
                       JOIN storyos.authoritative_revisions AS expected_revision
                         ON (expected_revision.owner_user_id, expected_revision.project_id,
                             expected_revision.manuscript_object_id,
                             expected_revision.revision_id) =
                            (session.owner_user_id, session.project_id,
                             $12::text::uuid, $13::text::uuid)
                       JOIN storyos.authoritative_heads AS head
                         ON (head.owner_user_id, head.project_id,
                             head.manuscript_object_id) =
                            (session.owner_user_id, session.project_id, $12::text::uuid)
                       JOIN storyos.project_command_challenges AS challenge
                         ON (challenge.owner_user_id, challenge.project_id,
                             challenge.command_kind, challenge.idempotency_key) =
                            (session.owner_user_id, session.project_id,
                             $26, $11::text::uuid)
                      WHERE session.owner_user_id = $1::text::uuid
                        AND session.project_id = $2::text::uuid
                        AND session.editor_session_id = $5::text::uuid
                        AND session.client_session_binding_ref = $21
                        AND session.client_session_generation = $6::text::numeric
                        AND session.client_contract_revision = $22
                        AND session.security_policy_revision = $23
                        AND challenge.consumed_at IS NOT NULL
                        AND EXISTS (
                          SELECT 1 FROM storyos.projects AS project
                           WHERE project.owner_user_id = session.owner_user_id
                             AND project.project_id = session.project_id
                        )",
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
                        chapter_object_id,
                        expected_authoritative_revision_id,
                        expected_proposal_head_revision_ids,
                        target_refs,
                        observed_ownership_partition,
                        editor_contract_revision,
                        undo_group_id,
                        completed_intent_record_id,
                        &local_intent_sequence.to_string(),
                        &binding.binding_ref,
                        &binding.client_contract_revision,
                        &binding.security_policy_revision,
                        &envelope.correlation_id,
                        &command_bytes,
                        &command_kind,
                    ],
                )
                .await
        }
    }
    .map_err(unavailable)?;
    Ok(inserted == 1)
}
