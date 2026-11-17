use super::*;
use crate::assemble_context::count_context_item_tokens;
use crate::{CONTEXT_ITEM_TOKEN_LIMIT, ToolResultProjection};

fn terms(values: &[&str]) -> PassageTerms {
    let values: Vec<String> = values.iter().map(|value| (*value).to_owned()).collect();
    PassageTerms::new(&values).unwrap()
}

fn place(chapter: usize, passage: usize) -> PassagePlace {
    PassagePlace { chapter, passage }
}

/// A Chapter whose passages have the texts `texts`, one Block for each passage.
fn chapter(title: &str, texts: &[&str]) -> ChapterPassages {
    ChapterPassages {
        chapter_id: format!("chapter-{title}"),
        title: title.to_owned(),
        passages: (texts.iter().enumerate())
            .map(|(index, text)| LocatedPassage {
                first_block: index + 1,
                last_block: index + 1,
                text: (*text).to_owned(),
                digest: format!("sha256:{title}-{index}"),
            })
            .collect(),
    }
}

/// The token count of the `tool_result` item of the result `text` of the call `call-1`.
fn item_tokens(text: &str) -> u64 {
    count_context_item_tokens(&ToolResultProjection::Ok(text).continuation_text("call-1"))
}

#[test]
fn the_terms_have_no_repeated_term_and_stay_in_their_bounds() {
    let list = |values: &[&str]| {
        let values: Vec<String> = values.iter().map(|value| (*value).to_owned()).collect();
        PassageTerms::new(&values)
    };
    let eight: Vec<String> = (0..8).map(|index| format!("词{index}")).collect();
    let nine: Vec<String> = (0..9).map(|index| format!("词{index}")).collect();
    let longest = "潮".repeat(PASSAGE_TERM_MAX_CHARS);
    let above = "潮".repeat(PASSAGE_TERM_MAX_CHARS + 1);

    assert_eq!(
        (
            list(&["林雪", "渡口", "林雪"]).map(|terms| terms.as_slice().to_vec()),
            PassageTerms::new(&eight).is_ok(),
            PassageTerms::new(&nine),
            list(&[""]),
            list(&[&longest]).is_ok(),
            list(&[&above]),
            list(&[]).map(|terms| terms.as_slice().len()),
        ),
        (
            Ok(vec!["林雪".to_owned(), "渡口".to_owned()]),
            true,
            Err(PassageTermsRefusal::TooMany),
            Err(PassageTermsRefusal::Empty),
            true,
            Err(PassageTermsRefusal::TooLong),
            Ok(0),
        )
    );
}

/// The numbers of the Blocks of a passage come from the current cut: a scene separator has a
/// number and is in no passage.
#[test]
fn each_passage_has_the_numbers_of_its_first_and_last_block() {
    let block = |id: &str, text: &str| RankingBlock {
        block_id: id.to_owned(),
        text: text.to_owned(),
    };
    let chapters = [RankingChapter {
        chapter_id: "chapter-1".to_owned(),
        title: "渡口".to_owned(),
        blocks: vec![
            block("b1", "潮水退去。"),
            block("b2", "灯塔亮了。"),
            block("b3", "＊＊＊"),
            block("b4", "林雪回到渡口。"),
        ],
    }];

    let passages = current_passages(&chapters);

    let shape: Vec<(usize, usize, &str)> = (passages[0].passages.iter())
        .map(|passage| {
            (
                passage.first_block,
                passage.last_block,
                passage.text.as_str(),
            )
        })
        .collect();
    assert_eq!(
        shape,
        [(1, 2, "潮水退去。\n灯塔亮了。"), (4, 4, "林雪回到渡口。")]
    );
    assert_eq!(
        passages[0].passages[1].digest,
        crate::cut_passages(&[PassageBlock {
            block_id: "b4",
            text: "林雪回到渡口。"
        }])[0]
            .digest
    );
}

/// More different terms go first, then more matches, then the Chapter order, then the position
/// in the Chapter. A passage with no term is not in the list, and no term gives no list.
#[test]
fn the_exact_term_list_orders_by_terms_then_matches_then_manuscript_order() {
    let chapters = [
        chapter("一", &["林雪走过渡口。", "风很大。", "林雪，林雪，林雪。"]),
        chapter("二", &["林雪在渡口等船。", "渡口没有人。", "林雪。"]),
    ];

    let list = exact_term_list(&chapters, &terms(&["林雪", "渡口"]));

    assert_eq!(
        list,
        [
            place(0, 0),
            place(1, 0),
            place(0, 2),
            place(1, 1),
            place(1, 2),
        ]
    );
    assert_eq!(
        exact_term_list(&chapters, &terms(&[])),
        Vec::<PassagePlace>::new()
    );
}

#[test]
fn each_list_has_at_most_the_list_limit() {
    let texts = vec!["灯塔"; PASSAGE_LIST_LIMIT + 1];
    let chapters = [chapter("一", &texts)];
    let nearest: Vec<NearestPassage> = (0..=PASSAGE_LIST_LIMIT)
        .map(|index| NearestPassage {
            chapter_id: "chapter-一".to_owned(),
            passage_digest: format!("sha256:一-{index}"),
        })
        .collect();

    assert_eq!(
        (
            exact_term_list(&chapters, &terms(&["灯塔"])).len(),
            vector_list(&chapters, &nearest).len(),
        ),
        (PASSAGE_LIST_LIMIT, PASSAGE_LIST_LIMIT)
    );
}

