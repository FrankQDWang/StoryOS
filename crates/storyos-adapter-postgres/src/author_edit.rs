use storyos_application::{
    ApplyAuthorEditCommand, AuthorEditError, AuthorEditSettlement, AuthorEditStore,
    ProjectCommandChallengeError, ProjectCommandChallengeTransaction, ProjectCommandChallengeUse,
};
use storyos_core::{
    ApplyAuthorEdit, ApplyAuthorEditResult, ApplyVersionedAuthorEdit,
    ApplyVersionedAuthorEditResult, AuthorEditPrimitive, COORDINATE_VERSION, CurrentOwnershipFacts,
    MANUSCRIPT_SCHEMA_VERSION, ManuscriptBlock, ManuscriptPayload,
    apply_author_edit as apply_core_author_edit, apply_versioned_author_edit, chapter_display_body,
};

use super::*;

#[derive(Debug)]
pub(crate) struct ClassifiedAuthorEdit {
    pub result: ApplyAuthorEditResult,
    pub successor_blocks: Option<Vec<ManuscriptBlock>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AuthorEditFault {
    None,
    AfterAdmissionBeforeCore,
    CoreBeforeCommit,
    CoreAfterCommitBeforeAcknowledgement,
}

impl AuthorEditStore for PostgresProjectReader {
    async fn apply_author_edit(
        &self,
        command: &ApplyAuthorEditCommand,
    ) -> Result<AuthorEditSettlement, AuthorEditError> {
        self.apply_author_edit_with_fault(command, AuthorEditFault::None)
            .await
    }
}

impl PostgresProjectReader {
    pub(crate) async fn apply_author_edit_with_fault(
        &self,
        command: &ApplyAuthorEditCommand,
        fault: AuthorEditFault,
    ) -> Result<AuthorEditSettlement, AuthorEditError> {
        match self.create_author_command_admission(command).await? {
            AdmissionUse::ExistingSettlement(receipt_id) => {
                return self.read_author_edit_settlement(command, &receipt_id).await;
            }
            AdmissionUse::Created => {}
        }
        if fault == AuthorEditFault::AfterAdmissionBeforeCore {
            return Err(fault_error("CFP-ADMISSION-BEFORE-CORE"));
        }
        self.complete_admitted_author_edit(command, fault).await
    }

