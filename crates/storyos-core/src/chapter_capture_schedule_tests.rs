use std::time::{Duration, SystemTime};

use super::*;
use crate::assemble_context::CONTEXT_ITEM_TOKEN_LIMIT;

fn at(seconds: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
}

/// The due time is the commit time and 1 hour, or the start of the last capture of the Chapter and
/// 6 hours when that is later.
#[test]
fn the_due_time_is_the_quiet_time_or_the_interval_of_the_last_capture() {
    let committed = at(100_000);
    assert_eq!(
        [
            chapter_capture_due(committed, None),
            chapter_capture_due(committed, Some(at(100_000 - 21_600))),
            chapter_capture_due(committed, Some(at(100_000 - 1_000))),
        ],
        [at(103_600), at(103_600), at(100_000 - 1_000 + 21_600)]
    );
}

/// The first Chapter in Chapter order goes first. The rate permits a start 2 minutes after the
/// last Chapter capture of the Project.
#[test]
fn the_first_chapter_in_order_goes_first_at_the_rate_of_the_project() {
    let now = at(10_000);
    let turn = |positions: &[usize], last_start| chapter_capture_turn(positions, last_start, now);
    assert_eq!(
        [
            turn(&[3, 1, 2], None),
            turn(&[3, 1, 2], Some(at(10_000 - 120))),
            turn(&[2], Some(at(10_000 - 30))),
            turn(&[], None),
        ],
        [
            Some(ChapterCaptureTurn {
                first: 1,
                rate_permits_at: None
            }),
            Some(ChapterCaptureTurn {
                first: 1,
                rate_permits_at: None
            }),
            Some(ChapterCaptureTurn {
                first: 0,
                rate_permits_at: Some(at(10_090))
            }),
            None,
        ]
    );
}

/// The decision table of S7-D5, in its order: the first fact that refuses gives the skip, and
/// each skip gives its change of the row.
#[test]
fn the_chapter_decision_follows_the_table_in_its_order() {
    let start = ChapterCaptureFacts {
        live: true,
        text_empty: false,
        revision_read: false,
        capture_in_progress: false,
        rate_permits_at: None,
        author_run_completed: true,
    };
    let permitted = at(500);
    let changes: [fn(&mut ChapterCaptureFacts); 6] = [
        |facts| facts.live = false,
        |facts| facts.text_empty = true,
        |facts| facts.revision_read = true,
        |facts| facts.capture_in_progress = true,
        |facts| facts.rate_permits_at = Some(at(500)),
        |facts| facts.author_run_completed = false,
    ];
    // Each fact applies with each later fact, so a later row cannot hide an earlier one.
    let decisions = (0..changes.len()).map(|row| {
        let mut facts = start;
        changes[row..].iter().for_each(|change| change(&mut facts));
        decide_chapter_capture(&facts).map_err(|skip| (skip.label(), skip.row_change()))
    });
    use ChapterRowChange::*;
    assert_eq!(
        decisions.collect::<Vec<_>>(),
        [
            Err(("chapter_not_live", DueEmpty)),
            Err(("text_empty", DueEmpty)),
            Err(("revision_read", DueEmpty)),
            Err(("capture_in_progress", DueEmpty)),
            Err(("rate_wait", DueAt(permitted))),
            Err(("no_completed_run", Held)),
        ]
    );
    assert_eq!(
        (
            decide_chapter_capture(&start),
            ChapterCaptureSkip::StartRefused.label(),
            ChapterCaptureSkip::StartRefused.row_change(),
        ),
        (Ok(()), "start_refused", Held)
    );
}

/// The input has the number and the title of the Chapter, then each Block in order.
#[test]
fn the_input_is_the_number_the_title_and_the_blocks() {
    let blocks = ["雨夜。".to_owned(), "林青回到旧宅。".to_owned()];
    assert_eq!(
        chapter_capture_input("第一章", 2, &blocks, CONTEXT_ITEM_TOKEN_LIMIT),
        ChapterCaptureInput {
            text: "{\"chapter_number\":2,\"chapter_title\":\"第一章\"}\n\n雨夜。\n\n林青回到旧宅。"
                .to_owned(),
            projection_mode: ProjectionMode::ExactRequired,
        }
    );
}

/// A text at the limit is exact. Above the limit, the excerpt keeps the whole Blocks that fit and
/// the longest prefix of the next Block that fits, with its mark.
#[test]
fn a_text_above_the_limit_gives_an_excerpt_with_its_mark() {
    let blocks = ["一".repeat(20), "二".repeat(300), "三".repeat(20)];
    let exact = chapter_capture_input("章", 1, &blocks, CONTEXT_ITEM_TOKEN_LIMIT);
    let limit = count_context_item_tokens(&exact.text);
    let excerpt = |kept: usize| ChapterCaptureInput {
        text: format!(
            "{{\"chapter_number\":1,\"chapter_title\":\"章\"}}\n\n{first}\n\n{prefix} [The Host \
             cut the Chapter here. Last whole Block: 1 of 3. It keeps {kept} of the 300 \
             characters of Block 2. read_chapter gives the other Blocks.]",
            first = blocks[0],
            prefix = "二".repeat(kept),
        ),
        projection_mode: ProjectionMode::DeterministicExcerpt,
    };
    let cut_limit = count_context_item_tokens(&excerpt(100).text);
    assert_eq!(
        [
            chapter_capture_input("章", 1, &blocks, limit),
            chapter_capture_input("章", 1, &blocks, cut_limit),
            chapter_capture_input("章", 1, &blocks, cut_limit + 1),
        ],
        [exact.clone(), excerpt(100), excerpt(101)]
    );
    assert_eq!(
        chapter_capture_input("章", 1, &blocks, limit - 1).projection_mode,
        ProjectionMode::DeterministicExcerpt
    );
}
