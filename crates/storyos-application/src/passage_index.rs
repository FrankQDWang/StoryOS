//! The passage vectors of one Chapter (ADR 0095 sections 2 and 4). The vectors are a Retrieval
//! Index: each row is a fact about the text of its passage, so a row stays correct when the
//! Chapter changes, and the next call removes each row that is not a current passage.

use std::collections::BTreeSet;
use std::future::Future;

use storyos_core::{
    EmbeddingInput, PASSAGE_CUT_REVISION, PassageBlock, cut_passages, passage_embedding_text,
};

use crate::{ManuscriptSearchBlockFact, ProjectReadError, ProjectScope};

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
    fn read_passage_index_facts(
        &self,
        scope: &ProjectScope,
        chapter_id: &str,
    ) -> impl Future<Output = Result<PassageIndexFacts, ProjectReadError>> + Send;
}

/// The distinct current passages of a Chapter with their keys, in passage order, and the stored
/// keys that are not a current passage.
pub(crate) struct PassagePlan {
    pub(crate) current: Vec<(PassageVectorKey, String)>,
    pub(crate) stale: Vec<PassageVectorKey>,
}

pub(crate) fn plan_passages(
    chapter: &ChapterPassageSource,
    registration_revision: &str,
    stored: &[PassageVectorKey],
) -> PassagePlan {
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
                embedding_registration_revision: registration_revision.to_owned(),
                cut_revision: PASSAGE_CUT_REVISION.to_owned(),
                passage_digest: passage.digest,
            };
            (key, passage.text)
        })
        .collect();
    let current_keys: BTreeSet<&PassageVectorKey> = current.iter().map(|(key, _)| key).collect();
    let stale = stored
        .iter()
        .filter(|key| !current_keys.contains(key))
        .cloned()
        .collect();
    PassagePlan { current, stale }
}

/// The embedding input of one passage of the Chapter.
pub(crate) fn passage_input(
    chapter: &ChapterPassageSource,
    chapter_id: &str,
    key: &PassageVectorKey,
    text: &str,
) -> EmbeddingInput {
    EmbeddingInput::Passage {
        chapter_id: chapter_id.to_owned(),
        revision_id: chapter.head_revision_id.clone(),
        passage_digest: key.passage_digest.clone(),
        text: passage_embedding_text(&chapter.volume_title, &chapter.title, text),
    }
}