    pub(crate) async fn complete_admitted_author_edit(
        &self,
        command: &ApplyAuthorEditCommand,
        fault: AuthorEditFault,
    ) -> Result<AuthorEditSettlement, AuthorEditError> {
        let transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(author_edit_challenge_error)?;
        if let Some(receipt_id) = existing_receipt_id(&transaction.client, command).await? {
            transaction
                .rollback()
                .await
                .map_err(author_edit_challenge_error)?;
            return self.read_author_edit_settlement(command, &receipt_id).await;
        }
        let row = transaction
            .client
            .query_opt(
                "SELECT head.current_revision_id::text,
                        convert_from(payload.canonical_bytes, 'UTF8')
                   FROM storyos.author_command_admissions AS admission
                   JOIN storyos.project_writer_generations AS writer
                     ON (writer.owner_user_id, writer.project_id,
                         writer.current_editor_session_id, writer.writer_generation) =
                        (admission.owner_user_id, admission.project_id,
                         admission.editor_session_id, admission.writer_generation)
                   JOIN storyos.authoritative_heads AS head
                     ON (head.owner_user_id, head.project_id, head.manuscript_object_id) =
                        (admission.owner_user_id, admission.project_id,
                         admission.chapter_object_id)
                   JOIN storyos.authoritative_revisions AS revision
                     ON (revision.owner_user_id, revision.project_id,
                         revision.manuscript_object_id, revision.revision_id) =
                        (head.owner_user_id, head.project_id,
                         head.manuscript_object_id, head.current_revision_id)
                   JOIN storyos.authoritative_payloads AS payload
                     ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
                        (revision.owner_user_id, revision.project_id, revision.payload_id)
                  WHERE admission.owner_user_id = $1::text::uuid
                    AND admission.project_id = $2::text::uuid
                    AND admission.author_command_admission_id = $3::text::uuid
                    AND admission.challenge_expires_at > clock_timestamp()
                   AND NOT EXISTS (
                     SELECT 1 FROM storyos.author_command_admission_settlements AS settlement
                      WHERE (settlement.owner_user_id, settlement.project_id,
                             settlement.author_command_admission_id) =
                            (admission.owner_user_id, admission.project_id,
                             admission.author_command_admission_id)
                   )
                  FOR UPDATE OF head",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &command.ids.author_command_admission_id,
                ],
            )
            .await
            .map_err(author_edit_database_error)?;
        let Some(row) = row else {
            let expired = transaction
                .client
                .query_one(
                    "SELECT EXISTS (
                   SELECT 1 FROM storyos.author_command_admissions
                    WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                      AND author_command_admission_id = $3::text::uuid
                      AND challenge_expires_at <= clock_timestamp())",
                    &[
                        &command.project_scope.owner_user_id.as_ref(),
                        &command.project_scope.project_id.as_ref(),
                        &command.ids.author_command_admission_id,
                    ],
                )
                .await
                .map_err(author_edit_database_error)?
                .get::<_, bool>(0);
            let settled = existing_receipt_id(&transaction.client, command).await?;
            transaction
                .rollback()
                .await
                .map_err(author_edit_challenge_error)?;
            if let Some(receipt_id) = settled {
                return self.read_author_edit_settlement(command, &receipt_id).await;
            }
            return Err(if expired {
                AuthorEditError::AdmissionExpired
            } else {
                AuthorEditError::BindingConflict
            });
        };
        let current_revision_id = row.get::<_, String>(0);
        let current_body = row.get::<_, String>(1);
        let classified = classify_author_edit(
            &transaction.client,
            command,
            &current_revision_id,
            current_body,
        )
        .await?;
        let settlement = match super::author_edit_settlement::persist_author_edit_settlement(
            &transaction.client,
            command,
            &current_revision_id,
            classified,
        )
        .await
        {
            Ok(settlement) => settlement,
            Err(error) => {
                transaction
                    .rollback()
                    .await
                    .map_err(author_edit_challenge_error)?;
                let client = self
                    .connect_challenge()
                    .await
                    .map_err(author_edit_challenge_error)?;
                set_challenge_scope_on_client(&client, &command.project_scope)
                    .await
                    .map_err(author_edit_challenge_error)?;
                if let Some(receipt_id) = existing_receipt_id(&client, command).await? {
                    return self.read_author_edit_settlement(command, &receipt_id).await;
                }
                return Err(error);
            }
        };
        if fault == AuthorEditFault::CoreBeforeCommit {
            transaction
                .rollback()
                .await
                .map_err(author_edit_challenge_error)?;
            return Err(fault_error("CFP-CORE-BEFORE-COMMIT"));
        }
        transaction
            .commit()
            .await
            .map_err(author_edit_challenge_error)?;
        if fault == AuthorEditFault::CoreAfterCommitBeforeAcknowledgement {
            return Err(fault_error("CFP-CORE-AFTER-COMMIT-BEFORE-ACK"));
        }
        Ok(settlement)
    }

    async fn create_author_command_admission(
        &self,
        command: &ApplyAuthorEditCommand,
    ) -> Result<AdmissionUse, AuthorEditError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(author_edit_challenge_error)?;
        let challenge_use = transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(author_edit_challenge_error)?;
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction
                    .rollback()
                    .await
                    .map_err(author_edit_challenge_error)?;
                if result_reference == "requires_reconfirmation" {
                    return Err(AuthorEditError::BindingConflict);
                }
                Ok(AdmissionUse::ExistingSettlement(result_reference))
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(author_edit_challenge_error)?;
                Err(AuthorEditError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                let writer_generation = command.writer_generation.to_string();
                let client_session_generation =
                    command.client_binding.session_generation.to_string();
                let local_intent_sequence = command.local_intent_sequence.to_string();
                let inserted = transaction
                    .client
                    .execute(
                        "INSERT INTO storyos.author_command_admissions
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
                            session.editor_session_id, $6::text::numeric,
                            session.client_session_binding_ref, $7::text::numeric,
                            session.client_contract_revision, session.security_policy_revision,
                            'direct_editor_action', $8, $9, $10, 'applyAuthorEdit', $11,
                            $12::text::uuid, challenge.consumed_at, challenge.expires_at,
                            $25::text::uuid, $13::text::uuid, $14::text::uuid,
                            $15::text[]::uuid[], $16::text[], $17, $18, $19::text::uuid,
                            $20::text::uuid, $21::text::numeric,
                            convert_from($26::bytea, 'UTF8')::jsonb
                       FROM storyos.editor_sessions AS session
                       JOIN storyos.project_writer_generations AS writer
                         ON (writer.owner_user_id, writer.project_id,
                             writer.current_editor_session_id, writer.writer_generation) =
                            (session.owner_user_id, session.project_id,
                             session.editor_session_id, $6::text::numeric)
                        AND writer.writer_generation = (
                          SELECT max(current_writer.writer_generation)
                            FROM storyos.project_writer_generations AS current_writer
                           WHERE current_writer.owner_user_id = session.owner_user_id
                             AND current_writer.project_id = session.project_id
                        )
                       JOIN storyos.authoritative_revisions AS expected_revision
                         ON (expected_revision.owner_user_id, expected_revision.project_id,
                             expected_revision.manuscript_object_id,
                             expected_revision.revision_id) =
                            (session.owner_user_id, session.project_id,
                             $13::text::uuid, $14::text::uuid)
                       JOIN storyos.authoritative_heads AS head
                         ON (head.owner_user_id, head.project_id,
                             head.manuscript_object_id) =
                            (session.owner_user_id, session.project_id, $13::text::uuid)
                       JOIN storyos.project_command_challenges AS challenge
                         ON (challenge.owner_user_id, challenge.project_id,
                             challenge.command_kind, challenge.idempotency_key) =
                            (session.owner_user_id, session.project_id,
                             'applyAuthorEdit', $12::text::uuid)
                      WHERE session.owner_user_id = $1::text::uuid
                        AND session.project_id = $2::text::uuid
                        AND session.editor_session_id = $5::text::uuid
                        AND session.client_session_binding_ref = $22
                        AND session.client_session_generation = $7::text::numeric
                        AND session.client_contract_revision = $23
                        AND session.security_policy_revision = $24
                        AND challenge.consumed_at IS NOT NULL
                        AND EXISTS (
                          SELECT 1 FROM storyos.projects AS project
                           WHERE project.owner_user_id = session.owner_user_id
                             AND project.project_id = session.project_id
                             AND project.lifecycle_state = 'active'
                        )",
                        &[
                            &command.project_scope.owner_user_id.as_ref(),
                            &command.project_scope.project_id.as_ref(),
                            &command.ids.author_command_admission_id,
                            &command.ids.command_id,
                            &command.editor_session_id.as_ref(),
                            &writer_generation,
                            &client_session_generation,
                            &command.challenge_binding.method,
                            &command.challenge_binding.route_template,
                            &command.challenge_binding.command_schema,
                            &command.challenge_binding.canonical_command_digest,
                            &command.challenge_binding.idempotency_key,
                            &command.chapter_id,
                            &command.expected_authoritative_revision_id,
                            &command.expected_proposal_head_revision_ids,
                            &command.target_refs,
                            &command.observed_ownership_partition,
                            &command.editor_contract_revision,
                            &command.undo_group_id,
                            &command.completed_intent_record_id,
                            &local_intent_sequence,
                            &command.client_binding.binding_ref,
                            &command.client_binding.client_contract_revision,
                            &command.client_binding.security_policy_revision,
                            &command.correlation_id,
                            &command.canonical_command_bytes.as_slice(),
                        ],
                    )
                    .await
                    .map_err(author_edit_database_error)?;
                if inserted != 1 {
                    let stale_writer = transaction
                        .client
                        .query_one(
                            "SELECT EXISTS (
                           SELECT 1 FROM storyos.editor_sessions AS session
                            WHERE session.owner_user_id = $1::text::uuid
                              AND session.project_id = $2::text::uuid
                              AND session.editor_session_id = $3::text::uuid)
                          AND NOT EXISTS (
                           SELECT 1 FROM storyos.project_writer_generations AS writer
                            WHERE writer.owner_user_id = $1::text::uuid
                              AND writer.project_id = $2::text::uuid
                              AND writer.current_editor_session_id = $3::text::uuid
                              AND writer.writer_generation = $4::text::numeric
                              AND writer.writer_generation = (
                                SELECT max(current_writer.writer_generation)
                                  FROM storyos.project_writer_generations AS current_writer
                                 WHERE current_writer.owner_user_id = $1::text::uuid
                                   AND current_writer.project_id = $2::text::uuid
                              ))",
                            &[
                                &command.project_scope.owner_user_id.as_ref(),
                                &command.project_scope.project_id.as_ref(),
                                &command.editor_session_id.as_ref(),
                                &writer_generation,
                            ],
                        )
                        .await
                        .map_err(author_edit_database_error)?
                        .get::<_, bool>(0);
                    transaction
                        .rollback()
                        .await
                        .map_err(author_edit_challenge_error)?;
                    return Err(if stale_writer {
                        AuthorEditError::StaleWriter
                    } else {
                        AuthorEditError::BindingConflict
                    });
                }
                transaction
                    .commit()
                    .await
                    .map_err(author_edit_challenge_error)?;
                Ok(AdmissionUse::Created)
            }
        }
    }
}

