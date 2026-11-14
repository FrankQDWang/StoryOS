//! The Source Snapshots and Source Gaps of one settled research ToolCall (research provenance
//! contract section 7). The Tool Registration names the text format of each tool result, so the
//! Snapshot does not name the service (ADR 0084). The result text that the Tool Gateway validated
//! is split once into entries: each entry with a URL is one Snapshot, and each requested URL that
//! the server reports as failed, empty, or missing is one Gap. A Gap is never a citation.

use crate::{ToolAttemptSettlement, ToolCallSettlement, ToolCallState, ToolIntake};

/// The character bounds of the stored metadata of one Source Snapshot. They are the CHECK bounds
/// of the store, so that a capture never fails the settlement transaction.
pub const SOURCE_LOCATOR_MAX_CHARS: usize = 8192;
pub const SOURCE_TITLE_MAX_CHARS: usize = 8192;
pub const SOURCE_PUBLISHED_MAX_CHARS: usize = 128;

/// The text format of the result of one registered tool.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResultFormat {
    /// Entries of `Title:`, `URL:`, `Published:`, and `Author:` lines, then `Highlights:` or
    /// `Text:` and the snippet, with a `---` line between two entries.
    TitledEntries,
    /// Pages of a `# <title>` line, a `URL:` line, optional `Published:` and `Author:` lines, and
    /// the text; then one `Error fetching <url>: <reason>` line for each failed URL.
    PageBlocks,
}

/// The medium of one Source Snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceKind {
    SearchResult,
    Page,
}

impl SourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SearchResult => "search_result",
            Self::Page => "page",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [Self::SearchResult, Self::Page]
            .into_iter()
            .find(|kind| kind.as_str() == value)
    }
}

/// Why a requested URL has no Snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceGapReason {
    /// The server reported that it could not read the URL.
    Failed,
    /// The server answered for the URL with no text.
    Empty,
    /// The result has no entry for the URL.
    Missing,
    /// The server answered, but StoryOS cannot split the result text into its pages with
    /// certainty, for example because a page quotes a header of the format.
    Unattributed,
}

impl SourceGapReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Failed => "failed",
            Self::Empty => "empty",
            Self::Missing => "missing",
            Self::Unattributed => "unattributed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [Self::Failed, Self::Empty, Self::Missing, Self::Unattributed]
            .into_iter()
            .find(|reason| reason.as_str() == value)
    }
}

/// A byte range of the exact result text of a ToolCall (`tool_calls.result_text`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResultSpan {
    pub offset: usize,
    pub bytes: usize,
}

/// One captured entry, before the store gives it its identity and capture time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapturedSource {
    pub kind: SourceKind,
    /// The URL that the server reported: the Evidence Locator of the entry.
    pub locator: String,
    pub title: Option<String>,
    /// The published date as the server reported it.
    pub published: Option<String>,
    /// The snippet of a search entry or the text of a page: the exact bytes of the result text
    /// between the wrappers of the format, with no change. It is at most
    /// `TOOL_RESULT_TEXT_MAX_BYTES`, because the Tool Gateway fails a longer result before any
    /// capture: StoryOS never cuts a source.
    pub text: String,
    /// Where `text` is in the exact result text. `span.bytes` is the length of `text`.
    pub span: ResultSpan,
    /// The SHA-256 of the stored text bytes.
    pub digest: String,
}

/// One requested URL with no Snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceGap {
    pub locator: String,
    pub reason: SourceGapReason,
    /// The lines of the result text that give the Gap: the error line of a `failed` URL, or the
    /// header of an `empty` page. A Gap that no line gives has none.
    pub span: Option<ResultSpan>,
}

/// The Snapshots, in result order, and the Gaps, in request order, of one ToolCall.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResearchCapture {
    pub snapshots: Vec<CapturedSource>,
    pub gaps: Vec<SourceGap>,
}

