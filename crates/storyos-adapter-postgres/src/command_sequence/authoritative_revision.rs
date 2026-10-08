//! The `AuthoritativeRevision` settlement profile: one new Authoritative Revision of a Chapter
//! with its Commit, Author Action, Activity record, writer base, and canonical Snapshot
//! (ADR 0044).

use storyos_application::{
    AuthorCommandAdmissionIds, AuthoritativeAppliedIds, ProjectCommandEnvelope,
    ProjectCommandError, ProjectScope, RevisionApplied,
};
use tokio_postgres::Client;
use uuid::Uuid;

use super::{
    AppliedVariant, CommandSpec, LockedProject, SelectsRecords, SettlementProfile, unavailable,
};
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::manuscript_block::{
    blocks_from_stored_payload, copy_or_upgrade_revision_members, display_body_from_stored,
    load_revision_blocks, persist_revision_members_from_blocks,
};

/// The scope sequences and identities that one new Authoritative Revision uses.
pub(crate) struct RevisionSequences {
    pub(crate) author_action_sequence: u64,
    authoritative_commit_sequence: u64,
    pub(crate) project_activity_position: u64,
    pub(crate) ids: AuthoritativeAppliedIds,
}

/// The Author Action disposition of one applied outcome (ADR 0044).
pub(crate) enum ActionDisposition {
    Forward,
    /// The Compensation of the Forward Author Action at `source_sequence`.
    Compensation {
        source_sequence: u64,
    },
}

/// The move of the writer base Snapshot of the Editor Session to the new Revision.
pub(crate) enum RevisionBase {
    /// Moves the base when it is on the prior Revision. Another base stays.
    WhenOnPrior,
    /// Moves the base. A base that is not on the prior Revision is a binding conflict.
    Required,
}

/// The Manuscript Blocks of the new Revision, in order.
pub(crate) enum RevisionMembers {
    /// The members of this earlier Revision of the Chapter.
    CopyFrom(String),
    /// These Blocks of a versioned edit.
    Blocks(Vec<storyos_core::ManuscriptBlock>),
}

/// The applied writes of one command of the profile. The profile writes every authority record.
pub(crate) struct RevisionWrite<E> {
    pub(crate) effect: E,
    pub(crate) chapter_id: String,
    /// The current head that the new Revision replaces: its parent, and the guard of the Head and
    /// writer base updates.
    pub(crate) prior_revision_id: String,
    /// The canonical bytes of the new Revision payload.
    pub(crate) payload: String,
    pub(crate) members: RevisionMembers,
    pub(crate) disposition: ActionDisposition,
    /// The writer Editor Session whose base Snapshot moves to the new Revision.
    pub(crate) editor_session_id: String,
    pub(crate) writer_base: RevisionBase,
}

pub(crate) struct AuthoritativeRevision;

impl<T> SelectsRecords<AuthoritativeRevision> for T {
    fn selector(&self) {}
}

impl SettlementProfile for AuthoritativeRevision {
    type Selector = ();
    type Sequences = RevisionSequences;
    type Write<E: Send> = RevisionWrite<E>;
    type Applied<E: Send> = RevisionApplied<E>;

    async fn allocate(
        client: &Client,
        scope: &ProjectScope,
        (): &(),
    ) -> Result<RevisionSequences, ProjectCommandError> {
        let row = client
            .query_one(
                "INSERT INTO storyos.scope_counters AS counters
                   (owner_user_id, project_id, author_action_sequence,
                    authoritative_commit_sequence, project_activity_position)
                 VALUES ($1::text::uuid, $2::text::uuid, 1, 1, 1)
                 ON CONFLICT (owner_user_id, project_id)
                 DO UPDATE SET
                   author_action_sequence = counters.author_action_sequence + 1,
                   authoritative_commit_sequence = counters.authoritative_commit_sequence + 1,
                   project_activity_position = counters.project_activity_position + 1
                 RETURNING counters.author_action_sequence::text,
                           counters.authoritative_commit_sequence::text,
                           counters.project_activity_position::text",
                &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
            )
            .await
            .map_err(unavailable)?;
        let sequence = |index: usize| row.get::<_, String>(index).parse().map_err(unavailable);
        Ok(RevisionSequences {
            author_action_sequence: sequence(/*index*/ 0)?,
            authoritative_commit_sequence: sequence(/*index*/ 1)?,
            project_activity_position: sequence(/*index*/ 2)?,
            ids: AuthoritativeAppliedIds {
                revision_id: Uuid::now_v7().to_string(),
                payload_id: Uuid::now_v7().to_string(),
                authoritative_commit_id: Uuid::now_v7().to_string(),
                project_activity_event_id: Uuid::now_v7().to_string(),
            },
        })
    }

    fn commit_ids(sequences: &RevisionSequences) -> Vec<String> {
        vec![sequences.ids.authoritative_commit_id.clone()]
    }

