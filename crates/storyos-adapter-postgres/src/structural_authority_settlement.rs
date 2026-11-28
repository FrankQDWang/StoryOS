use crate::statement_cache::Client;
use storyos_application::{ProjectCommandError, ProjectScope};
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
    // A Structure Transition changes the Chapter or the Chapters of the Volume (ADR 0095 section 4).
    let changed = match &binding.identity {
        StructureAffectedIdentity::Volume { volume_id } => *volume_id,
        StructureAffectedIdentity::Chapter { chapter_id }
        | StructureAffectedIdentity::ChapterInitialRevision { chapter_id, .. } => *chapter_id,
    };
    crate::passage_index_work::mark_passage_index_work(
        client,
        scope,
        crate::passage_index_work::MarkedChapters::Changed(&[changed]),
    )
    .await?;
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

/// Moves the base Snapshot of the Editor Session `editor_session_id` to the Snapshot `snapshot_id`
/// of the Chapter Revision `revision_id`, and gives the number of moved bases.
pub(crate) async fn update_session_base(
    client: &Client,
    scope: &ProjectScope,
    editor_session_id: &str,
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
              WHERE snapshot.owner_user_id = $1::text::uuid
                AND snapshot.project_id = $2::text::uuid
                AND snapshot.editor_session_id = $3::text::uuid",
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

/// Moves the base Snapshot of each Editor Session on the Chapter `removed_chapter_id` to the
/// Chapter Revision `revision_id`. Each base gets a new canonical Snapshot of its own at
/// `project_activity_position`, because the Snapshot ID is the key of a base row and a browser with
/// no stored Activity cursor starts the Activity stream from its base Snapshot.
pub(crate) async fn update_each_session_base(
    client: &Client,
    scope: &ProjectScope,
    removed_chapter_id: &str,
    chapter_id: &str,
    revision_id: &str,
    project_activity_position: u64,
) -> Result<(), ProjectCommandError> {
    let sessions = client
        .query(
            "SELECT editor_session_id::text FROM storyos.editor_session_base_snapshots
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND chapter_object_id = $3::text::uuid
              ORDER BY editor_session_id",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &removed_chapter_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    for session in sessions {
        let editor_session_id: String = session.get(/*idx*/ 0);
        let snapshot_id = Uuid::now_v7().to_string();
        crate::snapshot::persist_canonical_snapshot(
            client,
            scope,
            &snapshot_id,
            project_activity_position,
        )
        .await
        .map_err(unavailable)?;
        update_session_base(
            client,
            scope,
            &editor_session_id,
            chapter_id,
            revision_id,
            &snapshot_id,
            project_activity_position,
        )
        .await?;
    }
    Ok(())
}
