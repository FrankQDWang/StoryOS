//! The `ChapterSelection` settlement profile: the Structural Authority Settlement of one
//! Current Chapter change, which has an Author Action and a Snapshot but no Commit (ADR 0026).

use storyos_application::{
    AuthorityEvidence, ChapterSelectionApplied, ChapterSelectionAuthority, ProjectCommandEnvelope,
    ProjectCommandError, ProjectScope,
};
use tokio_postgres::Client;

use super::records::insert_applied_activity;
use super::{LockedProject, SettlementProfile, unavailable};
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::structural_authority_settlement::{
    CurrentChapterSequences, allocate_current_chapter_sequences,
    persist_current_chapter_forward_author_action,
};

/// The applied writes of one Current Chapter change. The profile writes every authority record.
pub(crate) struct ChapterSelectionWrite<E> {
    pub(crate) effect: E,
    /// The writer Editor Session whose base Snapshot moves to the selected Chapter.
    pub(crate) editor_session_id: String,
    pub(crate) prior_chapter_id: String,
    pub(crate) chapter_id: String,
    /// The current Authoritative Revision of the selected Chapter.
    pub(crate) chapter_revision_id: String,
}

/// The Current Chapter update, canonical Snapshot, writer base rebind, Activity record, and
/// Forward Author Action of one applied Current Chapter change.
pub(crate) struct ChapterSelection;

impl SettlementProfile for ChapterSelection {
    type Sequences = CurrentChapterSequences;
    type Write<E: Send> = ChapterSelectionWrite<E>;
    type Applied<E: Send> = ChapterSelectionApplied<E>;

    async fn allocate(
        client: &Client,
        scope: &ProjectScope,
    ) -> Result<CurrentChapterSequences, ProjectCommandError> {
        allocate_current_chapter_sequences(client, scope)
            .await
            .map_err(ProjectCommandError::Unavailable)
    }

    fn commit_ids(_sequences: &CurrentChapterSequences) -> Vec<String> {
        Vec::new()
    }

    async fn persist<E: Send>(
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
        activity_kind: &'static str,
        sequences: CurrentChapterSequences,
        write: ChapterSelectionWrite<E>,
    ) -> Result<ChapterSelectionApplied<E>, ProjectCommandError> {
        let scope = &envelope.project_scope;
        let updated = client
            .execute(
                "UPDATE storyos.projects
                    SET current_chapter_id = $3::text::uuid
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND lifecycle_state = 'active'
                    AND current_chapter_id = $4::text::uuid",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &write.chapter_id,
                    &write.prior_chapter_id,
                ],
            )
            .await
            .map_err(unavailable)?;
        if updated != 1 {
            return Err(unavailable("current Chapter changed under FOR UPDATE"));
        }
        crate::snapshot::persist_canonical_snapshot(
            client,
            scope,
            &sequences.snapshot_id,
            sequences.project_activity_position,
        )
        .await
        .map_err(unavailable)?;
        let base_updates = client
            .execute(
                "UPDATE storyos.editor_session_base_snapshots AS snapshot
                    SET snapshot_id = $4::text::uuid,
                        chapter_object_id = $5::text::uuid,
                        authoritative_revision_id = $6::text::uuid,
                        project_activity_position = $7::text::numeric,
                        created_at = clock_timestamp()
                   FROM storyos.project_writer_generations AS writer
                  WHERE snapshot.owner_user_id = $1::text::uuid
                    AND snapshot.project_id = $2::text::uuid
                    AND snapshot.editor_session_id = $3::text::uuid
                    AND (writer.owner_user_id, writer.project_id,
                         writer.current_editor_session_id) =
                        (snapshot.owner_user_id, snapshot.project_id,
                         snapshot.editor_session_id)
                    AND writer.writer_generation = (
                      SELECT max(current_writer.writer_generation)
                        FROM storyos.project_writer_generations AS current_writer
                       WHERE current_writer.owner_user_id = snapshot.owner_user_id
                         AND current_writer.project_id = snapshot.project_id
                    )",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &write.editor_session_id,
                    &sequences.snapshot_id,
                    &write.chapter_id,
                    &write.chapter_revision_id,
                    &sequences.project_activity_position.to_string(),
                ],
            )
            .await
            .map_err(unavailable)?;
        if base_updates != 1 {
            return Err(ProjectCommandError::BindingConflict);
        }
        insert_applied_activity(
            client,
            envelope,
            activity_kind,
            sequences.project_activity_position,
            &sequences.project_activity_event_id,
            serde_json::json!({
                "prior_chapter_id": write.prior_chapter_id,
                "current_chapter_id": write.chapter_id,
            }),
            &[("base_snapshot_id", sequences.snapshot_id.clone())],
        )
        .await?;
        persist_current_chapter_forward_author_action(
            client,
            scope,
            sequences.author_action_sequence,
            &envelope.ids.receipt_id,
        )
        .await
        .map_err(unavailable)?;
        Ok(ChapterSelectionApplied {
            effect: write.effect,
            project_activity_position: sequences.project_activity_position,
            project_activity_event_id: sequences.project_activity_event_id,
            authority: AuthorityEvidence::Settled(ChapterSelectionAuthority {
                author_action_sequence: sequences.author_action_sequence,
                snapshot_id: sequences.snapshot_id,
                manuscript_tree_revision: project.tree_revision,
            }),
        })
    }

    fn replay<E: Send>(
        effect: E,
        replay: &CommandReplay,
    ) -> Result<ChapterSelectionApplied<E>, ReplayFault> {
        let authority = match (
            replay.author_action_sequence,
            &replay.snapshot_id,
            &replay.manuscript_tree_revision,
        ) {
            (Some(author_action_sequence), Some(snapshot_id), Some(manuscript_tree_revision)) => {
                AuthorityEvidence::Settled(ChapterSelectionAuthority {
                    author_action_sequence,
                    snapshot_id: snapshot_id.clone(),
                    manuscript_tree_revision: manuscript_tree_revision
                        .parse()
                        .map_err(|error| ReplayFault::Unavailable(Box::new(error)))?,
                })
            }
            (None, None, _) => AuthorityEvidence::BeforeAuthorityHistoryFloor,
            (Some(_), None, _) | (None, Some(_), _) | (Some(_), Some(_), None) => {
                return Err(ReplayFault::Unavailable(
                    "Current Chapter authority evidence is damaged".into(),
                ));
            }
        };
        Ok(ChapterSelectionApplied {
            effect,
            project_activity_position: replay.project_activity_position,
            project_activity_event_id: replay.project_activity_event_id.clone(),
            authority,
        })
    }
}
