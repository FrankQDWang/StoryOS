use super::*;
use crate::{ToolObservation, WEB_FETCH_TOOL_SPEC, WEB_SEARCH_TOOL_SPEC, settle_tool_call};

fn called(text: &str, is_error: bool) -> ToolObservation {
    ToolObservation::Called {
        text: vec![text.to_owned()],
        is_error,
    }
}

fn fetch(urls: &[&str]) -> ToolIntake {
    ToolIntake::WebFetch {
        urls: urls.iter().map(|url| (*url).to_owned()).collect(),
    }
}

fn source(
    kind: SourceKind,
    locator: &str,
    title: Option<&str>,
    published: Option<&str>,
    text: &str,
) -> CapturedSource {
    CapturedSource {
        kind,
        locator: locator.to_owned(),
        title: title.map(str::to_owned),
        published: published.map(str::to_owned),
        text: text.to_owned(),
        original_bytes: text.len(),
        truncated: false,
        digest: format!("sha256:{}", crate::hex_sha256(text.as_bytes())),
    }
}

fn gap(locator: &str, reason: SourceGapReason) -> SourceGap {
    SourceGap {
        locator: locator.to_owned(),
        reason,
    }
}

#[test]
fn each_search_entry_with_a_url_is_one_search_result_snapshot() {
    let text = "Title: Liverpool tide table 1890\nURL: https://e.org/tides\n\
                Published: 1890-01-01T00:00:00.000Z\nAuthor: N/A\nHighlights:\n\
                High water at 10.42.\n\n...\n\nLow water at 4.55.\n\n---\n\n\
                Title: N/A\nURL: https://e.org/port\nPublished: N/A\nAuthor: Harbour Board\n\
                Text: The port log.";
    let settlement = settle_tool_call(&WEB_SEARCH_TOOL_SPEC, &called(text, false));
    let intake = ToolIntake::WebSearch {
        query: "tide tables of 1890".to_owned(),
        result_count: None,
    };

    assert_eq!(
        capture_research_sources(ResultFormat::TitledEntries, &intake, &settlement),
        ResearchCapture {
            snapshots: vec![
                source(
                    SourceKind::SearchResult,
                    "https://e.org/tides",
                    Some("Liverpool tide table 1890"),
                    Some("1890-01-01T00:00:00.000Z"),
                    "High water at 10.42.\n\n...\n\nLow water at 4.55.",
                ),
                source(
                    SourceKind::SearchResult,
                    "https://e.org/port",
                    None,
                    None,
                    "The port log."
                ),
            ],
            gaps: Vec::new(),
        }
    );
}

#[test]
fn a_failed_an_empty_and_a_missing_url_are_gaps_and_never_snapshots() {
    let text = "# Tide table\nURL: https://e.org/a\nPublished: 1890-01-01\n\nHigh water at 10.42.\n\n\
                # (no title)\nURL: https://e.org/b/\n\nThe port log.\n\n\
                # Empty page\nURL: https://e.org/e\n\n\n\
                Error fetching https://e.org/c: CRAWL_NOT_FOUND";
    let urls = [
        "https://e.org/a",
        "https://e.org/b",
        "https://e.org/c",
        "https://e.org/d",
        "https://e.org/e",
    ];
    let settlement = settle_tool_call(&WEB_FETCH_TOOL_SPEC, &called(text, false));

    assert_eq!(
        capture_research_sources(ResultFormat::PageBlocks, &fetch(&urls), &settlement),
        ResearchCapture {
            snapshots: vec![
                source(
                    SourceKind::Page,
                    "https://e.org/a",
                    Some("Tide table"),
                    Some("1890-01-01"),
                    "High water at 10.42."
                ),
                source(
                    SourceKind::Page,
                    "https://e.org/b/",
                    None,
                    None,
                    "The port log."
                ),
            ],
            gaps: vec![
                gap("https://e.org/c", SourceGapReason::Failed),
                gap("https://e.org/d", SourceGapReason::Missing),
                gap("https://e.org/e", SourceGapReason::Empty),
            ],
        }
    );
}

#[test]
fn a_page_at_a_changed_url_answers_the_requested_url_with_no_gap() {
    let text = "# Tide table\nURL: https://e.org/tides\n\nHigh water at 10.42.";
    let settlement = settle_tool_call(&WEB_FETCH_TOOL_SPEC, &called(text, false));

    assert_eq!(
        capture_research_sources(
            ResultFormat::PageBlocks,
            &fetch(&["http://e.org/old"]),
            &settlement
        ),
        ResearchCapture {
            snapshots: vec![source(
                SourceKind::Page,
                "https://e.org/tides",
                Some("Tide table"),
                None,
                "High water at 10.42."
            )],
            gaps: Vec::new(),
        }
    );
}

#[test]
fn a_server_error_fails_each_requested_url_and_an_unknown_outcome_captures_nothing() {
    let urls = ["https://e.org/a", "https://e.org/c"];
    let error = settle_tool_call(
        &WEB_FETCH_TOOL_SPEC,
        &called(
            "Error fetching URL(s): https://e.org/a: CRAWL_NOT_FOUND",
            true,
        ),
    );
    let unknown = settle_tool_call(&WEB_FETCH_TOOL_SPEC, &ToolObservation::OutcomeUnknown);

    assert_eq!(
        [
            capture_research_sources(ResultFormat::PageBlocks, &fetch(&urls), &error),
            capture_research_sources(ResultFormat::PageBlocks, &fetch(&urls), &unknown),
        ],
        [
            ResearchCapture {
                snapshots: Vec::new(),
                gaps: vec![
                    gap("https://e.org/a", SourceGapReason::Failed),
                    gap("https://e.org/c", SourceGapReason::Failed),
                ],
            },
            ResearchCapture::default(),
        ]
    );
}

#[test]
fn a_text_above_the_bound_keeps_a_prefix_with_an_explicit_mark_and_its_digest_covers_the_kept_bytes()
 {
    // A two-byte character, so that the cut must find a character boundary.
    let page = "é".repeat(SOURCE_TEXT_MAX_BYTES / 2 + 10);
    let settlement = ToolCallSettlement {
        result: Some(crate::ToolResultText {
            text: format!("# Long\nURL: https://e.org/long\n\n{page}"),
            digest: String::new(),
            bytes: 0,
        }),
        ..settle_tool_call(&WEB_FETCH_TOOL_SPEC, &called("x", false))
    };

    let captured = capture_research_sources(
        ResultFormat::PageBlocks,
        &fetch(&["https://e.org/long"]),
        &settlement,
    )
    .snapshots
    .remove(0);

    let mark = format!(
        "\n[StoryOS cut this source here. It keeps 262074 of its {} bytes.]",
        page.len()
    );
    let kept = format!("{}{mark}", "é".repeat(262_074 / 2));
    assert_eq!(
        captured,
        CapturedSource {
            original_bytes: page.len(),
            truncated: true,
            ..source(
                SourceKind::Page,
                "https://e.org/long",
                Some("Long"),
                None,
                &kept
            )
        }
    );
    assert!(captured.text.len() <= SOURCE_TEXT_MAX_BYTES);
}

#[test]
fn the_source_version_of_a_tool_result_names_its_snapshots_or_else_its_tool_call() {
    let ids = ["s1".to_owned(), "s2".to_owned()];

    assert_eq!(
        [
            tool_result_source_version("call", &ids),
            tool_result_source_version("call", &[])
        ],
        ["s1,s2".to_owned(), "call".to_owned()]
    );
}
