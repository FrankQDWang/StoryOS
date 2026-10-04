//! The `Structural` settlement profile: the Structural Authority Settlement of one Manuscript
//! Structure Transition (ADR 0029, ADR 0041).

use storyos_application::{
    ProjectCommandEnvelope, ProjectCommandError, ProjectScope, StructureApplied,
    StructureAuthority, StructureAuthorityEvidence,
};
use tokio_postgres::Client;

use super::records::insert_activity;
use super::{LockedProject, SettlementProfile, unavailable};
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::structural_authority_settlement::{
    StructureAffectedIdentity, StructureCommitBinding, StructureTransitionSequences,
    allocate_structure_transition_sequences, persist_forward_author_action,
    persist_structure_commit, rebind_writer_base,
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

/// The applied writes that one structural command returns; the profile writes every authority record.
pub(crate) struct StructureWrite<E> {
    pub(crate) effect: E,
    pub(crate) resulting_tree_revision: u64,
    pub(crate) identity: StructureIdentity,
    pub(crate) current_chapter: CurrentChapterChange,
    pub(crate) writer_base: WriterBase,
    /// The command fields of the Activity payload; the profile adds `kind` and `tree_revision`.
    pub(crate) activity: serde_json::Value,
}

/// The Activity record, Authoritative Commit, Forward Author Action, canonical Snapshot, and
/// Manuscript Tree Revision advance of one applied Manuscript Structure Transition.
pub(crate) struct Structural;

impl SettlementProfile for Structural {
    type Sequences = StructureTransitionSequences;
    type Write<E: Send> = StructureWrite<E>;
    type Applied<E: Send> = StructureApplied<E>;

    async fn allocate(
        client: &Client,
        scope: &ProjectScope,
    ) -> Result<StructureTransitionSequences, ProjectCommandError> {
        allocate_structure_transition_sequences(client, scope)
            .await
            .map_err(ProjectCommandError::Unavailable)
    }

    fn commit_ids(sequences: &StructureTransitionSequences) -> Vec<String> {
        vec![sequences.authoritative_commit_id.clone()]
    }

    async fn persist<E: Send>(
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
        activity_kind: &'static str,
        sequences: StructureTransitionSequences,
        write: StructureWrite<E>,
    ) -> Result<StructureApplied<E>, ProjectCommandError> {
        let scope = &envelope.project_scope;
        let ids = &envelope.ids;
        let resulting_current = match &write.current_chapter {
            CurrentChapterChange::Preserve => project.current_chapter_id.clone(),
            CurrentChapterChange::Select(chapter_id) => Some(chapter_id.clone()),
            CurrentChapterChange::Clear => None,
        };
        let bumped = client
            .execute(
                "UPDATE storyos.projects
                    SET tree_revision = $3::text::bigint, current_chapter_id = $5::text::uuid
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND tree_revision = $4::text::bigint AND lifecycle_state = 'active'
                    AND current_chapter_id IS NOT DISTINCT FROM $6::text::uuid",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &write.resulting_tree_revision.to_string(),
                    &project.tree_revision.to_string(),
                    &resulting_current,
                    &project.current_chapter_id,
                ],
            )
            .await
            .map_err(unavailable)?;
        if bumped != 1 {
            return Err(unavailable(
                "tree revision or Current Chapter changed under FOR UPDATE",
            ));
        }
        let serde_json::Value::Object(mut activity) = write.activity else {
            return Err(unavailable("the Activity payload is not an object"));
        };
        activity.insert("kind".to_owned(), activity_kind.into());
        activity.insert(
            "tree_revision".to_owned(),
            write.resulting_tree_revision.to_string().into(),
        );
        insert_activity(
            client,
            envelope,
            sequences.project_activity_position,
            &sequences.project_activity_event_id,
            activity_kind,
            activity,
        )
        .await?;
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
        persist_structure_commit(
            client,
            scope,
            &sequences,
            &ids.author_command_admission_id,
            &ids.receipt_id,
            StructureCommitBinding {
                prior_manuscript_tree_revision: project.tree_revision,
                resulting_manuscript_tree_revision: write.resulting_tree_revision,
                identity,
            },
        )
        .await
        .map_err(unavailable)?;
        persist_forward_author_action(client, scope, &sequences, &ids.receipt_id)
            .await
            .map_err(unavailable)?;
        crate::snapshot::persist_canonical_snapshot(
            client,
            scope,
            &sequences.snapshot_id,
            sequences.project_activity_position,
        )
        .await
        .map_err(unavailable)?;
        if let (WriterBase::RebindToCurrentChapter, Some(chapter_id)) =
            (&write.writer_base, &resulting_current)
        {
            rebind_writer_base(
                client,
                scope,
                chapter_id,
                &sequences.snapshot_id,
                sequences.project_activity_position,
            )
            .await
            .map_err(ProjectCommandError::Unavailable)?;
        }
        Ok(StructureApplied {
            effect: write.effect,
            project_activity_position: sequences.project_activity_position,
            project_activity_event_id: sequences.project_activity_event_id,
            authority: StructureAuthorityEvidence::Settled(StructureAuthority {
                authoritative_commit_id: sequences.authoritative_commit_id,
                author_action_sequence: sequences.author_action_sequence,
                snapshot_id: sequences.snapshot_id,
                prior_manuscript_tree_revision: project.tree_revision,
                resulting_manuscript_tree_revision: write.resulting_tree_revision,
                resulting_revision_id,
            }),
        })
    }

    fn replay<E: Send>(
        effect: E,
        replay: &CommandReplay,
    ) -> Result<StructureApplied<E>, ReplayFault> {
        Ok(StructureApplied {
            effect,
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
