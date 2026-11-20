//! The ranked passages of the whole manuscript for a question (ADR 0095 sections 1 and 2). The
//! Host makes a list by exact terms and a list by vector distance, and joins them with Reciprocal
//! Rank Fusion. The text of each result is the current text of its Chapter, never the index. A
//! rank or a score gives no authority and is not in the result text.

use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;

use crate::read_chapter::bounded_result;
use crate::{PassageBlock, ToolResultText, cut_passages, exact_matches};

/// The number of passages in the result. The coordinator set the initial values on 2026-11-16
/// from the record of S7-C1; S7-C3c records the values of its live run.
pub const PASSAGE_RESULT_LIMIT: usize = 8;
/// The number of passages that each list gives to the fusion.
pub const PASSAGE_LIST_LIMIT: usize = 40;
/// The constant of the Reciprocal Rank Fusion formula, the value of the original paper.
pub const PASSAGE_FUSION_CONSTANT: u32 = 60;
/// The largest number of terms.
pub const PASSAGE_TERM_LIMIT: usize = 8;
/// The length limit of one term, in characters.
pub const PASSAGE_TERM_MAX_CHARS: usize = 64;

const NO_PASSAGE: &str =
    "No passage of the manuscript holds a term or is near the question, and this is not a failure.";

/// One Block of the authoritative head of a live Chapter, in Block order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankingBlock {
    pub block_id: String,
    pub text: String,
}

/// One live Chapter of the Project, in manuscript order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankingChapter {
    pub chapter_id: String,
    pub title: String,
    pub blocks: Vec<RankingBlock>,
}

/// One passage of the current cut, with the numbers of its first Block and of its last Block in
/// the Chapter, from 1.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocatedPassage {
    pub first_block: usize,
    pub last_block: usize,
    pub text: String,
    pub digest: String,
}

/// The current passages of one live Chapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChapterPassages {
    pub chapter_id: String,
    pub title: String,
    pub passages: Vec<LocatedPassage>,
}

/// One passage: the index of its Chapter in manuscript order and its index in the Chapter. The
/// order of the type is the manuscript order.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PassagePlace {
    pub chapter: usize,
    pub passage: usize,
}

/// One nearest vector row: its Chapter and the digest of its passage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NearestPassage {
    pub chapter_id: String,
    pub passage_digest: String,
}

/// The terms of one ranking: at most `PASSAGE_TERM_LIMIT` different terms, each not empty and
/// not longer than `PASSAGE_TERM_MAX_CHARS`, in the order of their first occurrence.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PassageTerms(Vec<String>);

/// Why a list of terms is refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PassageTermsRefusal {
    TooMany,
    Empty,
    TooLong,
}

impl PassageTerms {
    /// Removes each repeated term, then refuses a list above the limit and a term out of bounds.
    pub fn new(terms: &[String]) -> Result<Self, PassageTermsRefusal> {
        let mut seen = BTreeSet::new();
        let terms: Vec<String> = (terms.iter())
            .filter(|term| seen.insert(term.as_str()))
            .cloned()
            .collect();
        if terms.len() > PASSAGE_TERM_LIMIT {
            return Err(PassageTermsRefusal::TooMany);
        }
        for term in &terms {
            match term.chars().count() {
                0 => return Err(PassageTermsRefusal::Empty),
                length if length > PASSAGE_TERM_MAX_CHARS => {
                    return Err(PassageTermsRefusal::TooLong);
                }
                _ => {}
            }
        }
        Ok(Self(terms))
    }

    pub fn as_slice(&self) -> &[String] {
        &self.0
    }
}

/// The current cut of each live Chapter, with the number of the first Block and of the last Block
/// of each passage.
pub fn current_passages(chapters: &[RankingChapter]) -> Vec<ChapterPassages> {
    chapters
        .iter()
        .map(|chapter| {
            let blocks: Vec<PassageBlock<'_>> = (chapter.blocks.iter())
                .map(|block| PassageBlock {
                    block_id: &block.block_id,
                    text: &block.text,
                })
                .collect();
            let number = |block_id: &str| {
                (chapter.blocks.iter())
                    .position(|block| block.block_id == block_id)
                    .map_or(0, |index| index + 1)
            };
            let passages = cut_passages(&blocks)
                .into_iter()
                .map(|passage| LocatedPassage {
                    first_block: number(&passage.first_block_id),
                    last_block: number(&passage.last_block_id),
                    text: passage.text,
                    digest: passage.digest,
                })
                .collect();
            ChapterPassages {
                chapter_id: chapter.chapter_id.clone(),
                title: chapter.title.clone(),
                passages,
            }
        })
        .collect()
}

/// The exact-term list: each current passage that holds one or more of `terms` with the exact
/// match of the author search. More different terms go first, then more matches, then the
/// manuscript order. The list has at most `PASSAGE_LIST_LIMIT` passages.
pub fn exact_term_list(chapters: &[ChapterPassages], terms: &PassageTerms) -> Vec<PassagePlace> {
    let mut found: Vec<(Reverse<usize>, Reverse<usize>, PassagePlace)> = Vec::new();
    for (chapter, read) in chapters.iter().enumerate() {
        for (passage, located) in read.passages.iter().enumerate() {
            let counts: Vec<usize> = (terms.as_slice().iter())
                .map(|term| exact_matches(&located.text, term).count())
                .collect();
            let different = counts.iter().filter(|count| **count > 0).count();
            if different > 0 {
                let matches = counts.iter().sum();
                let place = PassagePlace { chapter, passage };
                found.push((Reverse(different), Reverse(matches), place));
            }
        }
    }
    found.sort();
    (found.into_iter().take(PASSAGE_LIST_LIMIT))
        .map(|(_, _, place)| place)
        .collect()
}

