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
