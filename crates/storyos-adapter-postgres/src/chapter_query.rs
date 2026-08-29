use storyos_application::{
    Chapter, ChapterId, ChapterQueryFacts, ChapterQueryReader, OpenChapter, ProjectReadError,
    ProjectScope, RevisionId,
};

use super::{PostgresProjectReader, read_error, set_scope};
use crate::snapshot::CanonicalSnapshotQuery;

impl ChapterQueryReader for PostgresProjectReader {
    async fn read_chapter_query(
        &self,
        scope: &ProjectScope,
        chapter_id: &ChapterId,
    ) -> Result<OpenChapter, ProjectReadError> {
        let mut client = self.connect().await?;
        let transaction = client.transaction().await.map_err(read_error)?;
        set_scope(&transaction, scope).await?;
        let snapshot =
            crate::snapshot::load_latest_canonical_snapshot_query(&transaction, scope).await?;
        let opened = match snapshot {
            CanonicalSnapshotQuery::Missing => OpenChapter::Missing,
            CanonicalSnapshotQuery::Expired => OpenChapter::SnapshotExpired,
            CanonicalSnapshotQuery::Available(snapshot) => {
                match read_scoped_chapter(&transaction, scope, chapter_id).await? {
                    None => OpenChapter::Missing,
                    Some(chapter) => OpenChapter::Found(Box::new(ChapterQueryFacts {
                        project_scope: scope.clone(),
                        snapshot,
                        chapter,
                    })),
                }
            }
        };
        transaction.commit().await.map_err(read_error)?;
        Ok(opened)
    }
}

async fn read_scoped_chapter(
    transaction: &tokio_postgres::Transaction<'_>,
    scope: &ProjectScope,
    chapter_id: &ChapterId,
) -> Result<Option<Chapter>, ProjectReadError> {
    let row = transaction
        .query_opt(
            "SELECT object.manuscript_object_id::text, object.title, \
                    revision.revision_id::text, convert_from(payload.canonical_bytes, 'UTF8'), \
                    COALESCE(counters.project_activity_position, 0)::text \
             FROM storyos.manuscript_objects AS object \
             JOIN storyos.authoritative_heads AS head \
               ON (head.owner_user_id, head.project_id, head.manuscript_object_id) = \
                  (object.owner_user_id, object.project_id, object.manuscript_object_id) \
             JOIN storyos.authoritative_revisions AS revision \
               ON (revision.owner_user_id, revision.project_id, revision.manuscript_object_id, revision.revision_id) = \
                  (head.owner_user_id, head.project_id, head.manuscript_object_id, head.current_revision_id) \
             JOIN storyos.authoritative_payloads AS payload \
               ON (payload.owner_user_id, payload.project_id, payload.payload_id) = \
                  (revision.owner_user_id, revision.project_id, revision.payload_id) \
             LEFT JOIN storyos.scope_counters AS counters \
               ON (counters.owner_user_id, counters.project_id) = \
                  (object.owner_user_id, object.project_id) \
             WHERE object.owner_user_id = $1::text::uuid AND object.project_id = $2::text::uuid \
               AND object.manuscript_object_id = $3::text::uuid AND object.object_kind = 'chapter'",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &chapter_id.as_ref(),
            ],
        )
        .await
        .map_err(read_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let chapter_id = ChapterId::new(row.get::<_, String>(0));
    let title = row.get(1);
    let revision_id = RevisionId::new(row.get::<_, String>(2));
    let stored: String = row.get(3);
    let project_activity_position = row
        .get::<_, String>(4)
        .parse()
        .map_err(ProjectReadError::unavailable)?;
    let blocks = crate::manuscript_block::load_or_upgrade_blocks(
        transaction,
        scope.owner_user_id.as_ref(),
        scope.project_id.as_ref(),
        chapter_id.as_ref(),
        revision_id.as_ref(),
        &stored,
    )
    .await
    .map_err(read_error)?;
    Ok(Some(Chapter {
        chapter_id,
        title,
        revision_id,
        body: crate::manuscript_block::display_body_from_stored(&stored, &blocks),
        blocks,
        project_activity_position,
    }))
}
