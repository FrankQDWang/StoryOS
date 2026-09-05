use super::{
    ChapterStatistics, GetManuscriptStatistics, MANUSCRIPT_STATISTICS_PROJECTION_KIND,
    ManuscriptStatisticsPage, ManuscriptStatisticsRequest, ManuscriptTotals,
    get_manuscript_statistics,
};
use crate::manuscript_search::{
    ManuscriptSearchBlockFact, ManuscriptSearchChapterFact, ManuscriptSearchFactExtent,
    ManuscriptSearchFacts, ManuscriptSearchRead, ManuscriptSearchReader,
};
use crate::{CanonicalSnapshot, ChapterId, ProjectId, ProjectReadError, ProjectScope, UserId};

struct FixtureReader {
    read: ManuscriptSearchRead,
}

impl ManuscriptSearchReader for FixtureReader {
    async fn read_search_facts(
        &self,
        _scope: &ProjectScope,
        _fact_extent: ManuscriptSearchFactExtent,
    ) -> Result<ManuscriptSearchRead, ProjectReadError> {
        Ok(self.read.clone())
    }
}

fn owned_scope() -> ProjectScope {
    ProjectScope::new(UserId::new("user-a"), ProjectId::new("project-a"))
}

fn snapshot() -> CanonicalSnapshot {
    CanonicalSnapshot {
        snapshot_id: "snapshot-statistics".to_owned(),
        project_activity_position: 4,
        replay_generation: 1,
        floor_position: 0,
        redaction_profile: "storyos.author.v1".to_owned(),
        schema_profile: "storyos.public.release.1".to_owned(),
        created_at: "2026-08-31T00:00:00.000Z".to_owned(),
        expires_at: None,
    }
}

fn chapter(
    id: &str,
    volume_order: u64,
    chapter_order: u64,
    text: &str,
) -> ManuscriptSearchChapterFact {
    ManuscriptSearchChapterFact {
        project_scope: owned_scope(),
        chapter_id: ChapterId::new(id),
        volume_order,
        chapter_order,
        blocks: vec![ManuscriptSearchBlockFact {
            manuscript_block_id: format!("{id}-block"),
            text: text.to_owned(),
        }],
    }
}

fn ready(chapters: Vec<ManuscriptSearchChapterFact>) -> ManuscriptSearchRead {
    ManuscriptSearchRead::Ready(Box::new(ManuscriptSearchFacts {
        project_scope: owned_scope(),
        snapshot: snapshot(),
        current_chapter_id: Some(ChapterId::new("chapter-b")),
        chapters,
    }))
}

#[tokio::test]
async fn chapter_and_manuscript_counts_use_the_same_unicode_profile() {
    let scope = owned_scope();
    let reader = FixtureReader {
        read: ready(vec![
            chapter(
                "chapter-a",
                /*volume_order*/ 1,
                /*chapter_order*/ 1,
                "Hello world",
            ),
            chapter(
                "chapter-b",
                /*volume_order*/ 1,
                /*chapter_order*/ 2,
                "雨落在窗沿。",
            ),
        ]),
    };

    let result = get_manuscript_statistics(
        &reader,
        &scope,
        &ManuscriptStatisticsRequest {
            required_watermark: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(
        result,
        GetManuscriptStatistics::Ready(ManuscriptStatisticsPage {
            project_scope: scope,
            source_snapshot: snapshot(),
            projection_kind: MANUSCRIPT_STATISTICS_PROJECTION_KIND.to_owned(),
            projection_generation: 4,
            projection_watermark: 4,
            required_watermark: None,
            counting_profile: storyos_core::STATISTICS_COUNTING_PROFILE.to_owned(),
            lag: 0,
            current_chapter: Some(ChapterStatistics {
                chapter_id: ChapterId::new("chapter-b"),
                word_count: 1,
                character_count: 6,
            }),
            manuscript: ManuscriptTotals {
                chapter_count: 2,
                word_count: 3,
                character_count: 17,
            },
        })
    );
}

#[tokio::test]
async fn unmet_required_watermark_is_not_an_empty_success() {
    let scope = owned_scope();
    let reader = FixtureReader {
        read: ready(vec![chapter(
            "chapter-b",
            /*volume_order*/ 1,
            /*chapter_order*/ 1,
            "Hello",
        )]),
    };

    let result = get_manuscript_statistics(
        &reader,
        &scope,
        &ManuscriptStatisticsRequest {
            required_watermark: Some(9),
        },
    )
    .await
    .unwrap();

    assert_eq!(
        result,
        GetManuscriptStatistics::ProjectionNotReady {
            source_snapshot: snapshot(),
            projection_watermark: 4,
            required_watermark: 9,
        }
    );
}

#[tokio::test]
async fn omitted_chapters_do_not_contribute_invented_counts() {
    let scope = owned_scope();
    let reader = FixtureReader {
        read: ready(vec![chapter(
            "chapter-b",
            /*volume_order*/ 1,
            /*chapter_order*/ 1,
            "Hello",
        )]),
    };

    let GetManuscriptStatistics::Ready(page) = get_manuscript_statistics(
        &reader,
        &scope,
        &ManuscriptStatisticsRequest {
            required_watermark: None,
        },
    )
    .await
    .unwrap() else {
        panic!("ready page");
    };
    assert_eq!(
        page.manuscript,
        ManuscriptTotals {
            chapter_count: 1,
            word_count: 1,
            character_count: 5,
        }
    );
}
