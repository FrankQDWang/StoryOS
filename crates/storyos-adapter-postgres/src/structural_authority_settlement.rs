use storyos_application::{ProjectCommandError, ProjectScope};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::command_sequence::{CommitCounters, allocate_commit_counters, unavailable};

mod compensation;
pub(crate) use compensation::{ObservedStructureFrontier, StructureCompensation};

pub(crate) struct StructureTransitionSequences {
    pub author_action_sequence: u64,
    pub authoritative_commit_sequence: u64,
    pub project_activity_position: u64,
    pub authoritative_commit_id: String,
    pub project_activity_event_id: String,
    pub snapshot_id: String,
}

pub(crate) struct CurrentChapterSequences {
    pub author_action_sequence: u64,
    pub project_activity_position: u64,
    pub project_activity_event_id: String,
    pub snapshot_id: String,
}

pub(crate) enum StructureAffectedIdentity<'a> {
    Volume {
        volume_id: &'a str,
    },
    Chapter {
        chapter_id: &'a str,
    },
    ChapterInitialRevision {
        chapter_id: &'a str,
        resulting_revision_id: &'a str,
    },
}

pub(crate) struct StructureCommitBinding<'a> {
    pub prior_manuscript_tree_revision: u64,
    pub resulting_manuscript_tree_revision: u64,
    pub identity: StructureAffectedIdentity<'a>,
}

pub(crate) async fn allocate_structure_transition_sequences(
    client: &Client,
    scope: &ProjectScope,
) -> Result<StructureTransitionSequences, ProjectCommandError> {
    let CommitCounters {
        author_action_sequence,
        authoritative_commit_sequence,
        project_activity_position,
    } = allocate_commit_counters(client, scope).await?;
    Ok(StructureTransitionSequences {
        author_action_sequence,
        authoritative_commit_sequence,
        project_activity_position,
        authoritative_commit_id: Uuid::now_v7().to_string(),
        project_activity_event_id: Uuid::now_v7().to_string(),
        snapshot_id: Uuid::now_v7().to_string(),
    })
}

pub(crate) async fn persist_structure_commit(
    client: &Client,
    scope: &ProjectScope,
    sequences: &StructureTransitionSequences,
    admission_id: &str,
    receipt_id: &str,
    binding: StructureCommitBinding<'_>,
) -> Result<(), tokio_postgres::Error> {
    match &binding.identity {
        StructureAffectedIdentity::Volume { .. } | StructureAffectedIdentity::Chapter { .. } => {
            persist_empty_pair_structure_commit(
                client,
                scope,
                sequences,
                admission_id,
                receipt_id,
                &binding,
            )
            .await?;
        }
        StructureAffectedIdentity::ChapterInitialRevision {
            chapter_id,
            resulting_revision_id,
        } => {
            client
                .execute(
                    "INSERT INTO storyos.authoritative_commits
                       (owner_user_id, project_id, authoritative_commit_id,
                        authoritative_commit_sequence, author_command_admission_id, receipt_id,
                        receipt_result_kind, prior_manuscript_tree_revision,
                        resulting_manuscript_tree_revision, affected_chapter_id,
                        manuscript_object_id, resulting_revision_id)
                     VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::numeric,
                             $5::text::uuid, $6::text::uuid, 'authoritative_applied',
                             $7::text::numeric, $8::text::numeric, $9::text::uuid,
                             $9::text::uuid, $10::text::uuid)",
                    &[
                        &scope.owner_user_id.as_ref(),
                        &scope.project_id.as_ref(),
                        &sequences.authoritative_commit_id,
                        &sequences.authoritative_commit_sequence.to_string(),
                        &admission_id,
                        &receipt_id,
                        &binding.prior_manuscript_tree_revision.to_string(),
                        &binding.resulting_manuscript_tree_revision.to_string(),
                        &chapter_id,
                        &resulting_revision_id,
                    ],
                )
                .await?;
        }
    }
    Ok(())
}

async fn persist_empty_pair_structure_commit(
    client: &Client,
    scope: &ProjectScope,
    sequences: &StructureTransitionSequences,
    admission_id: &str,
    receipt_id: &str,
    binding: &StructureCommitBinding<'_>,
) -> Result<(), tokio_postgres::Error> {
    let (affected_volume_id, affected_chapter_id) = match binding.identity {
        StructureAffectedIdentity::Volume { volume_id } => (Some(volume_id), None),
        StructureAffectedIdentity::Chapter { chapter_id } => (None, Some(chapter_id)),
        StructureAffectedIdentity::ChapterInitialRevision { .. } => {
            unreachable!("initial Chapter Revision uses the genesis Commit insert")
        }
    };
    client
        .execute(
            "INSERT INTO storyos.authoritative_commits
               (owner_user_id, project_id, authoritative_commit_id,
                authoritative_commit_sequence, author_command_admission_id, receipt_id,
                receipt_result_kind, prior_manuscript_tree_revision,
                resulting_manuscript_tree_revision, affected_volume_id, affected_chapter_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::numeric,
                     $5::text::uuid, $6::text::uuid, 'authoritative_applied',
                     $7::text::numeric, $8::text::numeric, $9::text::uuid, $10::text::uuid)",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &sequences.authoritative_commit_id,
                &sequences.authoritative_commit_sequence.to_string(),
                &admission_id,
                &receipt_id,
                &binding.prior_manuscript_tree_revision.to_string(),
                &binding.resulting_manuscript_tree_revision.to_string(),
                &affected_volume_id,
                &affected_chapter_id,
            ],
        )
        .await?;
    Ok(())
}

pub(crate) async fn allocate_current_chapter_sequences(
    client: &Client,
    scope: &ProjectScope,
) -> Result<CurrentChapterSequences, Box<dyn std::error::Error + Send + Sync>> {
    let row = client
        .query_one(
            "INSERT INTO storyos.scope_counters AS counters
               (owner_user_id, project_id, author_action_sequence,
                authoritative_commit_sequence, project_activity_position)
             VALUES ($1::text::uuid, $2::text::uuid, 1, 1, 1)
             ON CONFLICT (owner_user_id, project_id)
             DO UPDATE SET
               author_action_sequence = counters.author_action_sequence + 1,
               project_activity_position = counters.project_activity_position + 1
             RETURNING counters.author_action_sequence::text,
                       counters.project_activity_position::text",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await?;
    Ok(CurrentChapterSequences {
        author_action_sequence: parse_counter(row.get(0))?,
        project_activity_position: parse_counter(row.get(1))?,
        project_activity_event_id: Uuid::now_v7().to_string(),
        snapshot_id: Uuid::now_v7().to_string(),
    })
}

fn parse_counter(value: String) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    Ok(value.parse()?)
}

/// Moves the writer base Snapshot to a new canonical Snapshot of the Chapter Revision
/// `revision_id`, and gives the number of moved bases.
///
/// The base moves only for the current writer Editor Session. With `editor_session_id`, it moves
/// only when that session is the current writer.
pub(crate) async fn update_writer_base(
    client: &Client,
    scope: &ProjectScope,
    editor_session_id: Option<&str>,
    chapter_id: &str,
    revision_id: &str,
    snapshot_id: &str,
    project_activity_position: u64,
) -> Result<u64, ProjectCommandError> {
    client
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
                AND ($3::text IS NULL OR snapshot.editor_session_id = $3::text::uuid)
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
                &editor_session_id,
                &snapshot_id,
                &chapter_id,
                &revision_id,
                &project_activity_position.to_string(),
            ],
        )
        .await
        .map_err(unavailable)
}
