use storyos_application::{
    ApplyAuthorEditCommand, AuthorEditError, AuthorEditSettlement, AuthorEditSettlementEffect,
    ProjectCommandChallengeError, ProjectCommandEnvelope,
};
use storyos_core::{
    ApplyAuthorEdit, ApplyAuthorEditOutcome, ApplyVersionedAuthorEdit, AuthorEditApplied,
    AuthorEditPrimitive, AuthorEditRefused, BlockReservation, COORDINATE_VERSION,
    MANUSCRIPT_SCHEMA_VERSION, ManuscriptBlock, ManuscriptPayload, TransitionOutcome,
    VersionedTargetOwnership, apply_author_edit as apply_core_author_edit,
    apply_versioned_author_edit, chapter_display_body,
};

use super::*;

use crate::command_sequence::{SettledCommand, admit_and_settle, settle_admitted_command};

mod compensation;
mod profile;
mod replay;
mod sequence;
pub(crate) use compensation::{
    ObservedProseFrontier, ProseCompensation, compensate_revision, decode_revision_compensation,
    load_revision_evidence,
};
pub(crate) use profile::AuthorEditProfileApplied;
pub(crate) use sequence::{AuthorEdit, AuthorEditFailure};

#[derive(Debug)]
pub(crate) struct ClassifiedAuthorEdit {
    pub result: ApplyAuthorEditOutcome,
    pub successor_blocks: Option<Vec<ManuscriptBlock>>,
    pub proposal_context: Option<super::author_edit_proposal::ProposalEditContext>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AuthorEditFault {
    None,
    AfterAdmissionBeforeCore,
    CoreBeforeCommit,
    CoreAfterCommitBeforeAcknowledgement,
}

impl PostgresProjectReader {
    /// Settles one Author Edit through the admit step and the settle step (ADR 0044).
    pub async fn apply_author_edit(
        &self,
        command: &ApplyAuthorEditCommand,
    ) -> Result<AuthorEditSettlement, AuthorEditError> {
        self.apply_author_edit_with_fault(command, AuthorEditFault::None)
            .await
    }

    async fn pause_generating_proposals_for_input(
        &self,
        command: &ApplyAuthorEditCommand,
    ) -> Result<(), AuthorEditError> {
        let transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(author_edit_challenge_error)?;
        super::stream_proposal_pause::pause_generating_proposals(&transaction.client, command)
            .await?;
        transaction
            .commit()
            .await
            .map_err(author_edit_challenge_error)?;
        Ok(())
    }

    pub(crate) async fn apply_author_edit_with_fault(
        &self,
        command: &ApplyAuthorEditCommand,
        fault: AuthorEditFault,
    ) -> Result<AuthorEditSettlement, AuthorEditError> {
        if !command
            .author_edit_units
            .iter()
            .any(|unit| unit.selection_snapshot.ordered_selection.is_some())
        {
            self.pause_generating_proposals_for_input(command).await?;
        }
        let envelope = author_edit_envelope(command);
        let author_edit = AuthorEdit::new(command, fault);
        let settled = admit_and_settle(self, &envelope, &author_edit, || {
            if fault == AuthorEditFault::AfterAdmissionBeforeCore {
                return Err(AuthorEditFailure(fault_error("CFP-ADMISSION-BEFORE-CORE")));
            }
            Ok(())
        })
        .await
        .map_err(|AuthorEditFailure(error)| error)?;
        acknowledged(settled, fault)
    }

