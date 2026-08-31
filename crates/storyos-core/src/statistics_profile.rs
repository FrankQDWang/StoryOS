//! Pinned Unicode 16.0.0 writing-statistics profile.

pub const STATISTICS_COUNTING_PROFILE: &str = "storyos.statistics.unicode-16.0.0.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextStatistics {
    pub word_count: u64,
    pub character_count: u64,
}

/// Count stored text without NFC rewrite.
///
/// Character count is Unicode scalar values. Word count is maximal non-empty
/// runs bounded by Unicode 16.0.0 `White_Space`.
pub fn count_stored_text(text: &str) -> TextStatistics {
    let character_count = u64::try_from(text.chars().count()).unwrap_or(u64::MAX);
    let mut word_count = 0_u64;
    let mut in_word = false;
    for scalar in text.chars() {
        if is_unicode_16_white_space(scalar) {
            in_word = false;
        } else if !in_word {
            word_count = word_count.saturating_add(1);
            in_word = true;
        }
    }
    TextStatistics {
        word_count,
        character_count,
    }
}

/// Count Block texts after the same newline join used for Chapter display body.
pub fn count_stored_texts<'a, I>(texts: I) -> TextStatistics
where
    I: IntoIterator<Item = &'a str>,
{
    count_stored_text(&texts.into_iter().collect::<Vec<_>>().join("\n"))
}

fn is_unicode_16_white_space(scalar: char) -> bool {
    matches!(
        scalar,
        '\u{0009}'..='\u{000D}'
            | '\u{0020}'
            | '\u{0085}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
    )
}

#[cfg(test)]
#[path = "statistics_profile_tests.rs"]
mod tests;
