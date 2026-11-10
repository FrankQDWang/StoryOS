//! The Source Snapshots and Source Gaps of one settled research ToolCall (research provenance
//! contract section 7). The Tool Registration names the text format of each tool result, so the
//! Snapshot does not name the service (ADR 0084). The result text that the Tool Gateway validated
//! is split once into entries: each entry with a URL is one Snapshot, and each requested URL that
//! the server reports as failed, empty, or missing is one Gap. A Gap is never a citation.

use crate::{ToolAttemptSettlement, ToolCallSettlement, ToolCallState, ToolIntake};

/// The bound of the stored text of one Source Snapshot, with its truncation mark.
pub const SOURCE_TEXT_MAX_BYTES: usize = 256 * 1024;

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

/// Splits `lines` at each line with `title_prefix` that a `URL:` line follows.
fn entries<'a>(lines: &[&'a str], title_prefix: &str, no_title: &str) -> Vec<Entry<'a>> {
    let starts: Vec<usize> = (0..lines.len())
        .filter(|&index| {
            lines[index].starts_with(title_prefix)
                && lines
                    .get(index + 1)
                    .is_some_and(|line| line.starts_with("URL: "))
        })
        .collect();
    starts
        .iter()
        .enumerate()
        .filter_map(|(position, &start)| {
            let end = starts.get(position + 1).copied().unwrap_or(lines.len());
            let locator = lines[start + 1]["URL: ".len()..].trim().to_owned();
            let mut body = &lines[start + 2..end];
            let mut published = None;
            while let Some(line) = body.first() {
                if let Some(value) = line.strip_prefix("Published: ") {
                    published = reported(value, "N/A");
                } else if !line.starts_with("Author: ") {
                    break;
                }
                body = &body[1..];
            }
            (!locator.is_empty()).then(|| Entry {
                title: reported(&lines[start][title_prefix.len()..], no_title),
                locator,
                published,
                body: body.to_vec(),
            })
        })
        .collect()
}

fn titled_entries(text: &str) -> Vec<CapturedSource> {
    let lines: Vec<&str> = text.split('\n').collect();
    entries(&lines, "Title: ", "N/A")
        .into_iter()
        .map(|entry| {
            let body = entry.body.join("\n");
            let body = body.trim_end();
            let body = body.strip_suffix("---").unwrap_or(body).trim();
            let snippet = body
                .strip_prefix("Highlights:")
                .or_else(|| body.strip_prefix("Text:"))
                .unwrap_or(body)
                .trim();
            captured(SourceKind::SearchResult, entry, snippet)
        })
        .collect()
}

fn page_blocks(text: &str, requested: &[String]) -> ResearchCapture {
    let mut lines: Vec<&str> = text.split('\n').collect();
    let mut failed = Vec::new();
    while let Some(last) = lines.last() {
        if last.trim().is_empty() {
            lines.pop();
        } else if let Some((url, _reason)) = last
            .strip_prefix("Error fetching ")
            .and_then(|rest| rest.rsplit_once(": "))
        {
            failed.push(url.trim().to_owned());
            lines.pop();
        } else {
            break;
        }
    }
    let pages = entries(&lines, "# ", "(no title)");
    let same =
        |url: &str, locator: &str| url.trim_end_matches('/') == locator.trim_end_matches('/');
    let mut claimed: Vec<Option<usize>> = vec![None; requested.len()];
    let mut used = vec![false; pages.len()];
    for (position, url) in requested.iter().enumerate() {
        if let Some(index) =
            (0..pages.len()).find(|&index| !used[index] && same(url, &pages[index].locator))
        {
            used[index] = true;
            claimed[position] = Some(index);
        }
    }
    // A page whose URL the server changed, for example after a redirect, answers the next
    // requested URL that no page and no error answered.
    let mut spare = (0..pages.len()).filter(|&index| !used[index]);
    for (position, url) in requested.iter().enumerate() {
        if claimed[position].is_none() && !failed.iter().any(|failed| same(url, failed)) {
            claimed[position] = spare.next();
        }
    }
    let bodies: Vec<String> = pages
        .iter()
        .map(|page| page.body.join("\n").trim().to_owned())
        .collect();
    let gaps = requested
        .iter()
        .zip(&claimed)
        .filter_map(|(url, claimed)| {
            let reason = match claimed {
                Some(index) if bodies[*index].is_empty() => SourceGapReason::Empty,
                Some(_) => return None,
                None if failed.iter().any(|failed| same(url, failed)) => SourceGapReason::Failed,
                None => SourceGapReason::Missing,
            };
            Some(SourceGap {
                locator: url.clone(),
                reason,
            })
        })
        .collect();
    let snapshots = pages
        .into_iter()
        .zip(&bodies)
        .filter(|(_, body)| !body.is_empty())
        .map(|(page, body)| captured(SourceKind::Page, page, body))
        .collect();
    ResearchCapture { snapshots, gaps }
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

/// A reported header value, or `None` for the value that the server writes when it has none.
fn reported(value: &str, none: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value != none).then(|| value.to_owned())
}

#[cfg(test)]
#[path = "research_source_tests.rs"]
mod tests;