    /// Settles one admitted Author Edit through the settle step.
    pub(crate) async fn complete_admitted_author_edit(
        &self,
        command: &ApplyAuthorEditCommand,
        fault: AuthorEditFault,
    ) -> Result<AuthorEditSettlement, AuthorEditError> {
        let envelope = author_edit_envelope(command);
        let settled = settle_admitted_command(self, &envelope, &AuthorEdit::new(command, fault))
            .await
            .map_err(|AuthorEditFailure(error)| error)?;
        acknowledged(settled, fault)
    }
}

/// The command envelope of one Author Edit.
pub(crate) fn author_edit_envelope(command: &ApplyAuthorEditCommand) -> ProjectCommandEnvelope {
    ProjectCommandEnvelope {
        project_scope: command.project_scope.clone(),
        client_binding: command.client_binding.clone(),
        challenge_binding: command.challenge_binding.clone(),
        nonce_digest: command.nonce_digest.clone(),
        canonical_command_bytes: command.canonical_command_bytes.clone(),
        correlation_id: command.correlation_id.clone(),
        ids: command.ids.clone(),
    }
}

/// The acknowledgement of one settled Author Edit, or the fault after its commit.
fn acknowledged(
    settled: SettledCommand<AuthorEdit<'_>>,
    fault: AuthorEditFault,
) -> Result<AuthorEditSettlement, AuthorEditError> {
    if fault == AuthorEditFault::CoreAfterCommitBeforeAcknowledgement {
        return Err(fault_error("CFP-CORE-AFTER-COMMIT-BEFORE-ACK"));
    }
    author_edit_settlement(settled)
}

/// The application settlement of one settled or replayed Author Edit.
pub(crate) fn author_edit_settlement(
    settled: SettledCommand<AuthorEdit<'_>>,
) -> Result<AuthorEditSettlement, AuthorEditError> {
    let missing = || {
        AuthorEditError::Unavailable(Box::new(std::io::Error::other(
            "the Author Edit acknowledgement is incomplete",
        )))
    };
    let zero = settled.zero_authority_effect;
    let (effect, record) = match settled.outcome {
        TransitionOutcome::Applied(AuthorEditProfileApplied::Revision(applied)) => (
            AuthorEditSettlementEffect::AuthoritativeApplied {
                ids: applied.ids,
                body: applied.body,
                blocks: applied.blocks,
                author_action_sequence: applied.author_action_sequence,
                project_activity_position: applied.project_activity_position,
            },
            applied.effect.record,
        ),
        TransitionOutcome::Applied(AuthorEditProfileApplied::Proposal(applied)) => (
            AuthorEditSettlementEffect::ProposalRevised {
                proposal_revision_id: applied.effect.proposal_revision_id.ok_or_else(missing)?,
                author_action_sequence: applied.author_action_sequence,
            },
            applied.effect.record,
        ),
        TransitionOutcome::NoEffect(reason) => (
            AuthorEditSettlementEffect::NoEffect { reason },
            zero.ok_or_else(missing)?.record,
        ),
        TransitionOutcome::Conflicted(reason) => {
            let zero = zero.ok_or_else(missing)?;
            (
                AuthorEditSettlementEffect::Conflicted {
                    reason,
                    current_authoritative_revision_id: zero.current_authoritative_revision_id,
                },
                zero.record,
            )
        }
        TransitionOutcome::Refused(AuthorEditRefused::RefusedToDraft) => {
            let zero = zero.ok_or_else(missing)?;
            (
                AuthorEditSettlementEffect::RefusedToDraft {
                    identity: zero.draft.ok_or_else(missing)?,
                },
                zero.record,
            )
        }
        TransitionOutcome::Refused(AuthorEditRefused::Refused(reason)) => (
            AuthorEditSettlementEffect::Refused { reason },
            zero.ok_or_else(missing)?.record,
        ),
    };
    Ok(AuthorEditSettlement {
        ids: settled.ids,
        effect,
        source_draft_disposition: record.source_draft_disposition,
        replacement_provenance: record.replacement_provenance,
        receipt_created_at: settled.receipt_created_at,
        completed_intent_record_id: record.completed_intent_record_id,
        local_intent_sequence: record.local_intent_sequence,
    })
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
    let loaded = super::author_edit_proposal::load_chapter_proposal_heads(
        client,
        command,
        current_body.clone(),
    )
    .await?;
    let current_ownership = loaded.ownership.clone();
    if command
        .author_edit_units
        .iter()
        .any(|unit| unit.selection_snapshot.ordered_selection.is_some())
    {
        let facts = super::refused_edit_source::load_sources(
            client,
            command,
            current_revision_id,
            &current_body,
        )
        .await?;
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
                ordered_source_facts: Some(facts),
                target_refs: command.target_refs.clone(),
                observed_ownership_partition: command.observed_ownership_partition.clone(),
                inline_edit_disposition: storyos_core::InlineEditDisposition::Unspecified,
                author_edit_units: command.author_edit_units.clone(),
            }),
            successor_blocks: None,
            proposal_context: None,
        });
    }
    if !uses_versioned_payload
        && loaded.context.is_none()
        && !current_ownership.proposal_head_revision_ids.is_empty()
    {
        let result = if command.expected_proposal_head_revision_ids
            != current_ownership.proposal_head_revision_ids
        {
            TransitionOutcome::Conflicted(storyos_core::AuthorEditConflict::ProposalHeadPresent)
        } else {
            TransitionOutcome::Refused(AuthorEditRefused::Refused(
                storyos_core::AuthorEditRefusal::TargetMismatch,
            ))
        };
        return Ok(ClassifiedAuthorEdit {
            result,
            successor_blocks: None,
            proposal_context: None,
        });
    }
    if !uses_versioned_payload {
        let routed = match super::author_edit_proposal::route_inline_author_edit(
            &loaded,
            current_body,
            command.author_edit_units.clone(),
        ) {
            Ok(routed) => routed,
            Err(reason) => {
                return Ok(ClassifiedAuthorEdit {
                    result: TransitionOutcome::Refused(AuthorEditRefused::Refused(reason)),
                    successor_blocks: None,
                    proposal_context: None,
                });
            }
        };
        return Ok(ClassifiedAuthorEdit {
            result: apply_core_author_edit(&ApplyAuthorEdit {
                chapter_id: command.chapter_id.clone(),
                current_authoritative_revision_id: current_revision_id.to_owned(),
                current_body: routed.current_body,
                expected_authoritative_revision_id: command
                    .expected_authoritative_revision_id
                    .clone(),
                expected_proposal_head_revision_ids: command
                    .expected_proposal_head_revision_ids
                    .clone(),
                current_ownership,
                ordered_source_facts: None,
                target_refs: command.target_refs.clone(),
                observed_ownership_partition: command.observed_ownership_partition.clone(),
                inline_edit_disposition: routed.disposition,
                author_edit_units: routed.author_edit_units,
            }),
            successor_blocks: None,
            proposal_context: routed.proposal_context,
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
    let mut current_target_ownership = super::author_edit_inline::load_inline_target_ownership(
        client,
        command,
        loaded.context.as_ref(),
        current_revision_id,
    )
    .await?
    .unwrap_or(VersionedTargetOwnership::Chapter);
    if command.retry_source.is_some()
        && let [unit] = command.author_edit_units.as_slice()
        && let [
            AuthorEditPrimitive::ReplaceBlockSelection {
                manuscript_block_id,
                ..
            },
        ] = unit.normalized_primitives.as_slice()
        && blocks
            .iter()
            .any(|block| block.manuscript_block_id == *manuscript_block_id)
    {
        let reserved: bool = client
            .query_one(
                "SELECT EXISTS (SELECT 1 FROM storyos.proposal_operations AS reservation
               WHERE reservation.owner_user_id=$1::text::uuid
                 AND reservation.project_id=$2::text::uuid
                 AND reservation.manuscript_block_id=$3::text::uuid
                 AND reservation.reservation_state='unresolved')",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &manuscript_block_id,
                ],
            )
            .await
            .map_err(author_edit_database_error)?
            .get(0);
        current_target_ownership = VersionedTargetOwnership::Block {
            manuscript_block_id: manuscript_block_id.clone(),
            reservation: if reserved {
                BlockReservation::Present
            } else {
                BlockReservation::Absent
            },
        };
    }
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
            current_target_ownership,
            target_refs: command.target_refs.clone(),
            observed_ownership_partition: command.observed_ownership_partition.clone(),
            author_edit_units: command.author_edit_units.clone(),
        }) {
            TransitionOutcome::Applied(payload) => ClassifiedAuthorEdit {
                result: TransitionOutcome::Applied(AuthorEditApplied::AuthoritativeApplied {
                    body: chapter_display_body(&payload.blocks),
                }),
                successor_blocks: Some(payload.blocks),
                proposal_context: None,
            },
            TransitionOutcome::Conflicted(reason) => ClassifiedAuthorEdit {
                result: TransitionOutcome::Conflicted(reason),
                successor_blocks: None,
                proposal_context: None,
            },
            TransitionOutcome::NoEffect(reason) => ClassifiedAuthorEdit {
                result: TransitionOutcome::NoEffect(reason),
                successor_blocks: None,
                proposal_context: None,
            },
            TransitionOutcome::Refused(reason) => ClassifiedAuthorEdit {
                result: TransitionOutcome::Refused(AuthorEditRefused::Refused(reason)),
                successor_blocks: None,
                proposal_context: None,
            },
        },
    )
}

#[cfg(test)]
#[path = "author_edit_tests.rs"]
pub(crate) mod tests;

#[cfg(test)]
#[path = "author_edit_counter_tests.rs"]
mod counter_tests;
