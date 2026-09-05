use std::future::Future;

use crate::{CanonicalSnapshot, ChapterId, ProjectReadError, ProjectScope};

pub const MANUSCRIPT_SEARCH_LIMIT_PROFILE_REVISION: &str = "storyos.foundation.absolute.v1";
pub(super) const MANUSCRIPT_SEARCH_PROJECTION_KIND: &str = "manuscript_search";
pub(super) const MANUSCRIPT_SEARCH_PAGE_ITEM_LIMIT: usize = 500;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManuscriptSearchSelection {
    CurrentChapter,
    Manuscript,
}

/// Internal live-Chapter fact extent for one search or statistics rebuild.
///
/// This is not a public request field. Application maps
/// [`ManuscriptSearchSelection::CurrentChapter`] to [`Self::CurrentChapter`]
/// and maps manuscript search plus writing statistics to [`Self::AllChapters`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManuscriptSearchFactExtent {
    AllChapters,
    CurrentChapter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManuscriptSearchRequest {
    pub query_text: String,
    pub selection: ManuscriptSearchSelection,
    pub required_watermark: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManuscriptSearchBlockFact {
    pub manuscript_block_id: String,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManuscriptSearchChapterFact {
    pub project_scope: ProjectScope,
    pub chapter_id: ChapterId,
    pub volume_order: u64,
    pub chapter_order: u64,
    pub blocks: Vec<ManuscriptSearchBlockFact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManuscriptSearchFacts {
    pub project_scope: ProjectScope,
    pub snapshot: CanonicalSnapshot,
    pub current_chapter_id: Option<ChapterId>,
    pub chapters: Vec<ManuscriptSearchChapterFact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManuscriptSearchRead {
    Missing,
    SnapshotExpired,
    Ready(Box<ManuscriptSearchFacts>),
}

/// Reads canonical Block facts for one bounded manuscript search under an already
/// authenticated exact Project Scope.
///
/// Implementations rebuild from live canonical Heads. They must omit removed and
/// foreign-Scope material and must not write. [`ManuscriptSearchFactExtent::CurrentChapter`]
/// must load at most the current Chapter observed in the same read.
pub trait ManuscriptSearchReader: Sync {
    fn read_search_facts(
        &self,
        scope: &ProjectScope,
        fact_extent: ManuscriptSearchFactExtent,
    ) -> impl Future<Output = Result<ManuscriptSearchRead, ProjectReadError>> + Send;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManuscriptSearchMatch {
    pub chapter_id: ChapterId,
    pub manuscript_block_id: String,
    pub start: u64,
    pub end: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManuscriptSearchCompleteness {
    Complete,
    Truncated,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManuscriptSearchPage {
    pub project_scope: ProjectScope,
    pub source_snapshot: CanonicalSnapshot,
    pub projection_kind: String,
    pub projection_generation: u64,
    pub projection_watermark: u64,
    pub required_watermark: Option<u64>,
    pub completeness: ManuscriptSearchCompleteness,
    pub lag: u64,
    pub items: Vec<ManuscriptSearchMatch>,
    pub page_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchManuscript {
    Missing,
    SnapshotExpired,
    ProjectionNotReady {
        source_snapshot: CanonicalSnapshot,
        projection_watermark: u64,
        required_watermark: u64,
    },
    Ready(ManuscriptSearchPage),
}

pub async fn search_manuscript(
    reader: &impl ManuscriptSearchReader,
    scope: &ProjectScope,
    request: &ManuscriptSearchRequest,
) -> Result<SearchManuscript, ProjectReadError> {
    let fact_extent = match request.selection {
        ManuscriptSearchSelection::CurrentChapter => ManuscriptSearchFactExtent::CurrentChapter,
        ManuscriptSearchSelection::Manuscript => ManuscriptSearchFactExtent::AllChapters,
    };
    match reader.read_search_facts(scope, fact_extent).await? {
        ManuscriptSearchRead::Missing => Ok(SearchManuscript::Missing),
        ManuscriptSearchRead::SnapshotExpired => Ok(SearchManuscript::SnapshotExpired),
        ManuscriptSearchRead::Ready(facts)
            if &facts.project_scope == scope
                && facts
                    .chapters
                    .iter()
                    .all(|chapter| &chapter.project_scope == scope) =>
        {
            Ok(evaluate_search(*facts, request))
        }
        ManuscriptSearchRead::Ready(_) => Ok(SearchManuscript::Missing),
    }
}

fn evaluate_search(
    facts: ManuscriptSearchFacts,
    request: &ManuscriptSearchRequest,
) -> SearchManuscript {
    let projection_watermark = facts.snapshot.project_activity_position;
    if let Some(required) = request.required_watermark
        && required > projection_watermark
    {
        return SearchManuscript::ProjectionNotReady {
            source_snapshot: facts.snapshot,
            projection_watermark,
            required_watermark: required,
        };
    }
    let mut chapters: Vec<&ManuscriptSearchChapterFact> = match request.selection {
        ManuscriptSearchSelection::Manuscript => facts.chapters.iter().collect(),
        ManuscriptSearchSelection::CurrentChapter => facts
            .current_chapter_id
            .as_ref()
            .map(|current| {
                facts
                    .chapters
                    .iter()
                    .filter(|chapter| &chapter.chapter_id == current)
                    .collect()
            })
            .unwrap_or_default(),
    };
    chapters.sort_by_key(|chapter| (chapter.volume_order, chapter.chapter_order));
    let mut items = Vec::new();
    let mut truncated = false;
    if !request.query_text.is_empty() {
        let query_utf16 = utf16_len(&request.query_text);
        for chapter in chapters {
            for block in &chapter.blocks {
                let mut byte = 0;
                let mut utf16 = 0;
                while let Some(found) = block.text[byte..].find(&request.query_text) {
                    if items.len() == MANUSCRIPT_SEARCH_PAGE_ITEM_LIMIT {
                        truncated = true;
                        break;
                    }
                    let start_byte = byte + found;
                    utf16 += utf16_len(&block.text[byte..start_byte]);
                    items.push(ManuscriptSearchMatch {
                        chapter_id: chapter.chapter_id.clone(),
                        manuscript_block_id: block.manuscript_block_id.clone(),
                        start: utf16,
                        end: utf16 + query_utf16,
                    });
                    byte = start_byte + request.query_text.len();
                    utf16 += query_utf16;
                }
                if truncated {
                    break;
                }
            }
            if truncated {
                break;
            }
        }
    }
    let page_count = items.len() as u64;
    SearchManuscript::Ready(ManuscriptSearchPage {
        project_scope: facts.project_scope,
        source_snapshot: facts.snapshot,
        projection_kind: MANUSCRIPT_SEARCH_PROJECTION_KIND.to_owned(),
        projection_generation: projection_watermark,
        projection_watermark,
        required_watermark: request.required_watermark,
        completeness: if truncated {
            ManuscriptSearchCompleteness::Truncated
        } else {
            ManuscriptSearchCompleteness::Complete
        },
        lag: 0,
        items,
        page_count,
    })
}

#[cfg(test)]
mod utf16_decode_count {
    use std::cell::Cell;

    thread_local! {
        static COUNT: Cell<u64> = const { Cell::new(0) };
    }

    pub fn reset() {
        COUNT.with(|count| count.set(0));
    }

    pub fn add(units: u64) {
        COUNT.with(|count| count.set(count.get() + units));
    }

    pub fn take() -> u64 {
        COUNT.with(|count| count.replace(0))
    }
}

fn utf16_len(text: &str) -> u64 {
    let count = u64::try_from(text.encode_utf16().count()).unwrap_or(u64::MAX);
    #[cfg(test)]
    utf16_decode_count::add(count);
    count
}

#[cfg(test)]
#[path = "manuscript_search_tests.rs"]
mod tests;
