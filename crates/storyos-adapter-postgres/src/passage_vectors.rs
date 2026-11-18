//! The passage vectors of the manuscript (ADR 0095 sections 2 and 4): the vector table of
//! `migrations/0002_passage_vectors.sql`. The rows are a Retrieval Index in the Project Scope.
//! They hold no text, no block identity, and no position.

use storyos_application::{
    ChapterId, ChapterPassageSource, PassageIndexFacts, PassageVectorKey, PassageVectorRow,
    PassageVectorStore, ProjectReadError, ProjectScope,
};

use crate::PostgresProjectReader;
use crate::manuscript_search::{LiveChapterReadExtent, read_live_chapter_blocks};

fn read_error(error: impl std::error::Error + Send + Sync + 'static) -> ProjectReadError {
    ProjectReadError::unavailable(error)
}

/// The text form of a pgvector value.
pub(crate) fn vector_text(embedding: &[f32]) -> String {
    let values: Vec<String> = embedding.iter().map(f32::to_string).collect();
    format!("[{}]", values.join(","))
}

impl PassageVectorStore for PostgresProjectReader {
    async fn vector_store_available(&self, scope: &ProjectScope) -> Result<bool, ProjectReadError> {
        let transaction = self
            .begin_serializable_project_command_transaction(scope)
            .await
            .map_err(read_error)?;
        let available: bool = transaction
            .client
            .query_one(
                "SELECT to_regclass('storyos.passage_vectors') IS NOT NULL",
                &[],
            )
            .await
            .map_err(read_error)?
            .get(/*idx*/ 0);
        transaction.commit().await.map_err(read_error)?;
        Ok(available)
    }

    async fn read_passage_index_facts(
        &self,
        scope: &ProjectScope,
        chapter_id: &str,
    ) -> Result<PassageIndexFacts, ProjectReadError> {
        let transaction = self
            .begin_serializable_project_command_transaction(scope)
            .await
            .map_err(read_error)?;
        let chapter_id = ChapterId::new(chapter_id.to_owned());
        let chapter = read_live_chapter_blocks(
            &*transaction.client,
            scope,
            LiveChapterReadExtent::CurrentChapter {
                chapter_id: &chapter_id,
            },
        )
        .await?
        .into_iter()
        .next()
        .map(|chapter| ChapterPassageSource {
            head_revision_id: chapter.head_revision_id,
            volume_title: chapter.volume_title,
            title: chapter.title,
            blocks: chapter.blocks,
        });
        let stored = transaction
            .client
            .query(
                "SELECT embedding_registration_revision::text, cut_revision, passage_digest
                   FROM storyos.passage_vectors
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND chapter_id = $3::text::uuid
                  ORDER BY 1, 2, 3",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &chapter_id.as_ref(),
                ],
            )
            .await
            .map_err(read_error)?
            .iter()
            .map(|row| PassageVectorKey {
                embedding_registration_revision: row.get(/*idx*/ 0),
                cut_revision: row.get(/*idx*/ 1),
                passage_digest: row.get(/*idx*/ 2),
            })
            .collect();
        transaction.commit().await.map_err(read_error)?;
        Ok(PassageIndexFacts { chapter, stored })
    }

    async fn remove_passage_vectors(
        &self,
        scope: &ProjectScope,
        chapter_id: &str,
        keys: &[PassageVectorKey],
    ) -> Result<u64, ProjectReadError> {
        if keys.is_empty() {
            return Ok(0);
        }
        let transaction = self
            .begin_serializable_project_command_transaction(scope)
            .await
            .map_err(read_error)?;
        let removed = delete_passage_vectors(&transaction.client, scope, chapter_id, keys).await?;
        transaction.commit().await.map_err(read_error)?;
        Ok(removed)
    }

    async fn write_passage_vector(
        &self,
        scope: &ProjectScope,
        row: &PassageVectorRow<'_>,
    ) -> Result<(), ProjectReadError> {
        let transaction = self
            .begin_serializable_project_command_transaction(scope)
            .await
            .map_err(read_error)?;
        insert_passage_vector(&transaction.client, scope, row).await?;
        transaction.commit().await.map_err(read_error)?;
        Ok(())
    }
}

/// Deletes the rows of `keys` of the Chapter, and returns the number of deleted rows.
pub(crate) async fn delete_passage_vectors(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    chapter_id: &str,
    keys: &[PassageVectorKey],
) -> Result<u64, ProjectReadError> {
    let column =
        |value: fn(&PassageVectorKey) -> &str| -> Vec<&str> { keys.iter().map(value).collect() };
    client
        .execute(
            "DELETE FROM storyos.passage_vectors AS vector
              USING unnest($4::text[], $5::text[], $6::text[])
                    AS stale(registration_revision, cut_revision, passage_digest)
              WHERE vector.owner_user_id = $1::text::uuid
                AND vector.project_id = $2::text::uuid
                AND vector.chapter_id = $3::text::uuid
                AND vector.embedding_registration_revision = stale.registration_revision::uuid
                AND vector.cut_revision = stale.cut_revision
                AND vector.passage_digest = stale.passage_digest",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &chapter_id,
                &column(|key| &key.embedding_registration_revision),
                &column(|key| &key.cut_revision),
                &column(|key| &key.passage_digest),
            ],
        )
        .await
        .map_err(read_error)
}

/// Writes one row. A row with the same key stays as it is.
pub(crate) async fn insert_passage_vector(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    row: &PassageVectorRow<'_>,
) -> Result<(), ProjectReadError> {
    client
        .execute(
            "INSERT INTO storyos.passage_vectors (
               owner_user_id, project_id, chapter_id, embedding_registration_revision,
               cut_revision, passage_digest, embedding, created_at
             ) VALUES (
               $1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5, $6,
               $7::text::extensions.vector, clock_timestamp()
             ) ON CONFLICT DO NOTHING",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &row.chapter_id,
                &row.key.embedding_registration_revision,
                &row.key.cut_revision,
                &row.key.passage_digest,
                &vector_text(row.embedding),
            ],
        )
        .await
        .map(|_| ())
        .map_err(read_error)
}

#[cfg(test)]
#[path = "passage_vectors_tests.rs"]
pub(crate) mod tests;
