//! The passage vectors of one Chapter (ADR 0095 sections 2 and 4). The vectors are a Retrieval
//! Index: each row is a fact about the text of its passage, so a row stays correct when the
//! Chapter changes, and the next call removes each row that is not a current passage.

use std::collections::BTreeSet;
use std::future::Future;

use storyos_core::{
    EmbeddingInput, EmbeddingRegistration, PASSAGE_CUT_REVISION, PassageBlock, cut_passages,
    passage_embedding_text,
};

use crate::{
    EmbeddingDispatchStore, EmbeddingProviderAdapter, EmbeddingResult, ManuscriptSearchBlockFact,
    ProjectReadError, ProjectScope, dispatch_embedding,
};

/// The current authoritative text of one live Chapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChapterPassageSource {
    pub head_revision_id: String,
    pub volume_title: String,
    pub title: String,
    pub blocks: Vec<ManuscriptSearchBlockFact>,
}

/// The key of one stored passage vector of a Chapter.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PassageVectorKey {
    pub embedding_registration_revision: String,
    pub cut_revision: String,
    pub passage_digest: String,
}

/// What the store reads in one transaction for one Chapter: its text when it is live in the
/// Project Scope, and the keys of its stored rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassageIndexFacts {
    pub chapter: Option<ChapterPassageSource>,
    pub stored: Vec<PassageVectorKey>,
}

/// One row to write.
#[derive(Clone, Debug, PartialEq)]
pub struct PassageVectorRow<'a> {
    pub chapter_id: &'a str,
    pub key: &'a PassageVectorKey,
    pub embedding: &'a [f32],
}

/// The persistence of the passage vectors. Each operation is one committed transaction in the
/// Project Scope.
pub trait PassageVectorStore: Sync {
    /// If the vector store is available: the database has the vector table.
    fn vector_store_available(
        &self,
        scope: &ProjectScope,
    ) -> impl Future<Output = Result<bool, ProjectReadError>> + Send;

    fn read_passage_index_facts(
        &self,
        scope: &ProjectScope,
        chapter_id: &str,
    ) -> impl Future<Output = Result<PassageIndexFacts, ProjectReadError>> + Send;

    /// Deletes the rows of `keys` of the Chapter, and returns the number of deleted rows.
    fn remove_passage_vectors(
        &self,
        scope: &ProjectScope,
        chapter_id: &str,
        keys: &[PassageVectorKey],
    ) -> impl Future<Output = Result<u64, ProjectReadError>> + Send;

    /// Writes one row. A row with the same key stays as it is.
    fn write_passage_vector(
        &self,
        scope: &ProjectScope,
        row: &PassageVectorRow<'_>,
    ) -> impl Future<Output = Result<(), ProjectReadError>> + Send;
}

/// One passage whose embedding request gave no vector. Its row is not written.
#[derive(Clone, Debug, PartialEq)]
pub struct PassageIndexFailure {
    pub passage_digest: String,
    pub result: EmbeddingResult,
}

/// The counts of one call for a live Chapter.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PassageIndexReport {
    /// The distinct current passages.
    pub passages: usize,
    /// The current passages that already had a row, with no request.
    pub unchanged: usize,
    pub written: usize,
    pub removed: u64,
    pub failed: Vec<PassageIndexFailure>,
}

/// The typed result of one call.
#[derive(Clone, Debug, PartialEq)]
pub enum PassageIndexResult {
    /// The deployment has no embedding configuration. The call changed nothing.
    NotConfigured,
    /// The database has no vector table. The call changed nothing.
    VectorStoreUnavailable,
    /// The Chapter is not live in the Project Scope. The call deleted each row of the Chapter.
    ChapterNotLive {
        removed: u64,
    },
    Indexed(PassageIndexReport),
}

/// Makes the rows of the Chapter equal to its current passages for `registration` and the current
/// cut revision. It sends one embedding request with the cause `passage_index` for each passage
/// with no row, and no request for a passage with a row. A refused or failed request writes no row.
pub async fn index_chapter_passages(
    store: &(impl PassageVectorStore + EmbeddingDispatchStore),
    adapter: &impl EmbeddingProviderAdapter,
    registration: Option<&'static EmbeddingRegistration>,
    scope: &ProjectScope,
    chapter_id: &str,
) -> Result<PassageIndexResult, ProjectReadError> {
    let Some(configured) = registration else {
        return Ok(PassageIndexResult::NotConfigured);
    };
    if !store.vector_store_available(scope).await? {
        return Ok(PassageIndexResult::VectorStoreUnavailable);
    }
    let facts = store.read_passage_index_facts(scope, chapter_id).await?;
    let Some(chapter) = facts.chapter else {
        let removed = store
            .remove_passage_vectors(scope, chapter_id, &facts.stored)
            .await?;
        return Ok(PassageIndexResult::ChapterNotLive { removed });
    };
    let blocks: Vec<PassageBlock<'_>> = chapter
        .blocks
        .iter()
        .map(|block| PassageBlock {
            block_id: &block.manuscript_block_id,
            text: &block.text,
        })
        .collect();
    let mut seen = BTreeSet::new();
    let current: Vec<_> = cut_passages(&blocks)
        .into_iter()
        .filter(|passage| seen.insert(passage.digest.clone()))
        .map(|passage| {
            let key = PassageVectorKey {
                embedding_registration_revision: configured.revision.to_owned(),
                cut_revision: PASSAGE_CUT_REVISION.to_owned(),
                passage_digest: passage.digest,
            };
            (key, passage.text)
        })
        .collect();
    let stored: BTreeSet<&PassageVectorKey> = facts.stored.iter().collect();
    let current_keys: BTreeSet<&PassageVectorKey> = current.iter().map(|(key, _)| key).collect();
    let stale: Vec<PassageVectorKey> = facts
        .stored
        .iter()
        .filter(|key| !current_keys.contains(key))
        .cloned()
        .collect();
    let mut report = PassageIndexReport {
        passages: current.len(),
        removed: store
            .remove_passage_vectors(scope, chapter_id, &stale)
            .await?,
        ..PassageIndexReport::default()
    };
    for (key, text) in &current {
        if stored.contains(key) {
            report.unchanged += 1;
            continue;
        }
        let input = EmbeddingInput::Passage {
            chapter_id: chapter_id.to_owned(),
            revision_id: chapter.head_revision_id.clone(),
            passage_digest: key.passage_digest.clone(),
            text: passage_embedding_text(&chapter.volume_title, &chapter.title, text),
        };
        match dispatch_embedding(store, adapter, Some(configured), scope, &input).await? {
            EmbeddingResult::Vector(embedding) => {
                store
                    .write_passage_vector(
                        scope,
                        &PassageVectorRow {
                            chapter_id,
                            key,
                            embedding: &embedding,
                        },
                    )
                    .await?;
                report.written += 1;
            }
            result => report.failed.push(PassageIndexFailure {
                passage_digest: key.passage_digest.clone(),
                result,
            }),
        }
    }
    Ok(PassageIndexResult::Indexed(report))
}