/// The capture of one settlement. A settled result gives its entries. A page read that the
/// server answered with an error gives one `failed` Gap for each URL. An unknown or unsent call
/// gives nothing: the server reported nothing about its URLs.
pub fn capture_research_sources(
    format: ResultFormat,
    intake: &ToolIntake,
    settlement: &ToolCallSettlement,
) -> ResearchCapture {
    let requested: &[String] = match intake {
        ToolIntake::WebFetch { urls } => urls,
        ToolIntake::WebSearch { .. }
        | ToolIntake::Finish
        | ToolIntake::ProposeEdit(_)
        | ToolIntake::SubmitResearch { .. } => &[],
    };
    match (settlement.state, &settlement.result) {
        (ToolCallState::Settled, Some(result)) => match format {
            ResultFormat::TitledEntries => ResearchCapture {
                snapshots: titled_entries(&result.text),
                gaps: Vec::new(),
            },
            ResultFormat::PageBlocks => page_blocks(&result.text, requested),
        },
        _ if settlement.attempt == ToolAttemptSettlement::NativeError => ResearchCapture {
            snapshots: Vec::new(),
            gaps: requested
                .iter()
                .map(|url| SourceGap {
                    locator: url.clone(),
                    reason: SourceGapReason::Failed,
                    span: None,
                })
                .collect(),
        },
        _ => ResearchCapture::default(),
    }
}

/// The Context Source Version of a `tool_result` candidate: its Source Snapshot ids in result
/// order, or the ToolCall identity when the ToolCall captured no Snapshot.
pub fn tool_result_source_version(tool_call_id: &str, source_snapshot_ids: &[String]) -> String {
    if source_snapshot_ids.is_empty() {
        tool_call_id.to_owned()
    } else {
        source_snapshot_ids.join(",")
    }
}

/// The lines of a result text, with the byte offset of each line.
struct Lines<'a> {
    text: &'a str,
    lines: Vec<&'a str>,
    starts: Vec<usize>,
}

impl<'a> Lines<'a> {
    fn new(text: &'a str) -> Self {
        let lines: Vec<&str> = text.split('\n').collect();
        let starts = lines
            .iter()
            .scan(0, |offset, line| {
                let start = *offset;
                *offset += line.len() + 1;
                Some(start)
            })
            .collect();
        Self {
            text,
            lines,
            starts,
        }
    }

    fn empty(&self, index: usize) -> bool {
        self.lines.get(index).is_some_and(|line| line.is_empty())
    }

    /// The exact bytes of `lines[first..end]` with their span. No line gives an empty span at the
    /// start of `first`.
    fn span(&self, first: usize, end: usize) -> (ResultSpan, &'a str) {
        let offset = self.starts.get(first).copied().unwrap_or(self.text.len());
        let stop = if first < end {
            self.starts[end - 1] + self.lines[end - 1].len()
        } else {
            offset
        };
        (
            ResultSpan {
                offset,
                bytes: stop - offset,
            },
            &self.text[offset..stop],
        )
    }
}

/// The header lines of one entry.
struct Entry {
    title: Option<String>,
    locator: String,
    published: Option<String>,
}

/// The header of the entry that starts at line `start`: a line with `title_prefix`, a `URL:`
/// line, and optional `Published:` and `Author:` lines. Returns the entry and the index of the
/// line after the header.
fn header(
    lines: &Lines<'_>,
    start: usize,
    title_prefix: &str,
    no_title: &str,
) -> Option<(Entry, usize)> {
    let title = lines.lines.get(start)?.strip_prefix(title_prefix)?;
    let locator = lines.lines.get(start + 1)?.strip_prefix("URL: ")?.trim();
    let mut next = start + 2;
    let mut published = None;
    while let Some(line) = lines.lines.get(next) {
        if let Some(value) = line.strip_prefix("Published: ") {
            published = reported(value, "N/A", SOURCE_PUBLISHED_MAX_CHARS);
        } else if !line.starts_with("Author: ") {
            break;
        }
        next += 1;
    }
    let entry = Entry {
        title: reported(title, no_title, SOURCE_TITLE_MAX_CHARS),
        locator: locator.to_owned(),
        published,
    };
    Some((entry, next))
}

