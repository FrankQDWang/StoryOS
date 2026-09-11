use storyos_application::ProjectScope;
use tokio_postgres::Client;
use uuid::Uuid;

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
) -> Result<StructureTransitionSequences, Box<dyn std::error::Error + Send + Sync>> {
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
        .await?;
    Ok(StructureTransitionSequences {
        author_action_sequence: parse_counter(row.get(0))?,
        authoritative_commit_sequence: parse_counter(row.get(1))?,
        project_activity_position: parse_counter(row.get(2))?,
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

pub(crate) async fn persist_current_chapter_forward_author_action(
    client: &Client,
    scope: &ProjectScope,
    author_action_sequence: u64,
    receipt_id: &str,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "INSERT INTO storyos.author_action_entries
               (owner_user_id, project_id, author_action_sequence, disposition,
                receipt_id, receipt_result_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, 'forward',
                     $4::text::uuid, 'authoritative_applied')",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &author_action_sequence.to_string(),
                &receipt_id,
            ],
        )
        .await?;
    Ok(())
}

pub(crate) async fn persist_current_chapter_compensation_author_action(
    client: &Client,
    scope: &ProjectScope,
    author_action_sequence: u64,
    receipt_id: &str,
    source_sequence: u64,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "INSERT INTO storyos.author_action_entries
               (owner_user_id, project_id, author_action_sequence, disposition,
                compensated_source_sequence, receipt_id, receipt_result_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, 'compensation',
                     $4::text::numeric, $5::text::uuid, 'authoritative_applied')",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &author_action_sequence.to_string(),
                &source_sequence.to_string(),
                &receipt_id,
            ],
        )
        .await?;
    Ok(())
}

pub(crate) async fn persist_forward_author_action(
    client: &Client,
    scope: &ProjectScope,
    sequences: &StructureTransitionSequences,
    receipt_id: &str,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "INSERT INTO storyos.author_action_entries
               (owner_user_id, project_id, author_action_sequence, disposition,
                authoritative_commit_id, receipt_id, receipt_result_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, 'forward',
                     $4::text::uuid, $5::text::uuid, 'authoritative_applied')",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &sequences.author_action_sequence.to_string(),
                &sequences.authoritative_commit_id,
                &receipt_id,
            ],
        )
        .await?;
    Ok(())
}

fn parse_counter(value: String) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    Ok(value.parse()?)
}

pub(crate) async fn persist_compensation_author_action(
    client: &Client,
    scope: &ProjectScope,
    sequences: &StructureTransitionSequences,
    receipt_id: &str,
    source_sequence: u64,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "INSERT INTO storyos.author_action_entries
               (owner_user_id, project_id, author_action_sequence, disposition,
                compensated_source_sequence, authoritative_commit_id, receipt_id,
                receipt_result_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, 'compensation',
                     $4::text::numeric, $5::text::uuid, $6::text::uuid, 'authoritative_applied')",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &sequences.author_action_sequence.to_string(),
                &source_sequence.to_string(),
                &sequences.authoritative_commit_id,
                &receipt_id,
            ],
        )
        .await?;
    Ok(())
}
