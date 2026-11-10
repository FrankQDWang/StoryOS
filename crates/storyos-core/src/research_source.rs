//! The Source Snapshots and Source Gaps of one settled research ToolCall (research provenance
//! contract section 7). The Tool Registration names the text format of each tool result, so the
//! Snapshot does not name the service (ADR 0084). The result text that the Tool Gateway validated
//! is split once into entries: each entry with a URL is one Snapshot, and each requested URL that
//! the server reports as failed, empty, or missing is one Gap. A Gap is never a citation.

use crate::{ToolAttemptSettlement, ToolCallSettlement, ToolCallState, ToolIntake};

/// The bound of the stored text of one Source Snapshot, with its truncation mark.
pub const SOURCE_TEXT_MAX_BYTES: usize = 256 * 1024;
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
}

impl SourceGapReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Failed => "failed",
            Self::Empty => "empty",
            Self::Missing => "missing",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [Self::Failed, Self::Empty, Self::Missing]
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
    /// The snippet of a search entry or the text of a page, bounded with its truncation mark.
    pub text: String,
    /// The byte length of the text before the bound.
    pub original_bytes: usize,
    pub truncated: bool,
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
            let body = body.trim_end();
            let body = body.strip_suffix("---").unwrap_or(body).trim();
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

/// The pages of a page read. StoryOS identifies a page only by a requested URL: a page starts at
/// a header whose URL is a requested URL that no earlier page answered, at the first line or after
/// an empty line, with an empty line or the end of the text after the header. A header with another URL, for example of
/// a page that the server read at a changed URL, ends the page before it and gives no Snapshot, so
/// that no page text goes under the wrong URL; its requested URL gets a `missing` Gap. Each other
/// line is page text.
fn page_blocks(text: &str, requested: &[String]) -> ResearchCapture {
    let mut lines: Vec<&str> = text.split('\n').collect();
    let mut failed: Vec<usize> = Vec::new();
    while let Some(last) = lines.last() {
        if last.trim().is_empty() {
            lines.pop();
        } else if let Some(position) = last
            .strip_prefix("Error fetching ")
            .and_then(|rest| error_url(rest, requested))
        {
            failed.push(position);
            lines.pop();
        } else {
            break;
        }
    }
    // Each page: the requested URL that it answers, or `None` for a page at another URL.
    let mut pages: Vec<(Option<usize>, Entry<'_>)> = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let at_boundary = index == 0 || lines[index - 1].trim().is_empty();
        let started = header(&lines, index, "# ", "(no title)")
            .filter(|(_, body)| at_boundary && lines.get(*body).is_none_or(|line| line.is_empty()));
        match started {
            Some((entry, body)) => {
                let answered = requested
                    .iter()
                    .position(|url| same_url(url, &entry.locator));
                let answered = answered.filter(|position| {
                    !failed.contains(position)
                        && !pages.iter().any(|(page, _)| *page == Some(*position))
                });
                pages.push((answered, entry));
                index = body;
            }
            None => {
                if let Some((_, entry)) = pages.last_mut() {
                    entry.body.push(lines[index]);
                }
                index += 1;
            }
        }
    }
    let mut snapshots = Vec::new();
    let mut answered: Vec<Option<bool>> = vec![None; requested.len()];
    for (position, entry) in pages {
        let Some(position) = position else { continue };
        let body = entry.body.join("\n").trim().to_owned();
        answered[position] = Some(!body.is_empty());
        if !body.is_empty() {
            snapshots.push(captured(SourceKind::Page, entry, &body));
        }
    }
    let gaps = requested
        .iter()
        .enumerate()
        .filter_map(|(position, url)| {
            let reason = match answered[position] {
                Some(true) => return None,
                Some(false) => SourceGapReason::Empty,
                None if failed.contains(&position) => SourceGapReason::Failed,
                None => SourceGapReason::Missing,
            };
            Some(SourceGap {
                locator: url.clone(),
                reason,
            })
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
    let (stored, truncated) = bounded(text);
    CapturedSource {
        kind,
        locator: entry.locator,
        title: entry.title,
        published: entry.published,
        digest: format!(
            "sha256:{}",
            crate::project_archive::hex_sha256(stored.as_bytes())
        ),
        original_bytes: text.len(),
        text: stored,
        truncated,
    }
}

/// The text kept at or below `SOURCE_TEXT_MAX_BYTES`. A longer text keeps a prefix and ends with
/// an explicit mark that gives the kept and the total byte counts.
fn bounded(text: &str) -> (String, bool) {
    if text.len() <= SOURCE_TEXT_MAX_BYTES {
        return (text.to_owned(), false);
    }
    let mark = |kept: usize| {
        format!(
            "\n[StoryOS cut this source here. It keeps {kept} of its {} bytes.]",
            text.len()
        )
    };
    // The mark with the largest kept count is the longest mark.
    let mut end = SOURCE_TEXT_MAX_BYTES - mark(text.len()).len();
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (format!("{}{}", &text[..end], mark(end)), true)
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
