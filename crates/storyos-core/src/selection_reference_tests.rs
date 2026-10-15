use super::*;

fn point(block: &str, offset: u32) -> TextPoint {
    TextPoint {
        manuscript_block_id: block.to_owned(),
        offset,
    }
}

#[test]
fn a_chapter_selection_keeps_the_exact_range_across_blocks() {
    let blocks = [
        ("b1", "雨夜。渡口"),
        ("b2", "船夫😀等着"),
        ("b3", "灯灭了。"),
    ]
    .map(|(id, text)| (id.to_owned(), text.to_owned()));

    assert_eq!(
        [
            select_chapter_text(&blocks, &point("b1", 3), &point("b3", 2)),
            select_chapter_text(&blocks, &point("b2", 2), &point("b2", 4)),
            select_chapter_text(&blocks, &point("b1", 5), &point("b2", 0)),
            select_chapter_text(&blocks, &point("b2", 0), &point("b1", 1)),
            select_chapter_text(&blocks, &point("b2", 3), &point("b2", 5)),
            select_chapter_text(&blocks, &point("b3", 0), &point("b3", 5)),
        ],
        [
            Some((
                "渡口\n船夫😀等着\n灯灭".to_owned(),
                vec!["b1".to_owned(), "b2".to_owned(), "b3".to_owned()]
            )),
            Some(("😀".to_owned(), vec!["b2".to_owned()])),
            Some(("\n".to_owned(), vec!["b1".to_owned(), "b2".to_owned()])),
            None,
            None,
            None,
        ]
    );
}
