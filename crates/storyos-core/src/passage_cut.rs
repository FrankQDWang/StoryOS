//! The cut of the current text of one Chapter into passages for the passage vectors (ADR 0095
//! section 2). A passage is the unit of one embedding request and of one vector row.

use crate::hex_sha256;

/// The length limit of one passage, in characters. The coordinator set the initial value on
/// 2026-11-16 from the record of S7-C1; S7-C3c records the value of its live run.
pub const PASSAGE_CHARACTER_LIMIT: usize = 800;
/// The revision name of the cut. A change of the rules or of the limit gets a new name, so that
/// each row of an earlier cut becomes stale.
pub const PASSAGE_CUT_REVISION: &str = "storyos.passage-cut.v1";

/// The characters that end a sentence.
const SENTENCE_ENDS: [char; 7] = ['。', '！', '？', '…', '!', '?', '.'];
/// The closing marks that stay with the sentence end before them.
const CLOSING_MARKS: [char; 8] = ['”', '’', '」', '』', '）', '"', '\'', ')'];

/// One block of the Chapter, in block order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PassageBlock<'a> {
    pub block_id: &'a str,
    pub text: &'a str,
}

/// One passage: its first block, its last block, its text, and the digest of its text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Passage {
    pub first_block_id: String,
    pub last_block_id: String,
    pub text: String,
    pub digest: String,
}

/// Cuts the blocks of one Chapter into passages, in block order. A passage is a sequence of whole
/// blocks, joined with a line feed, that is not longer than `PASSAGE_CHARACTER_LIMIT`. A block with
/// no letter and no digit is a scene separator: it ends the passage before it and is in no
/// passage. A block above the limit becomes two or more passages, cut at the last sentence end
/// before the limit, or at the limit when that part has no sentence end. The same blocks always
/// give the same passages.
pub fn cut_passages(blocks: &[PassageBlock<'_>]) -> Vec<Passage> {
    let mut passages = Vec::new();
    let mut open: Vec<PassageBlock<'_>> = Vec::new();
    let mut open_length = 0;
    for block in blocks {
        if is_separator(block.text) {
            close(&mut passages, &mut open);
            continue;
        }
        let length = block.text.chars().count();
        if length > PASSAGE_CHARACTER_LIMIT {
            close(&mut passages, &mut open);
            for part in long_block_parts(block.text) {
                passages.push(passage(block.block_id, block.block_id, part));
            }
            continue;
        }
        let joined = if open.is_empty() {
            length
        } else {
            open_length + 1 + length
        };
        if joined > PASSAGE_CHARACTER_LIMIT {
            close(&mut passages, &mut open);
            open_length = length;
        } else {
            open_length = joined;
        }
        open.push(*block);
    }
    close(&mut passages, &mut open);
    passages
}

/// The text of the embedding request of one passage: one context line with the title of the Volume
/// and the title of the Chapter, and then the text of the passage. The digest of the passage does
/// not include the context line, so a new title does not make a vector stale.
pub fn passage_embedding_text(volume_title: &str, chapter_title: &str, passage: &str) -> String {
    format!("{volume_title} / {chapter_title}\n{passage}")
}

fn is_separator(text: &str) -> bool {
    !text.chars().any(char::is_alphanumeric)
}

fn close(passages: &mut Vec<Passage>, open: &mut Vec<PassageBlock<'_>>) {
    let (Some(first), Some(last)) = (open.first(), open.last()) else {
        return;
    };
    let text = open
        .iter()
        .map(|block| block.text)
        .collect::<Vec<_>>()
        .join("\n");
    passages.push(passage(first.block_id, last.block_id, &text));
    open.clear();
}

fn passage(first_block_id: &str, last_block_id: &str, text: &str) -> Passage {
    Passage {
        first_block_id: first_block_id.to_owned(),
        last_block_id: last_block_id.to_owned(),
        text: text.to_owned(),
        digest: format!("sha256:{}", hex_sha256(text.as_bytes())),
    }
}

/// The parts of one block above the limit. A part with no letter and no digit is dropped.
fn long_block_parts(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut rest = text;
    while rest.chars().count() > PASSAGE_CHARACTER_LIMIT {
        let cut = part_end(rest);
        parts.push(&rest[..cut]);
        rest = &rest[cut..];
    }
    parts.push(rest);
    parts.retain(|part| !is_separator(part));
    parts
}

/// The byte end of the first part of `text`: after the last sentence end, with its closing marks,
/// in the first `PASSAGE_CHARACTER_LIMIT` characters, or after that limit.
fn part_end(text: &str) -> usize {
    let window: Vec<(usize, char)> = text.char_indices().take(PASSAGE_CHARACTER_LIMIT).collect();
    let limit = window
        .last()
        .map_or(0, |(start, character)| start + character.len_utf8());
    let Some(end) = window
        .iter()
        .rposition(|(_, character)| SENTENCE_ENDS.contains(character))
    else {
        return limit;
    };
    let after = window[end + 1..]
        .iter()
        .position(|(_, character)| !CLOSING_MARKS.contains(character))
        .map_or(window.len(), |offset| end + 1 + offset);
    window.get(after).map_or(limit, |(start, _)| *start)
}

#[cfg(test)]
#[path = "passage_cut_tests.rs"]
mod tests;
