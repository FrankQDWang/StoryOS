//! Qualify bounded human passage references for the Host-fake request profile.

use serde::{Deserialize, Serialize};

pub const ORDINARY_PASSAGE_PROFILE: &str = "storyos.host-fake.passage-resolution.v1";
// The finite fake metadata profile uses the absolute Query ceilings in protocol section 15.2.
pub const ORDINARY_REFERENCE_POSITION_LIMIT: usize = 500;
pub const ORDINARY_REFERENCE_CLAUSE_LIMIT: usize = 128;
pub const PASSAGE_REFERENCE_QUESTION: &str =
    "Which unique Chapter and paragraph numbers should I revise?";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OrdinaryPassageResolution {
    Resolved,
    Clarification,
}

#[derive(Debug, Eq, PartialEq)]
pub enum HumanChapterReference {
    Title(String),
    Ordinal(usize),
    Current,
}

#[derive(Debug, Eq, PartialEq)]
pub struct HumanPassageReference {
    pub chapter: HumanChapterReference,
    pub first: usize,
    pub last: usize,
    pub from_end: bool,
}

/// Legacy input has no resolution; a malformed qualified request needs clarification.
pub fn parse_ordinary_passage_request(
    text: &str,
) -> Option<Result<Vec<HumanPassageReference>, ()>> {
    if crate::assemble_context::count_context_item_tokens(text) > crate::CONTEXT_ITEM_TOKEN_LIMIT {
        return None;
    }
    let text = text
        .trim()
        .trim_start_matches("Please ")
        .trim_start_matches("please ")
        .trim_start_matches('请');
    let lower = text.to_ascii_lowercase();
    if ![
        "revise ", "rewrite ", "tighten ", "修改", "改写", "润色", "精简",
    ]
    .iter()
    .any(|verb| lower.starts_with(verb))
        || [
            "Revise this passage:",
            "Revise these passages",
            "Revise this phrase:",
            "Tighten this paragraph",
        ]
        .iter()
        .any(|prefix| text.starts_with(prefix))
    {
        return None;
    }
    let quoted = text.replace(['“', '”', '《', '》'], "\"");
    let normalized = quoted
        .split('"')
        .enumerate()
        .map(|(index, part)| {
            if index % 2 == 0 {
                part.replace(" and ", "|")
                    .replace(['和', '、', '；', ';'], "|")
            } else {
                part.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\"");
    if normalized.split('|').count() > ORDINARY_REFERENCE_CLAUSE_LIMIT {
        return Some(Err(()));
    }
    Some(
        normalized
            .split('|')
            .map(|clause| {
                let mut remainder = clause.to_owned();
                let chapter = if let Some(start) = clause.find('"') {
                    let end = clause[start + 1..].find('"').ok_or(())? + start + 1;
                    let title = clause[start + 1..end].to_owned();
                    remainder.replace_range(start..=end, "");
                    HumanChapterReference::Title(title)
                } else if let Some((before, after)) = clause.split_once("chapter ") {
                    if before.ends_with("this ") || before.ends_with("current ") {
                        HumanChapterReference::Current
                    } else {
                        let ordinal = after.split_whitespace().next().ok_or(())?;
                        remainder = before.to_owned() + after.strip_prefix(ordinal).ok_or(())?;
                        {
                            let ordinal = number(ordinal).ok_or(())?;
                            if ordinal == 0 || ordinal > ORDINARY_REFERENCE_POSITION_LIMIT {
                                return Err(());
                            }
                            HumanChapterReference::Ordinal(ordinal)
                        }
                    }
                } else if clause.contains("当前章") || clause.contains("本章") {
                    HumanChapterReference::Current
                } else {
                    return Err(());
                };
                let lower = remainder.to_ascii_lowercase();
                let from_end = lower.contains("last ") || lower.contains('后');
                let first_range = lower.contains("first ") || lower.contains('前') || from_end;
                let words = lower.replace(['第', '前', '后', '段', '至', '-', '到'], " ");
                let tokens: Vec<_> = words.split_whitespace().collect();
                if tokens.iter().any(|word| {
                    word.bytes().all(|byte| byte.is_ascii_digit()) && number(word).is_none()
                }) {
                    return Err(());
                }
                let numbers: Vec<_> = tokens
                    .into_iter()
                    .filter(|word| !first_range || *word != "first")
                    .filter_map(number)
                    .collect();
                let (first, last) = match numbers.as_slice() {
                    [] if lower.contains("first paragraph") || lower.contains("last paragraph") => {
                        (1, 1)
                    }
                    [count] if first_range => (1, *count),
                    [position] => (*position, *position),
                    [first, last] if !first_range => (*first, *last),
                    _ => return Err(()),
                };
                if first == 0 || last < first || last > ORDINARY_REFERENCE_POSITION_LIMIT {
                    return Err(());
                }
                Ok(HumanPassageReference {
                    chapter,
                    first,
                    last,
                    from_end,
                })
            })
            .collect(),
    )
}

fn number(word: &str) -> Option<usize> {
    match word.trim_matches(['.', ',', '。', '的']) {
        "first" | "one" | "一" => Some(1),
        "second" | "two" | "二" | "两" => Some(2),
        "third" | "three" | "三" => Some(3),
        "fourth" | "four" | "四" => Some(4),
        "fifth" | "five" | "五" => Some(5),
        value => value.parse().ok(),
    }
}
