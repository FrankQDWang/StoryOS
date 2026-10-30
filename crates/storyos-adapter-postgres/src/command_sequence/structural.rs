//! The `Structural` settlement profile: the Structural Authority Settlement of one Manuscript
//! Structure Transition (ADR 0029, ADR 0041).

use storyos_application::{
    ProjectCommandEnvelope, ProjectCommandError, ProjectScope, StructureApplied,
    StructureAuthority, StructureAuthorityEvidence,
};
use tokio_postgres::Client;

use super::structural_records::{
    StructuralInput, StructuralRecords, StructureCommit, WriterBaseRebind,
    persist_structural_records,
};
use super::{SettlementProfile, SingleRecordSet};
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::structural_authority_settlement::{
    StructureAffectedIdentity, StructureCommitBinding, StructureTransitionSequences,
    allocate_structure_transition_sequences,
};

pub(crate) enum StructureIdentity {
    Volume(String),
    Chapter(String),
    ChapterInitialRevision {
        chapter_id: String,
        revision_id: String,
    },
}

/// The Current Chapter that the transition leaves on the Project.
pub(crate) enum CurrentChapterChange {
    Preserve,
    Select(String),
    Clear,
}

/// Whether the current writer base Snapshot moves to the new canonical Snapshot.
pub(crate) enum WriterBase {
    Keep,
    RebindToCurrentChapter,
}

/// The applied writes that one structural command returns. The profile writes every authority record.
pub(crate) struct StructureWrite<E> {
    pub(crate) effect: E,
    pub(crate) resulting_tree_revision: u64,
    pub(crate) identity: StructureIdentity,
    pub(crate) current_chapter: CurrentChapterChange,
    pub(crate) writer_base: WriterBase,
    /// The command fields of the Activity payload. The profile adds `kind` and `tree_revision`.
    pub(crate) activity: serde_json::Value,
}

/// The Activity record, Authoritative Commit, Forward Author Action, canonical Snapshot, and
/// Manuscript Tree Revision advance of one applied Manuscript Structure Transition.
pub(crate) struct Structural;

impl SingleRecordSet for Structural {
    async fn allocate(
        client: &Client,
        scope: &ProjectScope,
    ) -> Result<StructureTransitionSequences, ProjectCommandError> {
        allocate_structure_transition_sequences(client, scope).await
    }
}

impl SettlementProfile for Structural {
    type Sequences = StructureTransitionSequences;
    type Write<E: Send> = StructureWrite<E>;
    type Applied<E: Send> = StructureApplied<E>;
    type Input = StructuralInput;

    fn commit_ids(sequences: &StructureTransitionSequences) -> Vec<String> {
        vec![sequences.authoritative_commit_id.clone()]
    }

    async fn persist<E: Send>(
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        input: StructuralInput,
        sequences: StructureTransitionSequences,
        write: StructureWrite<E>,
    ) -> Result<StructureApplied<E>, ProjectCommandError> {
        let resulting_current = match &write.current_chapter {
            CurrentChapterChange::Preserve => input.current_chapter_id.as_deref(),
            CurrentChapterChange::Select(chapter_id) => Some(chapter_id.as_str()),
            CurrentChapterChange::Clear => None,
        };
        let (identity, resulting_revision_id) = match &write.identity {
            StructureIdentity::Volume(volume_id) => {
                (StructureAffectedIdentity::Volume { volume_id }, None)
            }
            StructureIdentity::Chapter(chapter_id) => {
                (StructureAffectedIdentity::Chapter { chapter_id }, None)
            }
            StructureIdentity::ChapterInitialRevision {
                chapter_id,
                revision_id,
            } => (
                StructureAffectedIdentity::ChapterInitialRevision {
                    chapter_id,
                    resulting_revision_id: revision_id,
                },
                Some(revision_id.clone()),
            ),
        };
        let writer_base = match (&write.writer_base, resulting_current) {
            (WriterBase::RebindToCurrentChapter, Some(chapter_id)) => {
                WriterBaseRebind::CurrentWriter { chapter_id }
            }
            (WriterBase::Keep, _) | (WriterBase::RebindToCurrentChapter, None) => {
                WriterBaseRebind::Keep
            }
        };
        persist_structural_records(
            client,
            envelope,
            StructuralRecords {
                input: &input,
                resulting_tree_revision: write.resulting_tree_revision,
                resulting_current_chapter_id: resulting_current,
                author_action_sequence: sequences.author_action_sequence,
                project_activity_position: sequences.project_activity_position,
                project_activity_event_id: &sequences.project_activity_event_id,
                snapshot_id: &sequences.snapshot_id,
                activity: write.activity,
                activity_field: ("tree_revision", write.resulting_tree_revision.to_string()),
                commit: Some(StructureCommit {
                    sequences: &sequences,
                    binding: StructureCommitBinding {
                        prior_manuscript_tree_revision: input.tree_revision,
                        resulting_manuscript_tree_revision: write.resulting_tree_revision,
                        identity,
                    },
                }),
                writer_base,
            },
        )
        .await?;
        Ok(StructureApplied {
            effect: write.effect,
            project_activity_position: sequences.project_activity_position,
            project_activity_event_id: sequences.project_activity_event_id,
            authority: StructureAuthorityEvidence::Settled(StructureAuthority {
                authoritative_commit_id: sequences.authoritative_commit_id,
                author_action_sequence: sequences.author_action_sequence,
                snapshot_id: sequences.snapshot_id,
                prior_manuscript_tree_revision: input.tree_revision,
                resulting_manuscript_tree_revision: write.resulting_tree_revision,
                resulting_revision_id,
            }),
        })
    }

    fn replay<E: Send>(
        decode: impl FnOnce() -> Result<E, ReplayFault>,
        replay: &CommandReplay,
    ) -> Result<StructureApplied<E>, ReplayFault> {
        Ok(StructureApplied {
            effect: decode()?,
            project_activity_position: replay.project_activity_position,
            project_activity_event_id: replay.project_activity_event_id.clone(),
            authority: match &replay.authority {
                Some(authority) => StructureAuthorityEvidence::Settled(StructureAuthority {
                    authoritative_commit_id: authority.authoritative_commit_id.clone(),
                    author_action_sequence: authority.author_action_sequence,
                    snapshot_id: authority.snapshot_id.clone(),
                    prior_manuscript_tree_revision: authority.prior_manuscript_tree_revision,
                    resulting_manuscript_tree_revision: authority
                        .resulting_manuscript_tree_revision,
                    resulting_revision_id: authority.resulting_revision_id.clone(),
                }),
                None => StructureAuthorityEvidence::BeforeAuthorityHistoryFloor,
            },
        })
    }
}