enum AdmissionUse {
    Created,
    ExistingSettlement(String),
}

async fn existing_receipt_id(
    client: &tokio_postgres::Client,
    command: &ApplyAuthorEditCommand,
) -> Result<Option<String>, AuthorEditError> {
    client
        .query_opt(
            "SELECT settlement.receipt_id::text
               FROM storyos.author_command_admission_settlements AS settlement
              WHERE settlement.owner_user_id = $1::text::uuid
                AND settlement.project_id = $2::text::uuid
                AND settlement.author_command_admission_id = $3::text::uuid
                AND settlement.settlement_kind = 'receipt_settled'",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.author_command_admission_id,
            ],
        )
        .await
        .map_err(author_edit_database_error)
        .map(|row| row.map(|row| row.get(0)))
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut encoded, byte| {
            use std::fmt::Write as _;
            write!(encoded, "{byte:02x}").expect("writing to String cannot fail");
            encoded
        })
}

pub(super) fn parse_u64(value: String) -> Result<u64, AuthorEditError> {
    value
        .parse::<u64>()
        .map_err(|error| AuthorEditError::Unavailable(Box::new(error)))
}

pub(super) fn author_edit_challenge_error(error: ProjectCommandChallengeError) -> AuthorEditError {
    match error {
        ProjectCommandChallengeError::BindingConflict => AuthorEditError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired
        | ProjectCommandChallengeError::RateLimited { .. } => AuthorEditError::InvalidChallenge,
        ProjectCommandChallengeError::Unavailable(source) => AuthorEditError::Unavailable(source),
    }
}

