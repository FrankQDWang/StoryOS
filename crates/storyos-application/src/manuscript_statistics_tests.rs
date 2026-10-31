use super::{GetManuscriptStatistics, ManuscriptStatisticsRequest, get_manuscript_statistics};
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
