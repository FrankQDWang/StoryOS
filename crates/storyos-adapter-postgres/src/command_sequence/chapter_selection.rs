//! The `ChapterSelection` settlement profile: the Structural Authority Settlement of one
//! Current Chapter change, which has an Author Action and a Snapshot but no Commit (ADR 0026).

use storyos_application::{
    AuthorityEvidence, ChapterSelectionApplied, ChapterSelectionAuthority, ProjectCommandEnvelope,
    ProjectCommandError, ProjectScope,
};
use tokio_postgres::Client;

use super::structural_records::{
    StructuralInput, StructuralRecords, WriterBaseRebind, persist_structural_records,
};
use super::{SettlementProfile, SingleRecordSet};
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::structural_authority_settlement::{
    CurrentChapterSequences, allocate_current_chapter_sequences,
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

/// The Current Chapter update, Activity record, Forward Author Action, canonical Snapshot, and
/// writer base rebind of one applied Current Chapter change.
pub(crate) struct ChapterSelection;

impl SingleRecordSet for ChapterSelection {
    async fn allocate(
        client: &Client,
        scope: &ProjectScope,
    ) -> Result<CurrentChapterSequences, ProjectCommandError> {
        allocate_current_chapter_sequences(client, scope)
            .await
            .map_err(ProjectCommandError::Unavailable)
    }
}

impl SettlementProfile for ChapterSelection {
    type Sequences = CurrentChapterSequences;
    type Write<E: Send> = ChapterSelectionWrite<E>;
    type Applied<E: Send> = ChapterSelectionApplied<E>;
    type Input = StructuralInput;

    fn commit_ids(_sequences: &CurrentChapterSequences) -> Vec<String> {
        Vec::new()
    }

    /// The locked Current Chapter guards the Project update. Core applies a Current Chapter
    /// change only when the locked Current Chapter is the prior Chapter of the write.
    async fn persist<E: Send>(
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        input: StructuralInput,
        sequences: CurrentChapterSequences,
        write: ChapterSelectionWrite<E>,
    ) -> Result<ChapterSelectionApplied<E>, ProjectCommandError> {
        persist_structural_records(
            client,
            envelope,
            StructuralRecords {
                input: &input,
                resulting_tree_revision: input.tree_revision,
                resulting_current_chapter_id: Some(&write.chapter_id),
                author_action_sequence: sequences.author_action_sequence,
                project_activity_position: sequences.project_activity_position,
                project_activity_event_id: &sequences.project_activity_event_id,
                snapshot_id: &sequences.snapshot_id,
                activity: serde_json::json!({
                    "prior_chapter_id": write.prior_chapter_id,
                    "current_chapter_id": write.chapter_id,
                }),
                activity_field: ("base_snapshot_id", sequences.snapshot_id.clone()),
                commit: None,
                writer_base: WriterBaseRebind::Session {
                    editor_session_id: &write.editor_session_id,
                    chapter_id: &write.chapter_id,
                    revision_id: &write.chapter_revision_id,
                },
            },
        )
        .await?;
        Ok(ChapterSelectionApplied {
            effect: write.effect,
            project_activity_position: sequences.project_activity_position,
            project_activity_event_id: sequences.project_activity_event_id,
            authority: AuthorityEvidence::Settled(ChapterSelectionAuthority {
                author_action_sequence: sequences.author_action_sequence,
                snapshot_id: sequences.snapshot_id,
                manuscript_tree_revision: input.tree_revision,
            }),
        })
    }

    fn replay<E: Send>(
        decode: impl FnOnce() -> Result<E, ReplayFault>,
        replay: &CommandReplay,
    ) -> Result<ChapterSelectionApplied<E>, ReplayFault> {
        let authority = match (
            &replay.author_action_sequence,
            &replay.snapshot_id,
            replay.manuscript_tree_revision()?,
        ) {
            (Some(author_action_sequence), Some(snapshot_id), Some(manuscript_tree_revision)) => {
                AuthorityEvidence::Settled(ChapterSelectionAuthority {
                    author_action_sequence: author_action_sequence
                        .parse()
                        .map_err(|error| ReplayFault::Unavailable(Box::new(error)))?,
                    snapshot_id: snapshot_id.clone(),
                    manuscript_tree_revision,
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
            effect: decode()?,
            project_activity_position: replay.project_activity_position,
            project_activity_event_id: replay.project_activity_event_id.clone(),
            authority,
        })
    }
}