pub(super) fn author_edit_database_error(error: tokio_postgres::Error) -> AuthorEditError {
    AuthorEditError::Unavailable(Box::new(error))
}

fn fault_error(point: &'static str) -> AuthorEditError {
    AuthorEditError::Unavailable(Box::new(std::io::Error::other(point)))
}

async fn classify_author_edit(
    client: &tokio_postgres::Client,
    command: &ApplyAuthorEditCommand,
    current_revision_id: &str,
    current_body: String,
) -> Result<ClassifiedAuthorEdit, AuthorEditError> {
    let uses_versioned_payload = command.author_edit_units.iter().any(|unit| {
        unit.normalized_primitives.iter().any(|primitive| {
            matches!(
                primitive,
                AuthorEditPrimitive::ReplaceBlockSelection { .. }
                    | AuthorEditPrimitive::SplitBlock { .. }
                    | AuthorEditPrimitive::JoinBlocks { .. }
                    | AuthorEditPrimitive::MoveBlock { .. }
                    | AuthorEditPrimitive::RetypeBlock { .. }
            )
        })
    });
    let current_ownership = CurrentOwnershipFacts {
        proposal_head_revision_ids: Vec::new(),
        anchor_refs: Vec::new(),
        unresolved_reservation_refs: Vec::new(),
    };
    if !uses_versioned_payload {
        return Ok(ClassifiedAuthorEdit {
            result: apply_core_author_edit(&ApplyAuthorEdit {
                chapter_id: command.chapter_id.clone(),
                current_authoritative_revision_id: current_revision_id.to_owned(),
                current_body,
                expected_authoritative_revision_id: command
                    .expected_authoritative_revision_id
                    .clone(),
                expected_proposal_head_revision_ids: command
                    .expected_proposal_head_revision_ids
                    .clone(),
                current_ownership,
                target_refs: command.target_refs.clone(),
                observed_ownership_partition: command.observed_ownership_partition.clone(),
                author_edit_units: command.author_edit_units.clone(),
            }),
            successor_blocks: None,
        });
    }
    let blocks = crate::manuscript_block::load_or_upgrade_blocks(
        client,
        command.project_scope.owner_user_id.as_ref(),
        command.project_scope.project_id.as_ref(),
        &command.chapter_id,
        current_revision_id,
        &current_body,
    )
    .await
    .map_err(author_edit_database_error)?;
    Ok(
        match apply_versioned_author_edit(&ApplyVersionedAuthorEdit {
            chapter_id: command.chapter_id.clone(),
            current_authoritative_revision_id: current_revision_id.to_owned(),
            current_payload: ManuscriptPayload {
                schema_version: MANUSCRIPT_SCHEMA_VERSION,
                coordinate_version: COORDINATE_VERSION,
                blocks,
            },
            expected_authoritative_revision_id: command.expected_authoritative_revision_id.clone(),
            expected_proposal_head_revision_ids: command
                .expected_proposal_head_revision_ids
                .clone(),
            current_ownership,
            target_refs: command.target_refs.clone(),
            observed_ownership_partition: command.observed_ownership_partition.clone(),
            author_edit_units: command.author_edit_units.clone(),
        }) {
            ApplyVersionedAuthorEditResult::AuthoritativeApplied { payload } => {
                ClassifiedAuthorEdit {
                    result: ApplyAuthorEditResult::AuthoritativeApplied {
                        body: chapter_display_body(&payload.blocks),
                    },
                    successor_blocks: Some(payload.blocks),
                }
            }
            ApplyVersionedAuthorEditResult::Conflicted { reason } => ClassifiedAuthorEdit {
                result: ApplyAuthorEditResult::Conflicted { reason },
                successor_blocks: None,
            },
            ApplyVersionedAuthorEditResult::NoEffect { reason } => ClassifiedAuthorEdit {
                result: ApplyAuthorEditResult::NoEffect { reason },
                successor_blocks: None,
            },
            ApplyVersionedAuthorEditResult::Refused { reason } => ClassifiedAuthorEdit {
                result: ApplyAuthorEditResult::Refused { reason },
                successor_blocks: None,
            },
        },
    )
}

#[cfg(test)]
#[path = "author_edit_tests.rs"]
pub(crate) mod tests;

#[cfg(test)]
#[path = "author_command_outcome_unknown_tests.rs"]
mod outcome_unknown_tests;

#[cfg(test)]
#[path = "author_edit_counter_tests.rs"]
mod counter_tests;