/// The vector list: the place of each row of `nearest`, in distance order. A row is kept only when
/// its Chapter is live and its digest is the digest of a current passage of that Chapter. Each
/// other row is stale and is not in the list. The list has at most `PASSAGE_LIST_LIMIT` passages.
pub fn vector_list(chapters: &[ChapterPassages], nearest: &[NearestPassage]) -> Vec<PassagePlace> {
    let mut places = Vec::new();
    for row in nearest {
        let place = chapters
            .iter()
            .position(|chapter| chapter.chapter_id == row.chapter_id)
            .and_then(|chapter| {
                (chapters[chapter].passages.iter())
                    .position(|passage| passage.digest == row.passage_digest)
                    .map(|passage| PassagePlace { chapter, passage })
            });
        if let Some(place) = place.filter(|place| !places.contains(place)) {
            places.push(place);
        }
    }
    places.truncate(PASSAGE_LIST_LIMIT);
    places
}

/// Joins the exact-term list and the vector list with Reciprocal Rank Fusion. The score of a
/// passage is the sum, over the lists that hold it, of one divided by `PASSAGE_FUSION_CONSTANT`
/// plus its rank in that list, from 1. A higher score goes first, and equal scores go in
/// manuscript order. The score is an exact fraction, so two equal sums are equal (#1696). With
/// two lists, its denominator is at most the product of two ranks, so each comparison fits in
/// `u128` for lists of less than one billion passages. The result has at most
/// `PASSAGE_RESULT_LIMIT` passages.
pub fn fuse_passage_lists(lists: [&[PassagePlace]; 2]) -> Vec<PassagePlace> {
    let mut scores: BTreeMap<PassagePlace, FusedScore> = BTreeMap::new();
    for list in lists {
        for (index, place) in list.iter().enumerate() {
            let rank = u128::from(PASSAGE_FUSION_CONSTANT) + (index as u128 + 1);
            scores.entry(*place).or_default().add_reciprocal(rank);
        }
    }
    let mut ranked: Vec<(FusedScore, PassagePlace)> = (scores.into_iter())
        .map(|(place, score)| (score, place))
        .collect();
    ranked.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
    (ranked.into_iter().take(PASSAGE_RESULT_LIMIT))
        .map(|(_, place)| place)
        .collect()
}

/// A fused score as an exact fraction in lowest terms. The default is zero.
#[derive(Clone, Copy, Debug)]
struct FusedScore {
    numerator: u128,
    denominator: u128,
}

impl Default for FusedScore {
    fn default() -> Self {
        Self {
            numerator: 0,
            denominator: 1,
        }
    }
}

impl FusedScore {
    /// Adds `1 / rank`.
    fn add_reciprocal(&mut self, rank: u128) {
        let numerator = self.numerator * rank + self.denominator;
        let denominator = self.denominator * rank;
        let divisor = greatest_common_divisor(numerator, denominator);
        (self.numerator, self.denominator) = (numerator / divisor, denominator / divisor);
    }
}

impl PartialEq for FusedScore {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for FusedScore {}

impl PartialOrd for FusedScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FusedScore {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.numerator * other.denominator).cmp(&(other.numerator * self.denominator))
    }
}

fn greatest_common_divisor(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left
}

/// The result of the ranked passages `ranked` for the native call `call_id`. A `notice` is the
/// first line, and its cost is in the cost of the header. The next line is
/// the canonical JSON object `{"passages", "passages_returned", "terms"}`. Each passage then has
/// the line of its Chapter, one line with the numbers of its Blocks, and its current text. The
/// result has whole passages in rank order while its `tool_result` item, with its boundary and
/// its escape, fits in the item limit. The first passage that does not fit ends the result with
/// one line. When the first passage does not fit with that line, the result has only the first
/// passage and the line, and the bounded projection cuts it (ADR 0085 section 2).
pub fn admit_passage_ranking(
    chapters: &[ChapterPassages],
    ranked: &[PassagePlace],
    terms: &PassageTerms,
    notice: Option<&str>,
    call_id: &str,
) -> ToolResultText {
    let header = |returned: usize| {
        let header = crate::project_archive::canonical_json(&json!({
            "passages": ranked.len(),
            "passages_returned": returned,
            "terms": terms.as_slice(),
        }));
        match notice {
            Some(notice) => format!("{notice}\n{header}"),
            None => header,
        }
    };
    if ranked.is_empty() {
        return crate::host_tool_result(&format!("{}\n{NO_PASSAGE}", header(0)));
    }
    let parts: Vec<String> = (ranked.iter())
        .map(|place| {
            let chapter = &chapters[place.chapter];
            let passage = &chapter.passages[place.passage];
            let blocks = match passage.first_block == passage.last_block {
                true => format!("[block {}]", passage.first_block),
                false => format!("[blocks {}-{}]", passage.first_block, passage.last_block),
            };
            let line = crate::project_archive::canonical_json(&json!({
                "chapter": place.chapter + 1,
                "title": chapter.title,
                "chapter_id": chapter.chapter_id,
            }));
            format!("\n{line}\n{blocks}\n{}", passage.text)
        })
        .collect();
    let count = parts.len();
    let more = |kept: usize| {
        format!(
            "\n[This result does not have the other {} passages. Read the Blocks near a passage \
             with read_chapter.]",
            count - kept
        )
    };
    bounded_result(&parts, header, more, call_id)
}

#[cfg(test)]
#[path = "passage_ranking_tests.rs"]
mod tests;