/// A row of a Chapter that is not live, a row whose digest is not a current passage, and a second
/// row of one passage are not in the vector list. The other rows keep their distance order.
#[test]
fn the_vector_list_keeps_only_the_rows_of_current_passages() {
    let chapters = [chapter("一", &["甲", "乙"]), chapter("二", &["丙"])];
    let row = |chapter: &str, digest: &str| NearestPassage {
        chapter_id: format!("chapter-{chapter}"),
        passage_digest: format!("sha256:{digest}"),
    };

    let list = vector_list(
        &chapters,
        &[
            row("二", "二-0"),
            row("三", "三-0"),
            row("一", "一-9"),
            row("一", "一-1"),
            row("二", "二-0"),
        ],
    );

    assert_eq!(list, [place(1, 0), place(0, 1)]);
}

/// The score is the sum of `1 / (60 + rank)` over the lists that hold the passage. A passage in
/// the two lists goes first. Two passages with equal scores go in manuscript order. The result
/// has at most the result limit.
#[test]
fn the_fusion_sums_the_reciprocal_ranks_and_keeps_manuscript_order_for_equal_scores() {
    let exact = [place(1, 0), place(0, 3), place(0, 1)];
    let vector = [place(0, 2), place(0, 1)];

    let fused = fuse_passage_lists(&[&exact, &vector]);

    // (0, 1): 1/63 + 1/62. (1, 0) and (0, 2): 1/61 each, in manuscript order. (0, 3): 1/62.
    assert_eq!(fused, [place(0, 1), place(0, 2), place(1, 0), place(0, 3)]);
    let many: Vec<PassagePlace> = (0..20).map(|index| place(0, index)).collect();
    assert_eq!(
        fuse_passage_lists(&[&many, &[]]),
        many[..PASSAGE_RESULT_LIMIT]
    );
    assert_eq!(fuse_passage_lists(&[&[], &[]]), Vec::<PassagePlace>::new());
}

/// One header line, then each passage with the line of its Chapter, the numbers of its Blocks,
/// and its current text. No rank and no score is in the text.
#[test]
fn the_result_has_one_header_and_the_chapter_line_blocks_and_text_of_each_passage() {
    let mut chapters = [chapter("一", &["潮水退去。"]), chapter("二", &["林雪。"])];
    chapters[0].passages[0].last_block = 3;
    let ranked = [place(1, 0), place(0, 0)];

    let result = admit_passage_ranking(&chapters, &ranked, &terms(&["林雪"]), "call-1");
    let empty = admit_passage_ranking(&chapters, &[], &terms(&[]), "call-1");

    assert_eq!(
        result.text,
        "{\"passages\":2,\"passages_returned\":2,\"terms\":[\"林雪\"]}\n\
         {\"chapter\":2,\"chapter_id\":\"chapter-二\",\"title\":\"二\"}\n[block 1]\n林雪。\n\
         {\"chapter\":1,\"chapter_id\":\"chapter-一\",\"title\":\"一\"}\n[blocks 1-3]\n潮水退去。"
    );
    assert_eq!(
        empty.text,
        format!("{{\"passages\":0,\"passages_returned\":0,\"terms\":[]}}\n{NO_PASSAGE}")
    );
}

/// ADR 0052 and ADR 0085 section 2: the result keeps whole passages in rank order while its item
/// fits the limit. The first passage that does not fit ends the result with one line. A first
/// passage that does not fit with that line is the only passage, and the bounded projection cuts
/// it.
#[test]
fn the_result_keeps_whole_passages_while_its_item_fits_the_limit() {
    let header = |returned: usize| {
        format!(
            "{{\"passages\":2,\"passages_returned\":{returned},\"terms\":[]}}\n\
             {{\"chapter\":1,\"chapter_id\":\"chapter-一\",\"title\":\"一\"}}\n[block 1]\n"
        )
    };
    let second_line =
        "\n{\"chapter\":1,\"chapter_id\":\"chapter-一\",\"title\":\"一\"}\n[block 2]\n";
    // Each `<` costs four counted tokens after the escape.
    let filler = |tokens: u64| "<".repeat(usize::try_from(tokens / 4).unwrap());
    let result = |first: &str, second: &str| {
        admit_passage_ranking(
            &[chapter("一", &[first, second])],
            &[place(0, 0), place(0, 1)],
            &terms(&[]),
            "call-1",
        )
        .text
    };
    let two = item_tokens(&format!("{}{second_line}", header(2)));
    let first = filler(16_000);
    let room = CONTEXT_ITEM_TOKEN_LIMIT - two - 16_000;
    let at_limit = filler(room - room % 4);
    let exact = result(&first, &at_limit);
    let cut = result(&first, &format!("{at_limit}<<<<"));
    let more = "\n[This result does not have the other 1 passages. Read the Blocks near a passage \
                with read_chapter.]";
    let fills = filler(CONTEXT_ITEM_TOKEN_LIMIT - item_tokens(&header(1)));
    let alone = result(&fills, "林");
    let projected =
        ToolResultProjection::Ok(&alone).bounded_text("call-1", CONTEXT_ITEM_TOKEN_LIMIT);

    assert_eq!(
        (
            exact == format!("{}{first}{second_line}{at_limit}", header(2)),
            item_tokens(&exact) <= CONTEXT_ITEM_TOKEN_LIMIT,
            item_tokens(&exact) + 4 > CONTEXT_ITEM_TOKEN_LIMIT,
            cut == format!("{}{first}{more}", header(1)),
            item_tokens(&cut) <= CONTEXT_ITEM_TOKEN_LIMIT,
            alone == format!("{}{fills}{more}", header(1)),
            projected.kept_result_tokens.is_some(),
            count_context_item_tokens(&projected.text) <= CONTEXT_ITEM_TOKEN_LIMIT,
        ),
        (true, true, true, true, true, true, true, true)
    );
}
