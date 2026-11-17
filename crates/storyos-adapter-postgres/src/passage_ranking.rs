//! The read of the ranked passages of the manuscript (ADR 0095 sections 1 and 2). One read-only
//! transaction reads the authoritative head of each live Chapter and the nearest vector rows of
//! the current passages. The vector read is an exact scan in the Project Scope, with no vector
//! index (the record of S7-C1). The Core cut decides which passages are current.

use std::collections::BTreeSet;

use storyos_application::{
    PassageRankingFacts, PassageRankingStore, ProjectReadError, ProjectScope, QuestionVector,
};
use storyos_core::{
    NearestPassage, PASSAGE_CUT_REVISION, PASSAGE_LIST_LIMIT, RankingBlock, RankingChapter,
    current_passages,
};
use tokio_postgres::GenericClient;

use crate::manuscript_search::{LiveChapterReadExtent, read_live_chapter_blocks};
use crate::passage_vectors::vector_text;
use crate::{PostgresProjectReader, read_error, set_scope};

/// The nearest vector rows of the current passages `$3`/`$4` (Chapter, digest) of the Project, for
/// the Registration `$5` and the cut revision `$6`, by the cosine distance to `$7`, at most `$8`.
/// Equal distances go in the order of the Chapter and of the digest.
pub(crate) const NEAREST_PASSAGES_SQL: &str =
    "SELECT vector.chapter_id::text, vector.passage_digest
       FROM storyos.passage_vectors AS vector
       JOIN unnest($3::text[], $4::text[]) AS current(chapter_id, passage_digest)
         ON vector.chapter_id = current.chapter_id::uuid
        AND vector.passage_digest = current.passage_digest
      WHERE vector.owner_user_id = $1::text::uuid AND vector.project_id = $2::text::uuid
        AND vector.embedding_registration_revision = $5::text::uuid
        AND vector.cut_revision = $6
      ORDER BY vector.embedding OPERATOR(extensions.<=>) $7::text::extensions.vector,
               vector.chapter_id, vector.passage_digest
      LIMIT $8";

impl PassageRankingStore for PostgresProjectReader {
    async fn read_passage_ranking_facts(
        &self,
        scope: &ProjectScope,
        question: Option<QuestionVector<'_>>,
    ) -> Result<Option<PassageRankingFacts>, ProjectReadError> {
        let client = self.connect().await?;
        client
            .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .await
            .map_err(read_error)?;
        let outcome = read_facts(&*client, scope, question).await;
        match &outcome {
            Ok(_) => client.batch_execute("COMMIT").await.map_err(read_error)?,
            Err(_) => {
                let _rollback = client.batch_execute("ROLLBACK").await;
            }
        }
        outcome
    }
}

async fn read_facts(
    client: &impl GenericClient,
    scope: &ProjectScope,
    question: Option<QuestionVector<'_>>,
) -> Result<Option<PassageRankingFacts>, ProjectReadError> {
    set_scope(client, scope).await?;
    let (owner, project) = (scope.owner_user_id.as_ref(), scope.project_id.as_ref());
    let visible = client
        .query_opt(
            "SELECT 1 FROM storyos.projects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&owner, &project],
        )
        .await
        .map_err(read_error)?;
    if visible.is_none() {
        return Ok(None);
    }
    let chapters: Vec<RankingChapter> =
        read_live_chapter_blocks(client, scope, LiveChapterReadExtent::AllChapters)
            .await?
            .into_iter()
            .map(|chapter| RankingChapter {
                chapter_id: chapter.chapter_id.as_ref().to_owned(),
                title: chapter.title,
                blocks: (chapter.blocks.into_iter())
                    .map(|block| RankingBlock {
                        block_id: block.manuscript_block_id,
                        text: block.text,
                    })
                    .collect(),
            })
            .collect();
    let chapters = current_passages(&chapters);
    let available: bool = client
        .query_one(
            "SELECT to_regclass('storyos.passage_vectors') IS NOT NULL",
            &[],
        )
        .await
        .map_err(read_error)?
        .get(/*idx*/ 0);
    let nearest = match (question, available) {
        (Some(question), true) => {
            let current: BTreeSet<(&str, &str)> = (chapters.iter())
                .flat_map(|chapter| {
                    (chapter.passages.iter())
                        .map(|passage| (chapter.chapter_id.as_str(), passage.digest.as_str()))
                })
                .collect();
            let (chapter_ids, digests): (Vec<&str>, Vec<&str>) = current.into_iter().unzip();
            let limit = i64::try_from(PASSAGE_LIST_LIMIT).unwrap_or(i64::MAX);
            client
                .query(
                    NEAREST_PASSAGES_SQL,
                    &[
                        &owner,
                        &project,
                        &chapter_ids,
                        &digests,
                        &question.registration_revision,
                        &PASSAGE_CUT_REVISION,
                        &vector_text(question.values),
                        &limit,
                    ],
                )
                .await
                .map_err(read_error)?
                .iter()
                .map(|row| NearestPassage {
                    chapter_id: row.get(/*idx*/ 0),
                    passage_digest: row.get(/*idx*/ 1),
                })
                .collect()
        }
        _ => Vec::new(),
    };
    Ok(Some(PassageRankingFacts { chapters, nearest }))
}

#[cfg(test)]
#[path = "passage_ranking_tests.rs"]
mod tests;