/// The entries of a search result. The format of the server writes the header lines, then
/// `Highlights:` and the snippet lines or `Text: ` and the snippet, and the separator lines `""`,
/// `"---"`, `""` between two entries. An entry starts at the first line or after a separator, so
/// that a snippet line cannot start an entry. The snippet is the exact bytes between those
/// wrappers: the separator before a confirmed next entry and the empty lines at the end of the
/// text are wrappers. An entry with no URL or with a URL above the locator bound is dropped.
fn titled_entries(text: &str) -> Vec<CapturedSource> {
    let lines = Lines::new(text);
    let count = lines.lines.len();
    let separated = |index: usize| {
        index == 0 || (index >= 3 && lines.lines[index - 3..index] == ["", "---", ""])
    };
    let mut entries: Vec<(usize, Entry, usize)> = Vec::new();
    let mut index = 0;
    while index < count {
        match header(&lines, index, "Title: ", "N/A").filter(|_| separated(index)) {
            Some((entry, body)) => {
                entries.push((index, entry, body));
                index = body;
            }
            None => index += 1,
        }
    }
    let mut last = count;
    while last > 0 && lines.empty(last - 1) {
        last -= 1;
    }
    let ends: Vec<usize> = entries
        .iter()
        .skip(1)
        .map(|(start, _, _)| start - 3)
        .chain([last])
        .collect();
    entries
        .into_iter()
        .zip(ends)
        .filter(|((_, entry, _), _)| {
            !entry.locator.is_empty() && entry.locator.chars().count() <= SOURCE_LOCATOR_MAX_CHARS
        })
        .map(|((_, entry, body), end)| {
            let end = end.max(body);
            let (span, snippet) = match lines.lines.get(body) {
                Some(&"Highlights:") => lines.span(body + 1, end.max(body + 1)),
                Some(line) if line.starts_with("Text: ") && body < end => {
                    let (span, text) = lines.span(body, end);
                    let prefix = "Text: ".len();
                    (
                        ResultSpan {
                            offset: span.offset + prefix,
                            bytes: span.bytes - prefix,
                        },
                        &text[prefix..],
                    )
                }
                _ => lines.span(body, end),
            };
            captured(SourceKind::SearchResult, entry, snippet, span)
        })
        .collect()
}

