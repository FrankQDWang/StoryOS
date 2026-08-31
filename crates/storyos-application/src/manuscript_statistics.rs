use storyos_core::{STATISTICS_COUNTING_PROFILE, count_stored_texts};

use crate::{
    CanonicalSnapshot, ChapterId, ProjectReadError, ProjectScope,
    manuscript_search::{ManuscriptSearchFacts, ManuscriptSearchRead, ManuscriptSearchReader},
};

pub const MANUSCRIPT_STATISTICS_PROJECTION_KIND: &str = "manuscript_statistics";
pub const MANUSCRIPT_STATISTICS_LIMIT_PROFILE_REVISION: &str = "storyos.foundation.absolute.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManuscriptStatisticsRequest {
    pub required_watermark: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChapterStatistics {
    pub chapter_id: ChapterId,
    pub word_count: u64,
    pub character_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManuscriptTotals {
    pub chapter_count: u64,
    pub word_count: u64,
    pub character_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManuscriptStatisticsPage {
    pub project_scope: ProjectScope,
    pub source_snapshot: CanonicalSnapshot,
    pub projection_kind: String,
    pub projection_generation: u64,
    pub projection_watermark: u64,
    pub required_watermark: Option<u64>,
    pub counting_profile: String,
    pub lag: u64,
    pub current_chapter: Option<ChapterStatistics>,
    pub manuscript: ManuscriptTotals,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GetManuscriptStatistics {
    Missing,
    SnapshotExpired,
    ProjectionNotReady {
        source_snapshot: CanonicalSnapshot,
        projection_watermark: u64,
        required_watermark: u64,
    },
    Ready(ManuscriptStatisticsPage),
}

pub async fn get_manuscript_statistics(
    reader: &impl ManuscriptSearchReader,
    scope: &ProjectScope,
    request: &ManuscriptStatisticsRequest,
) -> Result<GetManuscriptStatistics, ProjectReadError> {
    match reader.read_search_facts(scope).await? {
        ManuscriptSearchRead::Missing => Ok(GetManuscriptStatistics::Missing),
        ManuscriptSearchRead::SnapshotExpired => Ok(GetManuscriptStatistics::SnapshotExpired),
        ManuscriptSearchRead::Ready(facts)
            if &facts.project_scope == scope
                && facts
                    .chapters
                    .iter()
                    .all(|chapter| &chapter.project_scope == scope) =>
        {
            Ok(evaluate_statistics(*facts, request))
        }
        ManuscriptSearchRead::Ready(_) => Ok(GetManuscriptStatistics::Missing),
    }
}

fn evaluate_statistics(
    facts: ManuscriptSearchFacts,
    request: &ManuscriptStatisticsRequest,
) -> GetManuscriptStatistics {
    let projection_watermark = facts.snapshot.project_activity_position;
    if let Some(required) = request.required_watermark
        && required > projection_watermark
    {
        return GetManuscriptStatistics::ProjectionNotReady {
            source_snapshot: facts.snapshot,
            projection_watermark,
            required_watermark: required,
        };
    }
    let mut manuscript = ManuscriptTotals {
        chapter_count: 0,
        word_count: 0,
        character_count: 0,
    };
    let mut current_chapter = None;
    for chapter in &facts.chapters {
        let counted = count_stored_texts(chapter.blocks.iter().map(|block| block.text.as_str()));
        manuscript.chapter_count = manuscript.chapter_count.saturating_add(1);
        manuscript.word_count = manuscript.word_count.saturating_add(counted.word_count);
        manuscript.character_count = manuscript
            .character_count
            .saturating_add(counted.character_count);
        if facts.current_chapter_id.as_ref() == Some(&chapter.chapter_id) {
            current_chapter = Some(ChapterStatistics {
                chapter_id: chapter.chapter_id.clone(),
                word_count: counted.word_count,
                character_count: counted.character_count,
            });
        }
    }
    GetManuscriptStatistics::Ready(ManuscriptStatisticsPage {
        project_scope: facts.project_scope,
        source_snapshot: facts.snapshot,
        projection_kind: MANUSCRIPT_STATISTICS_PROJECTION_KIND.to_owned(),
        projection_generation: projection_watermark,
        projection_watermark,
        required_watermark: request.required_watermark,
        counting_profile: STATISTICS_COUNTING_PROFILE.to_owned(),
        lag: 0,
        current_chapter,
        manuscript,
    })
}

#[cfg(test)]
#[path = "manuscript_statistics_tests.rs"]
mod tests;
