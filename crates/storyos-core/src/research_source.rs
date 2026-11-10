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

/// One captured entry, before the store gives it its identity and capture time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapturedSource {
    pub kind: SourceKind,
    /// The URL that the server reported: the Evidence Locator of the entry.
    pub locator: String,
    pub title: Option<String>,
    /// The published date as the server reported it.
    pub published: Option<String>,
    /// The snippet of a search entry or the text of a page, as the server reported it. It is at
    /// most `TOOL_RESULT_TEXT_MAX_BYTES`, because the Tool Gateway fails a longer result before
    /// any capture: StoryOS never cuts a source.
    pub text: String,
    /// The SHA-256 of the stored text bytes.
    pub digest: String,
}

/// One requested URL with no Snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceGap {
    pub locator: String,
    pub reason: SourceGapReason,
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
        ToolIntake::WebSearch { .. } => &[],
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

/// One entry of a result: its header lines, then its body lines.
struct Entry<'a> {
    title: Option<String>,
    locator: String,
    published: Option<String>,
    body: Vec<&'a str>,
}

/// The header of the entry that starts at `lines[start]`: a line with `title_prefix`, a `URL:`
/// line, and optional `Published:` and `Author:` lines. Returns the entry with an empty body and
/// the index of its first body line.
fn header<'a>(
    lines: &[&'a str],
    start: usize,
    title_prefix: &str,
    no_title: &str,
) -> Option<(Entry<'a>, usize)> {
    let title = lines.get(start)?.strip_prefix(title_prefix)?;
    let locator = lines.get(start + 1)?.strip_prefix("URL: ")?.trim();
    let mut next = start + 2;
    let mut published = None;
    while let Some(line) = lines.get(next) {
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
        body: Vec::new(),
    };
    Some((entry, next))
}

/// The entries of a search result. An entry starts at the first line, or after the separator
/// lines `""`, `"---"`, `""` that the server writes between two entries, so that a snippet line
/// cannot start an entry. An entry with no URL or with a URL above the locator bound is dropped.
fn titled_entries(text: &str) -> Vec<CapturedSource> {
    let lines: Vec<&str> = text.split('\n').collect();
    let separated =
        |index: usize| index == 0 || (index >= 3 && lines[index - 3..index] == ["", "---", ""]);
    let mut entries: Vec<Entry<'_>> = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        match header(&lines, index, "Title: ", "N/A").filter(|_| separated(index)) {
            Some((entry, body)) => {
                // The separator before a confirmed entry is not snippet text of the entry before.
                if let Some(previous) = entries.last_mut() {
                    previous
                        .body
                        .truncate(previous.body.len().saturating_sub(3));
                }
                entries.push(entry);
                index = body;
            }
            None => {
                if let Some(entry) = entries.last_mut() {
                    entry.body.push(lines[index]);
                }
                index += 1;
            }
        }
    }
    entries
        .into_iter()
        .filter(|entry| {
            !entry.locator.is_empty() && entry.locator.chars().count() <= SOURCE_LOCATOR_MAX_CHARS
        })
        .map(|entry| {
            let body = entry.body.join("\n");
            let body = body.trim();
            let snippet = body
                .strip_prefix("Highlights:")
                .or_else(|| body.strip_prefix("Text:"))
                .unwrap_or(body)
                .trim()
                .to_owned();
            captured(SourceKind::SearchResult, entry, &snippet)
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
/// one, each candidate starts one page, which ends at the next candidate. Otherwise, for example
/// when a page quotes a header or when the server read a page at a changed URL, StoryOS cannot
/// split the text with certainty: it keeps no Snapshot, and each requested URL that did not fail
/// gets an `unattributed` Gap. StoryOS never guesses a split and never cuts a page.
fn page_blocks(text: &str, requested: &[String]) -> ResearchCapture {
    let mut lines: Vec<&str> = text.split('\n').collect();
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    let candidates: Vec<(usize, Entry<'_>, usize)> = (0..lines.len())
        .filter(|&index| index == 0 || lines[index - 1].trim().is_empty())
        .filter_map(|index| {
            header(&lines, index, "# ", "(no title)")
                .filter(|(_, body)| lines.get(*body).is_none_or(|line| line.is_empty()))
                .map(|(entry, body)| (index, entry, body))
        })
        .collect();
    let has_candidate = |position: usize| {
        candidates
            .iter()
            .any(|(_, entry, _)| same_url(&requested[position], &entry.locator))
    };
    let mut failed: Vec<usize> = Vec::new();
    while let Some(last) = lines.last() {
        let named = last
            .strip_prefix("Error fetching ")
            .and_then(|rest| error_url(rest, requested))
            .filter(|position| !has_candidate(*position));
        match named {
            Some(position) => failed.push(position),
            None if last.trim().is_empty() => {}
            None => break,
        }
        lines.pop();
    }
    // Each candidate answers the next requested URL, in request order, that did not fail.
    let mut answers: Vec<usize> = Vec::new();
    let attributed = candidates.iter().all(|(start, entry, _)| {
        let next = answers.last().map_or(0, |last| last + 1);
        let answer = (next..requested.len()).find(|position| {
            !failed.contains(position) && same_url(&requested[*position], &entry.locator)
        });
        answer.is_some_and(|answer| {
            answers.push(answer);
            *start < lines.len()
        })
    }) && lines[..candidates
        .first()
        .map_or(lines.len(), |(start, _, _)| *start)]
        .iter()
        .all(|line| line.trim().is_empty());
    let gap = |position: usize, reason| SourceGap {
        locator: requested[position].clone(),
        reason,
    };
    if !attributed {
        return ResearchCapture {
            snapshots: Vec::new(),
            gaps: (0..requested.len())
                .map(|position| {
                    let failed = failed.contains(&position);
                    gap(
                        position,
                        if failed {
                            SourceGapReason::Failed
                        } else {
                            SourceGapReason::Unattributed
                        },
                    )
                })
                .collect(),
        };
    }
    let mut snapshots = Vec::new();
    let mut answered: Vec<Option<bool>> = vec![None; requested.len()];
    let ends = candidates
        .iter()
        .skip(1)
        .map(|(start, _, _)| *start)
        .chain([lines.len()])
        .collect::<Vec<_>>();
    for (((_, entry, body), end), position) in candidates.into_iter().zip(ends).zip(answers) {
        let text = lines[body.min(end)..end].join("\n").trim().to_owned();
        answered[position] = Some(!text.is_empty());
        if !text.is_empty() {
            snapshots.push(captured(SourceKind::Page, entry, &text));
        }
    }
    let gaps = (0..requested.len())
        .filter_map(|position| match answered[position] {
            Some(true) => None,
            Some(false) => Some(gap(position, SourceGapReason::Empty)),
            None if failed.contains(&position) => Some(gap(position, SourceGapReason::Failed)),
            None => Some(gap(position, SourceGapReason::Missing)),
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

fn captured(kind: SourceKind, entry: Entry<'_>, text: &str) -> CapturedSource {
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
