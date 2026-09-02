use super::{
    MANUSCRIPT_SEARCH_PAGE_ITEM_LIMIT, MANUSCRIPT_SEARCH_PROJECTION_KIND,
    ManuscriptSearchBlockFact, ManuscriptSearchChapterFact, ManuscriptSearchCompleteness,
    ManuscriptSearchFacts, ManuscriptSearchMatch, ManuscriptSearchPage, ManuscriptSearchRead,
    ManuscriptSearchReader, ManuscriptSearchRequest, ManuscriptSearchSelection, SearchManuscript,
    search_manuscript,
};
use crate::{CanonicalSnapshot, ChapterId, ProjectId, ProjectReadError, ProjectScope, UserId};

struct FixtureReader {
    read: ManuscriptSearchRead,
}

impl ManuscriptSearchReader for FixtureReader {
    async fn read_search_facts(
        &self,
        _scope: &ProjectScope,
    ) -> Result<ManuscriptSearchRead, ProjectReadError> {
        Ok(self.read.clone())
    }
}

fn owned_scope() -> ProjectScope {
    ProjectScope::new(UserId::new("user-a"), ProjectId::new("project-a"))
}

fn snapshot() -> CanonicalSnapshot {
    CanonicalSnapshot {
        snapshot_id: "snapshot-search".to_owned(),
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
    block_id: &str,
    text: &str,
) -> ManuscriptSearchChapterFact {
    ManuscriptSearchChapterFact {
        project_scope: owned_scope(),
        chapter_id: ChapterId::new(id),
        volume_order,
        chapter_order,
        blocks: vec![ManuscriptSearchBlockFact {
            manuscript_block_id: block_id.to_owned(),
            text: text.to_owned(),
        }],
    }
}

fn facts(chapters: Vec<ManuscriptSearchChapterFact>) -> ManuscriptSearchFacts {
    ManuscriptSearchFacts {
        project_scope: owned_scope(),
        snapshot: snapshot(),
        current_chapter_id: Some(ChapterId::new("chapter-a")),
        chapters,
    }
}

fn ready(facts: ManuscriptSearchFacts) -> ManuscriptSearchRead {
    ManuscriptSearchRead::Ready(Box::new(facts))
}

fn manuscript_request(query_text: &str) -> ManuscriptSearchRequest {
    ManuscriptSearchRequest {
        query_text: query_text.to_owned(),
        selection: ManuscriptSearchSelection::Manuscript,
        required_watermark: None,
    }
}

#[tokio::test]
async fn a_manuscript_search_returns_ordered_matches_with_chapter_block_and_range_identity() {
    let scope = owned_scope();
    let facts = facts(vec![
        chapter(
            "chapter-b",
            /*volume_order*/ 1,
            /*chapter_order*/ 2,
            "block-b",
            "alpha",
        ),
        chapter(
            "chapter-a",
            /*volume_order*/ 1,
            /*chapter_order*/ 1,
            "block-a",
            "alpha beta alpha",
        ),
    ]);
    let snapshot = facts.snapshot.clone();
    let reader = FixtureReader { read: ready(facts) };

    let result = search_manuscript(&reader, &scope, &manuscript_request("alpha"))
        .await
        .unwrap();

    assert_eq!(
        result,
        SearchManuscript::Ready(ManuscriptSearchPage {
            project_scope: scope,
            source_snapshot: snapshot,
            projection_kind: MANUSCRIPT_SEARCH_PROJECTION_KIND.to_owned(),
            projection_generation: 4,
            projection_watermark: 4,
            required_watermark: None,
            completeness: ManuscriptSearchCompleteness::Complete,
            lag: 0,
            items: vec![
                ManuscriptSearchMatch {
                    chapter_id: ChapterId::new("chapter-a"),
                    manuscript_block_id: "block-a".to_owned(),
                    start: 0,
                    end: 5,
                },
                ManuscriptSearchMatch {
                    chapter_id: ChapterId::new("chapter-a"),
                    manuscript_block_id: "block-a".to_owned(),
                    start: 11,
                    end: 16,
                },
                ManuscriptSearchMatch {
                    chapter_id: ChapterId::new("chapter-b"),
                    manuscript_block_id: "block-b".to_owned(),
                    start: 0,
                    end: 5,
                },
            ],
            page_count: 3,
        })
    );
}

#[tokio::test]
async fn a_current_chapter_search_excludes_other_live_chapters() {
    let scope = owned_scope();
    let facts = facts(vec![
        chapter(
            "chapter-a",
            /*volume_order*/ 1,
            /*chapter_order*/ 1,
            "block-a",
            "alpha",
        ),
        chapter(
            "chapter-b",
            /*volume_order*/ 1,
            /*chapter_order*/ 2,
            "block-b",
            "alpha",
        ),
    ]);
    let snapshot = facts.snapshot.clone();
    let reader = FixtureReader { read: ready(facts) };

    let result = search_manuscript(
        &reader,
        &scope,
        &ManuscriptSearchRequest {
            query_text: "alpha".to_owned(),
            selection: ManuscriptSearchSelection::CurrentChapter,
            required_watermark: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(
        result,
        SearchManuscript::Ready(ManuscriptSearchPage {
            project_scope: scope,
            source_snapshot: snapshot,
            projection_kind: MANUSCRIPT_SEARCH_PROJECTION_KIND.to_owned(),
            projection_generation: 4,
            projection_watermark: 4,
            required_watermark: None,
            completeness: ManuscriptSearchCompleteness::Complete,
            lag: 0,
            items: vec![ManuscriptSearchMatch {
                chapter_id: ChapterId::new("chapter-a"),
                manuscript_block_id: "block-a".to_owned(),
                start: 0,
                end: 5,
            }],
            page_count: 1,
        })
    );
}

#[tokio::test]
async fn zero_matches_are_a_complete_ready_page_not_projection_not_ready() {
    let scope = owned_scope();
    let facts = facts(vec![chapter(
        "chapter-a",
        /*volume_order*/ 1,
        /*chapter_order*/ 1,
        "block-a",
        "alpha",
    )]);
    let snapshot = facts.snapshot.clone();
    let reader = FixtureReader { read: ready(facts) };

    let result = search_manuscript(&reader, &scope, &manuscript_request("zzz"))
        .await
        .unwrap();

    assert_eq!(
        result,
        SearchManuscript::Ready(ManuscriptSearchPage {
            project_scope: scope,
            source_snapshot: snapshot,
            projection_kind: MANUSCRIPT_SEARCH_PROJECTION_KIND.to_owned(),
            projection_generation: 4,
            projection_watermark: 4,
            required_watermark: None,
            completeness: ManuscriptSearchCompleteness::Complete,
            lag: 0,
            items: Vec::new(),
            page_count: 0,
        })
    );
}

#[tokio::test]
async fn an_unmet_required_watermark_is_projection_not_ready() {
    let scope = owned_scope();
    let facts = facts(vec![chapter(
        "chapter-a",
        /*volume_order*/ 1,
        /*chapter_order*/ 1,
        "block-a",
        "alpha",
    )]);
    let snapshot = facts.snapshot.clone();
    let reader = FixtureReader { read: ready(facts) };

    let result = search_manuscript(
        &reader,
        &scope,
        &ManuscriptSearchRequest {
            query_text: "zzz".to_owned(),
            selection: ManuscriptSearchSelection::Manuscript,
            required_watermark: Some(9),
        },
    )
    .await
    .unwrap();

    assert_eq!(
        result,
        SearchManuscript::ProjectionNotReady {
            source_snapshot: snapshot,
            projection_watermark: 4,
            required_watermark: 9,
        }
    );
}

#[tokio::test]
async fn a_rebuild_from_the_same_canonical_facts_returns_the_same_page() {
    let scope = owned_scope();
    let reader = FixtureReader {
        read: ready(facts(vec![chapter(
            "chapter-a",
            /*volume_order*/ 1,
            /*chapter_order*/ 1,
            "block-a",
            "alpha beta alpha",
        )])),
    };
    let request = manuscript_request("alpha");

    let first = search_manuscript(&reader, &scope, &request).await.unwrap();
    let second = search_manuscript(&reader, &scope, &request).await.unwrap();

    assert_eq!(first, second);
}

#[tokio::test]
async fn foreign_scope_facts_fail_closed() {
    let scope = owned_scope();
    let mut facts = facts(vec![chapter(
        "chapter-a",
        /*volume_order*/ 1,
        /*chapter_order*/ 1,
        "block-a",
        "alpha",
    )]);
    facts.project_scope = ProjectScope::new(UserId::new("user-b"), ProjectId::new("project-a"));
    let reader = FixtureReader { read: ready(facts) };

    let result = search_manuscript(&reader, &scope, &manuscript_request("alpha"))
        .await
        .unwrap();

    assert_eq!(result, SearchManuscript::Missing);
}

#[tokio::test]
async fn a_bounded_page_stops_at_the_absolute_item_limit() {
    let scope = owned_scope();
    let reader = FixtureReader {
        read: ready(facts(vec![chapter(
            "chapter-a",
            /*volume_order*/ 1,
            /*chapter_order*/ 1,
            "block-a",
            &"x".repeat(MANUSCRIPT_SEARCH_PAGE_ITEM_LIMIT + 1),
        )])),
    };

    let result = search_manuscript(&reader, &scope, &manuscript_request("x"))
        .await
        .unwrap();

    match result {
        SearchManuscript::Ready(page) => {
            assert_eq!(page.items.len(), MANUSCRIPT_SEARCH_PAGE_ITEM_LIMIT);
            assert_eq!(page.completeness, ManuscriptSearchCompleteness::Truncated);
            assert_eq!(page.page_count, MANUSCRIPT_SEARCH_PAGE_ITEM_LIMIT as u64);
            assert_eq!(page.items[0].start, 0);
            assert_eq!(
                page.items[MANUSCRIPT_SEARCH_PAGE_ITEM_LIMIT - 1].start,
                (MANUSCRIPT_SEARCH_PAGE_ITEM_LIMIT - 1) as u64
            );
        }
        other => panic!("expected ready search, got {other:?}"),
    }
}

#[tokio::test]
async fn astral_unicode_before_and_inside_matches_keeps_exact_utf16_ranges() {
    let scope = owned_scope();
    let facts = facts(vec![chapter(
        "chapter-a",
        /*volume_order*/ 1,
        /*chapter_order*/ 1,
        "block-a",
        "😀ax😀b😀cx😀d",
    )]);
    let snapshot = facts.snapshot.clone();
    let reader = FixtureReader { read: ready(facts) };

    let result = search_manuscript(&reader, &scope, &manuscript_request("x😀"))
        .await
        .unwrap();

    assert_eq!(
        result,
        SearchManuscript::Ready(ManuscriptSearchPage {
            project_scope: scope,
            source_snapshot: snapshot,
            projection_kind: MANUSCRIPT_SEARCH_PROJECTION_KIND.to_owned(),
            projection_generation: 4,
            projection_watermark: 4,
            required_watermark: None,
            completeness: ManuscriptSearchCompleteness::Complete,
            lag: 0,
            items: vec![
                ManuscriptSearchMatch {
                    chapter_id: ChapterId::new("chapter-a"),
                    manuscript_block_id: "block-a".to_owned(),
                    start: 3,
                    end: 6,
                },
                ManuscriptSearchMatch {
                    chapter_id: ChapterId::new("chapter-a"),
                    manuscript_block_id: "block-a".to_owned(),
                    start: 10,
                    end: 13,
                },
            ],
            page_count: 2,
        })
    );
}

#[tokio::test]
async fn dense_ascii_page_decodes_each_utf16_unit_once() {
    const BLOCK_LEN: usize = 1_000_001;
    const MATCH_STRIDE: usize = 2_000;
    const TARGET_UTF16_UNITS: u64 = 997_502;
    let mut text = vec![b'a'; BLOCK_LEN];
    for offset in (0..=1_000_000).step_by(MATCH_STRIDE) {
        text[offset] = b'x';
    }
    let text = String::from_utf8(text).expect("ascii block");
    let scope = owned_scope();
    let reader = FixtureReader {
        read: ready(facts(vec![chapter(
            "chapter-a",
            /*volume_order*/ 1,
            /*chapter_order*/ 1,
            "block-a",
            &text,
        )])),
    };

    super::utf16_decode_count::reset();
    let result = search_manuscript(&reader, &scope, &manuscript_request("x"))
        .await
        .unwrap();

    match result {
        SearchManuscript::Ready(page) => {
            assert_eq!(page.items.len(), MANUSCRIPT_SEARCH_PAGE_ITEM_LIMIT);
            assert_eq!(page.completeness, ManuscriptSearchCompleteness::Truncated);
            assert_eq!(page.items[0].start, 0);
            assert_eq!(page.items[0].end, 1);
            assert_eq!(
                page.items[MANUSCRIPT_SEARCH_PAGE_ITEM_LIMIT - 1].start,
                998_000
            );
            assert_eq!(
                page.items[MANUSCRIPT_SEARCH_PAGE_ITEM_LIMIT - 1].end,
                998_001
            );
        }
        other => panic!("expected ready search, got {other:?}"),
    }
    assert_eq!(super::utf16_decode_count::take(), TARGET_UTF16_UNITS);
}
