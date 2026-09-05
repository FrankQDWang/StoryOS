//! Focused benchmark for the writing-statistics counting path.
//!
//! Compares representative multi-Block Chapters between the shipped
//! streaming `count_stored_texts` and a bench-local joined-String reference
//! that reproduces the earlier collect-join-then-count shape. The reference
//! stays in this file only; it is not product code.
//!
//! Run with: `cargo bench -p storyos-core`

use std::hint::black_box;
use std::time::{Duration, Instant};

use storyos_core::{TextStatistics, count_stored_text, count_stored_texts};

/// The earlier Chapter counting shape: collect Block references, join the
/// complete Chapter with LF, then count the temporary String.
fn joined_reference(texts: &[String]) -> TextStatistics {
    count_stored_text(
        &texts
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// One representative multi-Block Chapter: mixed Chinese and English prose,
/// empty Blocks, ideographic space, astral scalars, and inner whitespace.
fn representative_chapter(block_count: usize) -> Vec<String> {
    let prose = [
        "第一章\u{3000}风起于青萍之末，浪成于微澜之间。",
        "He said: \"go on\" — 然后她合上了笔记本.",
        "",
        "🌊 The tide answered\nacross the harbor wall.",
        "  多余的空白  keeps its exact scalars.  ",
    ];
    (0..block_count)
        .map(|index| prose[index % prose.len()].repeat(1 + index % 3))
        .collect()
}

fn measure<F: FnMut() -> TextStatistics>(rounds: u32, count: &mut F) -> (Duration, TextStatistics) {
    let mut result = TextStatistics {
        word_count: 0,
        character_count: 0,
    };
    let start = Instant::now();
    for _ in 0..rounds {
        result = count();
    }
    (start.elapsed() / rounds, result)
}

fn main() {
    const ROUNDS: u32 = 200;
    const SAMPLES: u32 = 7;
    for block_count in [16_usize, 256, 2048] {
        let chapter = representative_chapter(block_count);
        let mut streaming_count =
            || count_stored_texts(black_box(&chapter).iter().map(String::as_str));
        let mut joined_count = || joined_reference(black_box(&chapter));
        let mut best_streaming = Duration::MAX;
        let mut best_joined = Duration::MAX;
        let mut streaming = TextStatistics {
            word_count: 0,
            character_count: 0,
        };
        let mut joined = streaming;
        // Alternate path order on each sample and keep each path's fastest
        // sample so a one-sided warmup or scheduler pause does not bias the
        // comparison.
        for sample in 0..SAMPLES {
            let (streaming_elapsed, streaming_result, joined_elapsed, joined_result) =
                if sample % 2 == 0 {
                    let streaming_sample = measure(ROUNDS, &mut streaming_count);
                    let joined_sample = measure(ROUNDS, &mut joined_count);
                    (
                        streaming_sample.0,
                        streaming_sample.1,
                        joined_sample.0,
                        joined_sample.1,
                    )
                } else {
                    let joined_sample = measure(ROUNDS, &mut joined_count);
                    let streaming_sample = measure(ROUNDS, &mut streaming_count);
                    (
                        streaming_sample.0,
                        streaming_sample.1,
                        joined_sample.0,
                        joined_sample.1,
                    )
                };
            best_streaming = best_streaming.min(streaming_elapsed);
            best_joined = best_joined.min(joined_elapsed);
            streaming = streaming_result;
            joined = joined_result;
        }
        assert_eq!(
            streaming, joined,
            "streaming and joined-reference counts must agree"
        );
        println!(
            "blocks={block_count} characters={} words={} streaming={best_streaming:?} joined_reference={best_joined:?}",
            streaming.character_count, streaming.word_count
        );
    }
}
