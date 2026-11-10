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

/// The span of the first `part` of the result text `result`.
fn at(result: &str, part: &str) -> ResultSpan {
    ResultSpan {
        offset: result.find(part).expect("the part is in the result text"),
        bytes: part.len(),
    }
}

/// A Snapshot whose text is at `offset` of its result text.
fn source(
    offset: usize,
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
        span: ResultSpan {
            offset,
            bytes: text.len(),
        },
        digest: format!("sha256:{}", crate::hex_sha256(text.as_bytes())),
    }
}

fn gap(locator: &str, reason: SourceGapReason, span: Option<ResultSpan>) -> SourceGap {
    SourceGap {
        locator: locator.to_owned(),
        reason,
        span,
    }
}

/// Each Snapshot text is exactly the bytes of its span in the result text.
fn exact(result: &str, capture: &ResearchCapture) -> bool {
    capture.snapshots.iter().all(|source| {
        result.get(source.span.offset..source.span.offset + source.span.bytes)
            == Some(source.text.as_str())
    })
}

#[test]
fn each_search_entry_with_a_url_is_one_search_result_snapshot_and_a_snippet_line_starts_none() {
    let text = "Title: Liverpool tide table 1890\nURL: https://e.org/tides\n\
                Published: 1890-01-01T00:00:00.000Z\nAuthor: N/A\nHighlights:\n\
                High water at 10.42.\n\n...\n\nLow water at 4.55.\n\n---\n\n\
                Title: N/A\nURL: https://e.org/port\nPublished: N/A\nAuthor: Harbour Board\n\
                Text: The port log.\nTitle: Quoted\nURL: https://other.example/";
    let settlement = settle_tool_call(&WEB_SEARCH_TOOL_SPEC, &called(text, false));
    let intake = ToolIntake::WebSearch {
        query: "tide tables of 1890".to_owned(),
        result_count: None,
    };

    let tides = "High water at 10.42.\n\n...\n\nLow water at 4.55.";
    let port = "The port log.\nTitle: Quoted\nURL: https://other.example/";
    let capture = capture_research_sources(ResultFormat::TitledEntries, &intake, &settlement);

    assert!(exact(text, &capture));
    assert_eq!(
        capture,
        ResearchCapture {
            snapshots: vec![
                source(
                    at(text, tides).offset,
                    SourceKind::SearchResult,
                    "https://e.org/tides",
                    Some("Liverpool tide table 1890"),
                    Some("1890-01-01T00:00:00.000Z"),
                    tides,
                ),
                // 6 bytes after the line start: the `Text: ` wrapper is not snippet text.
                source(
                    at(text, "Text: The port").offset + 6,
                    SourceKind::SearchResult,
                    "https://e.org/port",
                    None,
                    None,
                    port
                ),
            ],
            gaps: Vec::new(),
        }
    );
}

#[test]
fn a_failed_an_empty_and_a_missing_url_are_gaps_and_never_snapshots() {
    // Page a quotes a header of another page, and the reason of c contains ": ".
    let quote = "High water at 10.42.\n\n# Example\nURL: https://other.example/\nQuoted text.";
    let text = format!(
        "# Tide table\nURL: https://e.org/a\nPublished: 1890-01-01\n\n{quote}\n\n\
         # (no title)\nURL: https://e.org/b/\n\nThe port log.\n\n\
         # Empty page\nURL: https://e.org/e\n\n\n\
         Error fetching https://e.org/c: HTTP error: 404"
    );
    let urls = [
        "https://e.org/a",
        "https://e.org/b",
        "https://e.org/c",
        "https://e.org/d",
        "https://e.org/e",
    ];
    let settlement = settle_tool_call(&WEB_FETCH_TOOL_SPEC, &called(&text, false));
    let capture = capture_research_sources(ResultFormat::PageBlocks, &fetch(&urls), &settlement);

    assert!(exact(&text, &capture));
    assert_eq!(
        capture,
        ResearchCapture {
            snapshots: vec![
                source(
                    at(&text, quote).offset,
                    SourceKind::Page,
                    "https://e.org/a",
                    Some("Tide table"),
                    Some("1890-01-01"),
                    quote
                ),
                source(
                    at(&text, "The port log.").offset,
                    SourceKind::Page,
                    "https://e.org/b/",
                    None,
                    None,
                    "The port log."
                ),
            ],
            gaps: vec![
                gap(
                    "https://e.org/c",
                    SourceGapReason::Failed,
                    Some(at(&text, "Error fetching https://e.org/c: HTTP error: 404"))
                ),
                gap("https://e.org/d", SourceGapReason::Missing, None),
                gap(
                    "https://e.org/e",
                    SourceGapReason::Empty,
                    Some(at(&text, "# Empty page\nURL: https://e.org/e"))
                ),
            ],
        }
    );
}

