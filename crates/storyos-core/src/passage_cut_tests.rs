use super::*;

/// One sentence of ten characters.
const SENTENCE: &str = "潮水退去，灯塔亮了。";

fn block<'a>(block_id: &'a str, text: &'a str) -> PassageBlock<'a> {
    PassageBlock { block_id, text }
}

/// The first block, the last block, and the length of each passage.
fn shape(passages: &[Passage]) -> Vec<(&str, &str, usize)> {
    passages
        .iter()
        .map(|passage| {
            (
                passage.first_block_id.as_str(),
                passage.last_block_id.as_str(),
                passage.text.chars().count(),
            )
        })
        .collect()
}

#[test]
fn whole_blocks_join_into_one_passage_until_the_next_block_is_above_the_limit() {
    let paragraph = SENTENCE.repeat(30);
    let short = "她没有回头。";
    let blocks = [
        block("b1", &paragraph),
        block("b2", &paragraph),
        block("b3", &paragraph),
        block("b4", short),
    ];

    let passages = cut_passages(&blocks);

    assert_eq!(shape(&passages), [("b1", "b2", 601), ("b3", "b4", 307)]);
    assert_eq!(passages[0].text, format!("{paragraph}\n{paragraph}"));
    assert_eq!(
        passages[0].digest,
        format!("sha256:{}", crate::hex_sha256(passages[0].text.as_bytes()))
    );
    // A passage of exactly the limit is one passage.
    let at_limit = SENTENCE.repeat(80);
    assert_eq!(
        shape(&cut_passages(&[block("b1", &at_limit)])),
        [("b1", "b1", PASSAGE_CHARACTER_LIMIT)]
    );
}

#[test]
fn a_block_with_no_letter_and_no_digit_ends_the_passage_and_is_in_no_passage() {
    let blocks = [
        block("b1", SENTENCE),
        block("b2", "＊＊＊"),
        block("b3", SENTENCE),
        block("b4", ""),
        block("b5", "——"),
        block("b6", "第3章完。"),
    ];

    let passages = cut_passages(&blocks);

    assert_eq!(
        shape(&passages),
        [("b1", "b1", 10), ("b3", "b3", 10), ("b6", "b6", 5)]
    );
    assert!(passages.iter().all(|passage| !passage.text.contains('＊')));
    assert_eq!(cut_passages(&[block("b1", "＊＊＊")]), []);
}

#[test]
fn a_block_above_the_limit_is_cut_at_the_last_sentence_end_before_the_limit() {
    // 79 sentences of 10 characters, then one quoted sentence of 12 characters that ends at
    // character 802, then more text.
    let quoted = "“你回来了。”他说";
    let long = format!("{}{quoted}{}", SENTENCE.repeat(79), SENTENCE.repeat(20));
    let before = format!("{}“你回来了。”", SENTENCE.repeat(79));
    let blocks = [
        block("b0", SENTENCE),
        block("b1", &long),
        block("b2", SENTENCE),
    ];

    let passages = cut_passages(&blocks);

    // The sentence end at character 797 keeps its closing quote; the rest of the block is the
    // next passage.
    assert_eq!(
        shape(&passages),
        [
            ("b0", "b0", 10),
            ("b1", "b1", 797),
            ("b1", "b1", long.chars().count() - 797),
            ("b2", "b2", 10),
        ]
    );
    assert_eq!(passages[1].text, before);
    assert_eq!(format!("{}{}", passages[1].text, passages[2].text), long);
}

#[test]
fn a_block_above_the_limit_with_no_sentence_end_is_cut_at_the_limit() {
    let long = "潮".repeat(1_700);

    let passages = cut_passages(&[block("b1", &long)]);

    assert_eq!(
        shape(&passages),
        [("b1", "b1", 800), ("b1", "b1", 800), ("b1", "b1", 100)]
    );
}

#[test]
fn the_same_blocks_give_the_same_passages_and_a_changed_block_changes_only_its_passage() {
    let paragraph = SENTENCE.repeat(50);
    let changed = format!("{}灯塔熄了。", SENTENCE.repeat(49));
    let first = [block("b1", &paragraph), block("b2", &paragraph)];
    let second = [block("b1", &paragraph), block("b2", &changed)];

    let digests = |blocks: &[PassageBlock<'_>]| {
        cut_passages(blocks)
            .into_iter()
            .map(|passage| passage.digest)
            .collect::<Vec<_>>()
    };

    assert_eq!(cut_passages(&first), cut_passages(&first));
    assert_eq!(digests(&first)[0], digests(&second)[0]);
    assert_ne!(digests(&first)[1], digests(&second)[1]);
    assert_eq!(PASSAGE_CUT_REVISION, "storyos.passage-cut.v1");
}

#[test]
fn the_request_text_has_one_context_line_and_the_digest_does_not_include_it() {
    let passages = cut_passages(&[block("b1", SENTENCE)]);

    assert_eq!(
        passage_embedding_text("第一卷 潮汐", "第三章 灯塔", &passages[0].text),
        "第一卷 潮汐 / 第三章 灯塔\n潮水退去，灯塔亮了。"
    );
    assert_eq!(
        passages[0].digest,
        format!("sha256:{}", crate::hex_sha256(SENTENCE.as_bytes()))
    );
}
