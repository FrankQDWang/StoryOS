use storyos_application::{
    ChapterId, ManuscriptSearchBlockFact, ManuscriptSearchChapterFact, ManuscriptSearchFactExtent,
    ManuscriptSearchFacts, ManuscriptSearchRead, ManuscriptSearchReader, ProjectReadError,
    ProjectScope,
};
use tokio_postgres::GenericClient;

use super::{PostgresProjectReader, read_error, set_scope};
use crate::snapshot::CanonicalSnapshotQuery;

impl ManuscriptSearchReader for PostgresProjectReader {
    async fn read_search_facts(
        &self,
        scope: &ProjectScope,
        fact_extent: ManuscriptSearchFactExtent,
    ) -> Result<ManuscriptSearchRead, ProjectReadError> {
        let mut client = self.connect().await?;
        let transaction = client.transaction().await.map_err(read_error)?;
        set_scope(&transaction, scope).await?;
        let Some(project) = transaction
            .query_opt(
                "SELECT current_chapter_id::text
                   FROM storyos.projects
                  WHERE owner_user_id = $1::text::uuid
                    AND project_id = $2::text::uuid
                    AND lifecycle_state = 'active'",
                &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
            )
            .await
            .map_err(read_error)?
        else {
            transaction.commit().await.map_err(read_error)?;
            return Ok(ManuscriptSearchRead::Missing);
        };
        let current_chapter_id = project.get::<_, Option<String>>(0).map(ChapterId::new);
        let snapshot =
            crate::snapshot::load_latest_canonical_snapshot_query(&transaction, scope).await?;
        let opened = match snapshot {
            CanonicalSnapshotQuery::Missing => ManuscriptSearchRead::Missing,
            CanonicalSnapshotQuery::Expired => ManuscriptSearchRead::SnapshotExpired,
            CanonicalSnapshotQuery::Available(snapshot) => {
                let chapters = match (fact_extent, current_chapter_id.as_ref()) {
                    (ManuscriptSearchFactExtent::AllChapters, _) => {
                        read_live_chapter_blocks(
                            &transaction,
                            scope,
                            LiveChapterReadExtent::AllChapters,
                        )
                        .await?
                    }
                    (ManuscriptSearchFactExtent::CurrentChapter, Some(chapter_id)) => {
                        read_live_chapter_blocks(
                            &transaction,
                            scope,
                            LiveChapterReadExtent::OneChapter { chapter_id },
                        )
                        .await?
                    }
                    (ManuscriptSearchFactExtent::CurrentChapter, None) => Vec::new(),
                };
                ManuscriptSearchRead::Ready(Box::new(ManuscriptSearchFacts {
                    project_scope: scope.clone(),
                    snapshot,
                    current_chapter_id,
                    chapters,
                }))
            }
        };
        transaction.commit().await.map_err(read_error)?;
        Ok(opened)
    }
}

pub(crate) enum LiveChapterReadExtent<'a> {
    AllChapters,
    OneChapter { chapter_id: &'a ChapterId },
}