    fn revision_ids(sequences: &RevisionSequences) -> Vec<String> {
        vec![sequences.ids.revision_id.clone()]
    }

    async fn persist<E: Send>(
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        _spec: &CommandSpec,
        _variant: AppliedVariant,
        sequences: RevisionSequences,
        write: RevisionWrite<E>,
    ) -> Result<RevisionApplied<E>, ProjectCommandError> {
        write_revision(
            client,
            &envelope.project_scope,
            &envelope.ids,
            sequences,
            write,
        )
        .await
    }

    fn replay<E: Send>(
        decode: impl FnOnce() -> Result<E, ReplayFault>,
        replay: &CommandReplay,
    ) -> Result<RevisionApplied<E>, ReplayFault> {
        let damaged = || ReplayFault::Unavailable("the Authoritative Revision is damaged".into());
        let revision = replay.revision.as_ref().ok_or_else(damaged)?;
        let activity = revision.activity.as_ref().ok_or_else(damaged)?;
        let author_action_sequence = replay
            .author_action_sequence
            .as_deref()
            .ok_or_else(damaged)?;
        if activity.author_action_sequence != author_action_sequence
            || activity.authoritative_commit_id != revision.authoritative_commit_id
            || activity.resulting_revision_id != revision.revision_id
        {
            return Err(damaged());
        }
        let blocks = blocks_from_stored_payload(&revision.payload, &revision.member_block_ids);
        Ok(RevisionApplied {
            effect: decode()?,
            ids: AuthoritativeAppliedIds {
                revision_id: revision.revision_id.clone(),
                payload_id: revision.payload_id.clone(),
                authoritative_commit_id: revision.authoritative_commit_id.clone(),
                project_activity_event_id: activity.project_activity_event_id.clone(),
            },
            author_action_sequence: author_action_sequence.parse().map_err(|_| damaged())?,
            project_activity_position: activity
                .project_activity_position
                .parse()
                .map_err(|_| damaged())?,
            body: display_body_from_stored(&revision.payload, &blocks),
            blocks,
        })
    }
}

