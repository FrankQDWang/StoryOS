//! Pinned Unicode 16.0.0 writing-statistics profile.

pub const STATISTICS_COUNTING_PROFILE: &str = "storyos.statistics.unicode-16.0.0.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextStatistics {
    pub word_count: u64,
    pub character_count: u64,
}

/// One-pass counting state. Pushing every scalar of a text stream produces
/// the same result as counting a materialized copy of that stream, so callers
/// can count joined Blocks without allocating the joined String.
#[derive(Default)]
struct CountingAccumulator {
    word_count: u64,
    character_count: u64,
    in_word: bool,
}

impl CountingAccumulator {
    fn push_scalar(&mut self, scalar: char) {
        self.character_count = self.character_count.saturating_add(1);
        if is_unicode_16_white_space(scalar) {
            self.in_word = false;
        } else if !self.in_word {
            self.word_count = self.word_count.saturating_add(1);
            self.in_word = true;
        }
    }

    fn push_text(&mut self, text: &str) {
        for scalar in text.chars() {
            self.push_scalar(scalar);
        }
    }

    fn finish(self) -> TextStatistics {
        TextStatistics {
            word_count: self.word_count,
            character_count: self.character_count,
        }
    }
}

/// Count stored text without NFC rewrite.
///
/// Character count is Unicode scalar values. Word count is maximal non-empty
/// runs bounded by Unicode 16.0.0 `White_Space`.
pub fn count_stored_text(text: &str) -> TextStatistics {
    let mut accumulator = CountingAccumulator::default();
    accumulator.push_text(text);
    accumulator.finish()
}

/// Count Block texts after the same newline join used for Chapter display body.
///
/// The scalar stream is byte-for-byte the join of the texts with one LF
/// between adjacent Blocks, but no joined String is materialized.
pub fn count_stored_texts<'a, I>(texts: I) -> TextStatistics
where
    I: IntoIterator<Item = &'a str>,
{
    let mut accumulator = CountingAccumulator::default();
    let mut is_first = true;
    for text in texts {
        if is_first {
            is_first = false;
        } else {
            accumulator.push_scalar('\n');
        }
        accumulator.push_text(text);
    }
    accumulator.finish()
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
