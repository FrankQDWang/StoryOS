//! The record writes that the `Structural` and `ChapterSelection` profiles share (ADR 0076).

use storyos_application::{ProjectCommandEnvelope, ProjectCommandError};
use tokio_postgres::Client;

use super::records::insert_applied_activity;
use super::{ActionDisposition, PersistFacts, PersistInput, insert_author_action, unavailable};
use crate::structural_authority_settlement::{
    StructureCommitBinding, StructureTransitionSequences, persist_structure_commit,
    update_writer_base,
};

/// The settlement facts that `Structural` and `ChapterSelection` read: the locked Project facts
/// that guard the Project update, and the Activity kind of the command.
pub(crate) struct StructuralInput {
    pub(super) tree_revision: u64,
    pub(super) current_chapter_id: Option<String>,
    pub(super) activity_kind: &'static str,
}

impl PersistInput for StructuralInput {
    fn from_facts(facts: &PersistFacts<'_>) -> Self {
        Self {
            tree_revision: facts.project.tree_revision,
            current_chapter_id: facts.project.current_chapter_id.clone(),
            activity_kind: facts.activity_kind,
        }
    }
}

/// The writer base Snapshot that moves to the new canonical Snapshot.
pub(super) enum WriterBaseRebind<'a> {
    Keep,
    /// The base of the current writer, if one exists, moves to the current head of the Chapter.
    CurrentWriter {
        chapter_id: &'a str,
    },
    /// The base of the Editor Session moves to the Chapter Revision. A session that is not the
    /// current writer is a binding conflict.
    Session {
        editor_session_id: &'a str,
        chapter_id: &'a str,
        revision_id: &'a str,
    },
}

/// The Authoritative Commit of a `Structural` settlement.
pub(super) struct StructureCommit<'a> {
    pub(super) sequences: &'a StructureTransitionSequences,
    pub(super) binding: StructureCommitBinding<'a>,
}

/// The records of one applied Structural Authority Settlement.
pub(super) struct StructuralRecords<'a> {
    pub(super) input: &'a StructuralInput,
    pub(super) resulting_tree_revision: u64,
    pub(super) resulting_current_chapter_id: Option<&'a str>,
    pub(super) author_action_sequence: u64,
    pub(super) project_activity_position: u64,
    pub(super) project_activity_event_id: &'a str,
    pub(super) snapshot_id: &'a str,
    /// The command fields of the Activity payload.
    pub(super) activity: serde_json::Value,
    /// The profile field that the Activity payload adds.
    pub(super) activity_field: (&'static str, String),
    /// `ChapterSelection` has no Authoritative Commit.
    pub(super) commit: Option<StructureCommit<'a>>,
    pub(super) writer_base: WriterBaseRebind<'a>,
}

/// Writes the guarded Project update, the Activity record, the Authoritative Commit, the Forward
/// Author Action, the canonical Snapshot, and the writer base rebind, in this order.
pub(super) async fn persist_structural_records(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    records: StructuralRecords<'_>,
) -> Result<(), ProjectCommandError> {
    let scope = &envelope.project_scope;
    let ids = &envelope.ids;
    let prior = records.input;
    let updated = client
        .execute(
            "UPDATE storyos.projects
                SET tree_revision = $3::text::bigint, current_chapter_id = $5::text::uuid
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND tree_revision = $4::text::bigint
                AND current_chapter_id IS NOT DISTINCT FROM $6::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &records.resulting_tree_revision.to_string(),
                &prior.tree_revision.to_string(),
                &records.resulting_current_chapter_id,
                &prior.current_chapter_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    if updated != 1 {
        return Err(unavailable(
            "tree revision or Current Chapter changed under FOR UPDATE",
        ));
    }
    insert_applied_activity(
        client,
        envelope,
        prior.activity_kind,
        records.project_activity_position,
        records.project_activity_event_id,
        records.activity,
        &[records.activity_field],
    )
    .await?;
    let authoritative_commit_id = match records.commit {
        Some(StructureCommit { sequences, binding }) => {
            persist_structure_commit(
                client,
                scope,
                sequences,
                &ids.author_command_admission_id,
                &ids.receipt_id,
                binding,
            )
            .await
            .map_err(unavailable)?;
            Some(sequences.authoritative_commit_id.as_str())
        }
        None => None,
    };
    insert_author_action(
        client,
        scope,
        records.author_action_sequence,
        &ActionDisposition::Forward,
        authoritative_commit_id,
        &ids.receipt_id,
    )
    .await?;
    crate::snapshot::persist_canonical_snapshot(
        client,
        scope,
        records.snapshot_id,
        records.project_activity_position,
    )
    .await
    .map_err(unavailable)?;
    match records.writer_base {
        WriterBaseRebind::Keep => {}
        WriterBaseRebind::CurrentWriter { chapter_id } => {
            let head = client
                .query_opt(
                    "SELECT head.current_revision_id::text
                       FROM storyos.authoritative_heads AS head
                      WHERE head.owner_user_id = $1::text::uuid
                        AND head.project_id = $2::text::uuid
                        AND head.manuscript_object_id = $3::text::uuid",
                    &[
                        &scope.owner_user_id.as_ref(),
                        &scope.project_id.as_ref(),
                        &chapter_id,
                    ],
                )
                .await
                .map_err(unavailable)?
                .map(|row| row.get::<_, String>(/*idx*/ 0))
                .ok_or_else(|| unavailable("the Current Chapter has no authoritative head"))?;
            update_writer_base(
                client,
                scope,
                None,
                chapter_id,
                &head,
                records.snapshot_id,
                records.project_activity_position,
            )
            .await?;
        }
        WriterBaseRebind::Session {
            editor_session_id,
            chapter_id,
            revision_id,
        } => {
            let moved = update_writer_base(
                client,
                scope,
                Some(editor_session_id),
                chapter_id,
                revision_id,
                records.snapshot_id,
                records.project_activity_position,
            )
            .await?;
            if moved != 1 {
                return Err(ProjectCommandError::BindingConflict);
            }
        }
    }
    Ok(())
}
