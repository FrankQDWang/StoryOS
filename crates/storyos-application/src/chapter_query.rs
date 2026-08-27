use std::future::Future;

use crate::{CanonicalSnapshot, Chapter, ChapterId, ProjectReadError, ProjectScope};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChapterQueryFacts {
    pub project_scope: ProjectScope,
    pub snapshot: CanonicalSnapshot,
    pub chapter: Chapter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpenChapter {
    Found(Box<ChapterQueryFacts>),
    Missing,
    SnapshotExpired,
}

/// Reads one Chapter Canonical Query under an already authenticated exact Project Scope.
///
/// Implementations return the Chapter Head and its bound Canonical Query Snapshot,
/// or a closed missing/expired outcome. They must not invent a Chapter from a
/// current-Chapter pointer, a cache, or a foreign Scope join.
pub trait ChapterQueryReader: Sync {
    fn read_chapter_query(
        &self,
        scope: &ProjectScope,
        chapter_id: &ChapterId,
    ) -> impl Future<Output = Result<OpenChapter, ProjectReadError>> + Send;
}

pub async fn open_chapter(
    reader: &impl ChapterQueryReader,
    scope: &ProjectScope,
    chapter_id: &ChapterId,
) -> Result<OpenChapter, ProjectReadError> {
    match reader.read_chapter_query(scope, chapter_id).await? {
        OpenChapter::Found(facts)
            if &facts.project_scope == scope && facts.chapter.chapter_id == *chapter_id =>
        {
            Ok(OpenChapter::Found(facts))
        }
        OpenChapter::SnapshotExpired => Ok(OpenChapter::SnapshotExpired),
        OpenChapter::Found(_) | OpenChapter::Missing => Ok(OpenChapter::Missing),
    }
}

#[cfg(test)]
#[path = "chapter_query_tests.rs"]
mod tests;
