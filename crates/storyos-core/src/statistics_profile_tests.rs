use super::*;

#[test]
fn unicode_16_profile_matches_pinned_golden_cases() {
    let golden: serde_json::Value =
        serde_json::from_str(include_str!("statistics_unicode_16_0_0_v1.golden.json"))
            .expect("the statistics golden file must be JSON");
    assert_eq!(
        golden["profile"],
        serde_json::Value::String(STATISTICS_COUNTING_PROFILE.to_owned())
    );
    for case in golden["cases"]
        .as_array()
        .expect("golden cases must be an array")
    {
        let name = case["name"].as_str().expect("case name");
        let text = case["text"].as_str().expect("case text");
        assert_eq!(
            count_stored_text(text),
            TextStatistics {
                word_count: case["word_count"].as_u64().expect("word_count"),
                character_count: case["character_count"].as_u64().expect("character_count"),
            },
            "{name}"
        );
    }
}

#[test]
fn chapter_blocks_join_like_display_body_before_counting() {
    assert_eq!(
        count_stored_texts(["Hello", "world"]),
        TextStatistics {
            word_count: 2,
            character_count: 11,
        }
    );
}

#[test]
fn empty_blocks_still_contribute_their_separating_lf() {
    // Join reference: "\nHello\n\nworld\n" = 10 scalars + 4 LF separators.
    assert_eq!(
        count_stored_texts(["", "Hello", "", "world", ""]),
        TextStatistics {
            word_count: 2,
            character_count: 14,
        }
    );
    assert_eq!(
        count_stored_texts(std::iter::empty::<&str>()),
        TextStatistics {
            word_count: 0,
            character_count: 0,
        }
    );
    assert_eq!(
        count_stored_texts(["", ""]),
        TextStatistics {
            word_count: 0,
            character_count: 1,
        }
    );
}

#[test]
fn block_stream_counts_equal_the_joined_chapter_reference() {
    // Representative multi-Block Chapter: Chinese and English prose, an empty
    // Block, ideographic space, an astral scalar, and trailing whitespace.
    let blocks = [
        "第一章\u{3000}风起于青萍之末。",
        "He said: \"go\" — 然后离开了.",
        "",
        "🌊 waves crash\ninside one Block",
        "  leading and trailing  ",
    ];
    assert_eq!(
        count_stored_texts(blocks),
        count_stored_text(&blocks.join("\n"))
    );
}