/// The pages of a page read. The text format has no boundary that page text cannot also hold,
/// so StoryOS attributes a page only when the result is unambiguous:
///
/// - A header candidate is a `# <title>` line and a `URL:` line, with optional `Published:` and
///   `Author:` lines, at the first line or after an empty line, with an empty line or the end of
///   the text after it.
/// - The trailing `Error fetching <url>: <reason>` lines name requested URLs with no candidate.
/// - Each candidate must answer the next requested URL in request order that did not fail.
///
/// When each candidate answers a requested URL in that order and no text comes before the first
/// one, each candidate starts one page, which ends at the next candidate. The page text is the
/// exact bytes between the wrappers of the format: the empty line after the header, the empty
/// line before the next header, and the empty lines before the error lines or the end. Otherwise,
/// for example when a page quotes a header or when the server read a page at a changed URL,
/// StoryOS cannot split the text with certainty: it keeps no Snapshot, and each requested URL
/// that did not fail gets an `unattributed` Gap. StoryOS never guesses a split and never cuts a
/// page.
fn page_blocks(text: &str, requested: &[String]) -> ResearchCapture {
    let lines = Lines::new(text);
    let mut last = lines.lines.len();
    while last > 0 && lines.empty(last - 1) {
        last -= 1;
    }
    let candidates: Vec<(usize, Entry, usize)> = (0..last)
        .filter(|&index| index == 0 || lines.empty(index - 1))
        .filter_map(|index| {
            header(&lines, index, "# ", "(no title)")
                .filter(|(_, body)| *body >= last || lines.empty(*body))
                .map(|(entry, body)| (index, entry, body))
        })
        .collect();
    let has_candidate = |position: usize| {
        candidates
            .iter()
            .any(|(_, entry, _)| same_url(&requested[position], &entry.locator))
    };
    let mut failed: Vec<(usize, ResultSpan)> = Vec::new();
    while last > 0 {
        let line = lines.lines[last - 1];
        let named = line
            .strip_prefix("Error fetching ")
            .and_then(|rest| error_url(rest, requested))
            .filter(|position| !has_candidate(*position));
        match named {
            Some(position) => failed.push((position, lines.span(last - 1, last).0)),
            None if line.is_empty() => {}
            None => break,
        }
        last -= 1;
    }
    let failed_at = |position: usize| {
        failed
            .iter()
            .find(|(failed, _)| *failed == position)
            .map(|(_, span)| *span)
    };
    // Each candidate answers the next requested URL, in request order, that did not fail.
    let mut answers: Vec<usize> = Vec::new();
    let attributed = candidates.iter().all(|(start, entry, _)| {
        let next = answers.last().map_or(0, |answer| answer + 1);
        let answer = (next..requested.len()).find(|position| {
            failed_at(*position).is_none() && same_url(&requested[*position], &entry.locator)
        });
        answer.is_some_and(|answer| {
            answers.push(answer);
            *start < last
        })
    }) && (0..candidates.first().map_or(last, |(start, _, _)| *start))
        .all(|index| lines.empty(index));
    let gap = |position: usize, reason, span| SourceGap {
        locator: requested[position].clone(),
        reason,
        span,
    };
    if !attributed {
        return ResearchCapture {
            snapshots: Vec::new(),
            gaps: (0..requested.len())
                .map(|position| match failed_at(position) {
                    Some(span) => gap(position, SourceGapReason::Failed, Some(span)),
                    None => gap(position, SourceGapReason::Unattributed, None),
                })
                .collect(),
        };
    }
    // A page ends at the empty line before the next header, or at the end of the pages.
    let ends: Vec<usize> = candidates
        .iter()
        .skip(1)
        .map(|(start, _, _)| start - 1)
        .chain([last])
        .collect();
    let mut snapshots = Vec::new();
    let mut answered: Vec<Option<ResultSpan>> = vec![None; requested.len()];
    let mut empty: Vec<bool> = vec![false; requested.len()];
    for (((start, entry, body), end), position) in candidates.into_iter().zip(ends).zip(answers) {
        let first = (body + 1).min(end.max(body));
        let (span, page) = lines.span(first, end.max(first));
        answered[position] = Some(lines.span(start, body).0);
        if page.is_empty() {
            empty[position] = true;
        } else {
            snapshots.push(captured(SourceKind::Page, entry, page, span));
        }
    }
    let gaps = (0..requested.len())
        .filter_map(|position| match answered[position] {
            Some(header) if empty[position] => {
                Some(gap(position, SourceGapReason::Empty, Some(header)))
            }
            Some(_) => None,
            None => match failed_at(position) {
                Some(span) => Some(gap(position, SourceGapReason::Failed, Some(span))),
                None => Some(gap(position, SourceGapReason::Missing, None)),
            },
        })
        .collect();
    ResearchCapture { snapshots, gaps }
}

/// The requested URL that an error line names: the longest requested URL that the line starts
/// with, before `": "` and the reason. The reason can contain `": "` too.
fn error_url(rest: &str, requested: &[String]) -> Option<usize> {
    requested
        .iter()
        .enumerate()
        .filter(|(_, url)| {
            [url.as_str(), url.trim_end_matches('/')].iter().any(|url| {
                rest.strip_prefix(url)
                    .is_some_and(|tail| tail.starts_with(": "))
            })
        })
        .max_by_key(|(_, url)| url.len())
        .map(|(position, _)| position)
}

/// Two URLs are the same when they differ at most by a trailing slash.
fn same_url(url: &str, locator: &str) -> bool {
    url.trim_end_matches('/') == locator.trim_end_matches('/')
}

fn captured(kind: SourceKind, entry: Entry, text: &str, span: ResultSpan) -> CapturedSource {
    CapturedSource {
        kind,
        locator: entry.locator,
        title: entry.title,
        published: entry.published,
        digest: format!(
            "sha256:{}",
            crate::project_archive::hex_sha256(text.as_bytes())
        ),
        text: text.to_owned(),
        span,
    }
}

/// A reported header value, or `None` for the value that the server writes when it has none and
/// for a value above `max_chars`, which the store cannot keep.
fn reported(value: &str, none: &str, max_chars: usize) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value != none && value.chars().count() <= max_chars)
        .then(|| value.to_owned())
}

#[cfg(test)]
#[path = "research_source_tests.rs"]
mod tests;