/// Writes the new Revision and its authority records after the Domain Receipt.
pub(crate) async fn write_revision<E>(
    client: &Client,
    scope: &ProjectScope,
    ids: &AuthorCommandAdmissionIds,
    sequences: RevisionSequences,
    write: RevisionWrite<E>,
) -> Result<RevisionApplied<E>, ProjectCommandError> {
    let owner_user_id = scope.owner_user_id.as_ref();
    let project_id = scope.project_id.as_ref();
    let revision = &sequences.ids;
    client
        .execute(
            "INSERT INTO storyos.authoritative_payloads
               (owner_user_id, project_id, payload_id, canonical_bytes)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, convert_to($4, 'UTF8'))",
            &[
                &owner_user_id,
                &project_id,
                &revision.payload_id,
                &write.payload,
            ],
        )
        .await
        .map_err(unavailable)?;
    client
        .execute(
            "INSERT INTO storyos.authoritative_revisions
               (owner_user_id, project_id, manuscript_object_id, revision_id, payload_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid)",
            &[
                &owner_user_id,
                &project_id,
                &write.chapter_id,
                &revision.revision_id,
                &revision.payload_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    let copied = match &write.members {
        RevisionMembers::CopyFrom(source_revision_id) => {
            copy_or_upgrade_revision_members(
                client,
                owner_user_id,
                project_id,
                &write.chapter_id,
                source_revision_id,
                &revision.revision_id,
            )
            .await
        }
        RevisionMembers::Blocks(blocks) => {
            persist_revision_members_from_blocks(
                client,
                owner_user_id,
                project_id,
                &write.chapter_id,
                &revision.revision_id,
                blocks,
            )
            .await
        }
    }
    .map_err(unavailable)?;
    if copied == 0 {
        return Err(unavailable("the new revision members were not copied"));
    }
    client
        .execute(
            "INSERT INTO storyos.authoritative_revision_envelopes
               (owner_user_id, project_id, manuscript_object_id, revision_id, parent_revision_id,
                schema_revision, creator_kind, creator_ref, receipt_id, receipt_result_kind,
                cause_kind, payload_digest)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, 'storyos.authoritative-revision-envelope.v1',
                     'author_command_admission', $6::text::uuid, $7::text::uuid,
                     'authoritative_applied', 'direct_author_action', $8)",
            &[
                &owner_user_id,
                &project_id,
                &write.chapter_id,
                &revision.revision_id,
                &write.prior_revision_id,
                &ids.author_command_admission_id,
                &ids.receipt_id,
                &crate::author_edit::sha256_hex(write.payload.as_bytes()),
            ],
        )
        .await
        .map_err(unavailable)?;
    let head_updates = client
        .execute(
            "UPDATE storyos.authoritative_heads SET current_revision_id = $4::text::uuid
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND manuscript_object_id = $3::text::uuid
                AND current_revision_id = $5::text::uuid",
            &[
                &owner_user_id,
                &project_id,
                &write.chapter_id,
                &revision.revision_id,
                &write.prior_revision_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    if head_updates != 1 {
        return Err(ProjectCommandError::BindingConflict);
    }
    client
        .execute(
            "INSERT INTO storyos.authoritative_commits
               (owner_user_id, project_id, authoritative_commit_id, authoritative_commit_sequence,
                manuscript_object_id, prior_revision_id, resulting_revision_id,
                author_command_admission_id, receipt_id, receipt_result_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::numeric,
                     $5::text::uuid, $6::text::uuid, $7::text::uuid, $8::text::uuid,
                     $9::text::uuid, 'authoritative_applied')",
            &[
                &owner_user_id,
                &project_id,
                &revision.authoritative_commit_id,
                &sequences.authoritative_commit_sequence.to_string(),
                &write.chapter_id,
                &write.prior_revision_id,
                &revision.revision_id,
                &ids.author_command_admission_id,
                &ids.receipt_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    let author_action_sequence = sequences.author_action_sequence.to_string();
    let (disposition, source_sequence) = match write.disposition {
        ActionDisposition::Forward => ("forward", None),
        ActionDisposition::Compensation { source_sequence } => {
            ("compensation", Some(source_sequence.to_string()))
        }
    };
    client
        .execute(
            "INSERT INTO storyos.author_action_entries
               (owner_user_id, project_id, author_action_sequence, disposition,
                compensated_source_sequence, authoritative_commit_id, receipt_id,
                receipt_result_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4,
                     $5::text::numeric, $6::text::uuid, $7::text::uuid, 'authoritative_applied')",
            &[
                &owner_user_id,
                &project_id,
                &author_action_sequence,
                &disposition,
                &source_sequence,
                &revision.authoritative_commit_id,
                &ids.receipt_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    let position = sequences.project_activity_position.to_string();
    client
        .execute(
            "INSERT INTO storyos.project_activity_events
               (owner_user_id, project_id, project_activity_position,
                project_activity_event_id, event_kind, receipt_id,
                receipt_result_kind, authoritative_commit_id,
                resulting_revision_id, author_action_sequence)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric,
                     $4::text::uuid, 'authoritative_author_edit_applied',
                     $5::text::uuid, 'authoritative_applied', $6::text::uuid,
                     $7::text::uuid, $8::text::numeric)",
            &[
                &owner_user_id,
                &project_id,
                &position,
                &revision.project_activity_event_id,
                &ids.receipt_id,
                &revision.authoritative_commit_id,
                &revision.revision_id,
                &author_action_sequence,
            ],
        )
        .await
        .map_err(unavailable)?;
    let base_snapshot_id = Uuid::now_v7().to_string();
    let base_updates = client
        .execute(
            "UPDATE storyos.editor_session_base_snapshots AS snapshot
                SET snapshot_id = $4::text::uuid,
                    authoritative_revision_id = $5::text::uuid,
                    project_activity_position = $6::text::numeric,
                    created_at = clock_timestamp()
               FROM storyos.project_writer_generations AS writer
              WHERE snapshot.owner_user_id = $1::text::uuid
                AND snapshot.project_id = $2::text::uuid
                AND snapshot.editor_session_id = $3::text::uuid
                AND snapshot.authoritative_revision_id = $7::text::uuid
                AND (writer.owner_user_id, writer.project_id,
                     writer.current_editor_session_id) =
                    (snapshot.owner_user_id, snapshot.project_id, snapshot.editor_session_id)",
            &[
                &owner_user_id,
                &project_id,
                &write.editor_session_id,
                &base_snapshot_id,
                &revision.revision_id,
                &position,
                &write.prior_revision_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    match write.writer_base {
        RevisionBase::WhenOnPrior => {}
        RevisionBase::Required => {
            if base_updates != 1 {
                return Err(ProjectCommandError::BindingConflict);
            }
        }
    }
    crate::snapshot::persist_canonical_snapshot(
        client,
        scope,
        &base_snapshot_id,
        sequences.project_activity_position,
    )
    .await
    .map_err(unavailable)?;
    let blocks = load_revision_blocks(
        client,
        owner_user_id,
        project_id,
        &write.chapter_id,
        &revision.revision_id,
        &write.payload,
    )
    .await
    .map_err(unavailable)?;
    Ok(RevisionApplied {
        effect: write.effect,
        body: display_body_from_stored(&write.payload, &blocks),
        blocks,
        ids: sequences.ids,
        author_action_sequence: sequences.author_action_sequence,
        project_activity_position: sequences.project_activity_position,
    })
}