const LIVE_CHAPTER_BLOCKS_SELECT_FROM_WHERE: &str = "SELECT volume.manuscript_object_id::text,
                    chapter.manuscript_object_id::text,
                    convert_from(payload.canonical_bytes, 'UTF8'),
                    COALESCE(
                      (
                        SELECT array_agg(block.manuscript_block_id::text ORDER BY member.block_order)
                          FROM storyos.manuscript_revision_members AS member
                          JOIN storyos.manuscript_blocks AS block
                            ON (block.owner_user_id, block.project_id, block.manuscript_block_id) =
                               (member.owner_user_id, member.project_id, member.manuscript_block_id)
                         WHERE member.owner_user_id = chapter.owner_user_id
                           AND member.project_id = chapter.project_id
                           AND member.manuscript_object_id = chapter.manuscript_object_id
                           AND member.revision_id = revision.revision_id
                      ),
                      ARRAY[]::text[]
                    )
               FROM storyos.manuscript_objects AS chapter
               JOIN storyos.manuscript_objects AS volume
                 ON (volume.owner_user_id, volume.project_id, volume.manuscript_object_id) =
                    (chapter.owner_user_id, chapter.project_id, chapter.parent_volume_id)
               JOIN storyos.authoritative_heads AS head
                 ON (head.owner_user_id, head.project_id, head.manuscript_object_id) =
                    (chapter.owner_user_id, chapter.project_id, chapter.manuscript_object_id)
               JOIN storyos.authoritative_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.manuscript_object_id,
                     revision.revision_id) =
                    (head.owner_user_id, head.project_id, head.manuscript_object_id,
                     head.current_revision_id)
               JOIN storyos.authoritative_payloads AS payload
                 ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
                    (revision.owner_user_id, revision.project_id, revision.payload_id)
              WHERE chapter.owner_user_id = $1::text::uuid
                AND chapter.project_id = $2::text::uuid
                AND chapter.object_kind = 'chapter'
                AND volume.object_kind = 'volume'
                AND chapter.parent_volume_id IS NOT NULL
                AND NOT EXISTS (
                  SELECT 1 FROM storyos.chapter_removal_decisions AS removal
                   WHERE removal.owner_user_id = chapter.owner_user_id
                     AND removal.project_id = chapter.project_id
                     AND removal.chapter_id = chapter.manuscript_object_id
                )
                AND NOT EXISTS (
                  SELECT 1 FROM storyos.volume_removal_decisions AS removal
                   WHERE removal.owner_user_id = volume.owner_user_id
                     AND removal.project_id = volume.project_id
                     AND removal.volume_id = volume.manuscript_object_id
                )";

pub(crate) async fn read_live_chapter_blocks(
    client: &impl GenericClient,
    scope: &ProjectScope,
    extent: LiveChapterReadExtent<'_>,
) -> Result<Vec<ManuscriptSearchChapterFact>, ProjectReadError> {
    #[cfg(test)]
    crate::manuscript_block::revision_member_read_sql_statement_count::increment();
    let rows = match extent {
        LiveChapterReadExtent::AllChapters => {
            let sql = format!(
                "{LIVE_CHAPTER_BLOCKS_SELECT_FROM_WHERE}\n              ORDER BY volume.tree_order, chapter.tree_order"
            );
            client
                .query(
                    sql.as_str(),
                    &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
                )
                .await
        }
        LiveChapterReadExtent::OneChapter { chapter_id } => {
            let sql = format!(
                "{LIVE_CHAPTER_BLOCKS_SELECT_FROM_WHERE}\n                AND chapter.manuscript_object_id = $3::text::uuid\n              ORDER BY volume.tree_order, chapter.tree_order"
            );
            client
                .query(
                    sql.as_str(),
                    &[
                        &scope.owner_user_id.as_ref(),
                        &scope.project_id.as_ref(),
                        &chapter_id.as_ref(),
                    ],
                )
                .await
        }
    }
    .map_err(read_error)?;
    let mut chapters = Vec::with_capacity(rows.len());
    let mut last_volume = None;
    let mut volume_order = 0_u64;
    let mut chapter_order = 0_u64;
    for row in rows {
        let volume_id = row.get::<_, String>(0);
        if last_volume.as_ref() != Some(&volume_id) {
            volume_order += 1;
            chapter_order = 0;
            last_volume = Some(volume_id);
        }
        chapter_order += 1;
        let chapter_id = row.get::<_, String>(1);
        let stored: String = row.get(2);
        let member_ids: Vec<String> = row.get(3);
        let blocks = crate::manuscript_block::blocks_from_stored_payload(&stored, &member_ids);
        chapters.push(ManuscriptSearchChapterFact {
            project_scope: scope.clone(),
            chapter_id: ChapterId::new(chapter_id),
            volume_order,
            chapter_order,
            blocks: blocks
                .into_iter()
                .map(|block| ManuscriptSearchBlockFact {
                    manuscript_block_id: block.manuscript_block_id,
                    text: block.text,
                })
                .collect(),
        });
    }
    Ok(chapters)
}

#[cfg(test)]
#[path = "manuscript_search_tests.rs"]
mod tests;