#[test]
fn a_header_that_answers_no_requested_url_in_order_makes_the_page_read_unattributed() {
    let page = |url: &str, text: &str| format!("# Page\nURL: {url}\n\n{text}");
    let read = |text: &str, urls: &[&str]| {
        let settlement = settle_tool_call(&WEB_FETCH_TOOL_SPEC, &called(text, false));
        capture_research_sources(ResultFormat::PageBlocks, &fetch(urls), &settlement)
    };
    let unattributed = |urls: &[&str]| ResearchCapture {
        snapshots: Vec::new(),
        gaps: urls
            .iter()
            .map(|url| gap(url, SourceGapReason::Unattributed, None))
            .collect(),
    };
    let a = "https://e.org/a";
    let b = "https://e.org/b";
    // Page a quotes its own header, with an empty line after the quoted header.
    let quotes_itself = page(a, &format!("Before.\n\n{}", page(a, "After.")));
    // Page a quotes the header of b, and the page of b follows.
    let quotes_the_next = format!("{}\n\n{}", page(a, &page(b, "Quoted.")), page(b, "Real."));
    // The server read b at a changed URL.
    let changed = format!(
        "{}\n\n{}",
        page(a, "High water."),
        page("https://e.org/new", "Moved.")
    );

    assert_eq!(
        [
            read(&quotes_itself, &[a]),
            read(&quotes_the_next, &[a, b]),
            read(&changed, &[a, b]),
        ],
        [
            unattributed(&[a]),
            unattributed(&[a, b]),
            unattributed(&[a, b])
        ]
    );
}

#[test]
fn a_quoted_error_line_at_the_end_of_a_page_stays_page_text() {
    let text = "# Page\nURL: https://e.org/a\n\nThe log said:\nError fetching https://e.org/a: HTTP error: 404";
    let settlement = settle_tool_call(&WEB_FETCH_TOOL_SPEC, &called(text, false));

    assert_eq!(
        capture_research_sources(
            ResultFormat::PageBlocks,
            &fetch(&["https://e.org/a"]),
            &settlement
        ),
        ResearchCapture {
            snapshots: vec![source(
                at(text, "The log said:").offset,
                SourceKind::Page,
                "https://e.org/a",
                Some("Page"),
                None,
                "The log said:\nError fetching https://e.org/a: HTTP error: 404"
            )],
            gaps: Vec::new(),
        }
    );
}

#[test]
fn the_last_search_snippet_keeps_a_literal_separator() {
    let text = "Title: Port\nURL: https://e.org/port\nText: Example---";
    let settlement = settle_tool_call(&WEB_SEARCH_TOOL_SPEC, &called(text, false));
    let intake = ToolIntake::WebSearch {
        query: "port".to_owned(),
        result_count: None,
    };

    assert_eq!(
        capture_research_sources(ResultFormat::TitledEntries, &intake, &settlement).snapshots,
        vec![source(
            at(text, "Example---").offset,
            SourceKind::SearchResult,
            "https://e.org/port",
            Some("Port"),
            None,
            "Example---"
        )]
    );
}

#[test]
fn metadata_above_the_store_bounds_is_absent_and_an_entry_with_a_long_url_is_dropped() {
    let title = "T".repeat(SOURCE_TITLE_MAX_CHARS + 1);
    let published = "1".repeat(SOURCE_PUBLISHED_MAX_CHARS + 1);
    let long_url = format!("https://e.org/{}", "u".repeat(SOURCE_LOCATOR_MAX_CHARS));
    let text = format!(
        "Title: {title}\nURL: https://e.org/a\nPublished: {published}\nText: High water.\n\n---\n\n\
         Title: Long\nURL: {long_url}\nText: Dropped."
    );
    let settlement = settle_tool_call(&WEB_SEARCH_TOOL_SPEC, &called(&text, false));
    let intake = ToolIntake::WebSearch {
        query: "tides".to_owned(),
        result_count: None,
    };

    assert_eq!(
        capture_research_sources(ResultFormat::TitledEntries, &intake, &settlement).snapshots,
        vec![source(
            at(&text, "High water.").offset,
            SourceKind::SearchResult,
            "https://e.org/a",
            None,
            None,
            "High water."
        )]
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
                    gap("https://e.org/a", SourceGapReason::Failed, None),
                    gap("https://e.org/c", SourceGapReason::Failed, None),
                ],
            },
            ResearchCapture::default(),
        ]
    );
}

#[test]
fn a_source_keeps_its_exact_bytes_with_leading_and_trailing_whitespace() {
    // An indented first line, a Markdown code block, a line with trailing spaces, and an extra
    // empty line at the end of the page all belong to the source.
    let page = "    indented first line\n\n```\ncode  \n```\nlast line with spaces   \n";
    let text = format!(
        "# Page\nURL: https://e.org/a\n\n{page}\n\n# Next\nURL: https://e.org/b\n\n \tB text \n\n"
    );
    let settlement = settle_tool_call(&WEB_FETCH_TOOL_SPEC, &called(&text, false));
    let capture = capture_research_sources(
        ResultFormat::PageBlocks,
        &fetch(&["https://e.org/a", "https://e.org/b"]),
        &settlement,
    );

    assert!(exact(&text, &capture));
    assert_eq!(
        capture
            .snapshots
            .iter()
            .map(|source| source.text.as_str())
            .collect::<Vec<_>>(),
        [page, " \tB text "]
    );
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
