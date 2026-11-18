use super::{ChapterListEntry, assemble_chapter_list, chapter_list_text};
use crate::assemble_context::{
    CONTEXT_ITEM_TOKEN_LIMIT, ConsideredSource, ProjectionMode, SelectedProjection,
    count_context_item_tokens,
};
use crate::{
    ContextSourceClass, CurrentPassageAssembly, InstructionBindingInput,
    assemble_current_passage_context, hex_sha256,
};

fn entry(volume: &str, chapter: &str, title: &str, length: u64) -> ChapterListEntry {
    ChapterListEntry {
        volume_order: volume.parse().unwrap(),
        volume_title: format!("Volume {volume}"),
        chapter_id: format!("chapter-{chapter}"),
        title: title.to_owned(),
        length,
    }
}

/// 100 Chapters, 50 in each of two Volumes, each with a title of 3,000 characters. A Chapter line
/// has about 3,010 characters, so 10 Chapter lines and the other lines fit in the limit, and 11 do
/// not.
fn long_manuscript() -> Vec<ChapterListEntry> {
    (1..=100)
        .map(|number| {
            let volume = if number <= 50 { "1" } else { "2" };
            entry(volume, &number.to_string(), &"t".repeat(3_000), 100)
        })
        .collect()
}

/// The expected excerpt of `long_manuscript` with the Chapters `first..=last`.
fn excerpt(first: usize, last: usize, current: usize, not_listed: &str) -> String {
    let mut lines = Vec::new();
    for number in first..=last {
        if number == first || number == 51 {
            let volume = if number <= 50 { 1 } else { 2 };
            lines.push(format!("{{\"volume\":\"Volume {volume}\"}}"));
        }
        let mark = if number == current {
            ",\"current\""
        } else {
            ""
        };
        lines.push(format!("[{number},100,\"{}\"{mark}]", "t".repeat(3_000)));
    }
    lines.push(format!(
        "{{\"chapter_count\":100,\"not_listed\":{not_listed}}}"
    ));
    lines.join("\n")
}

#[test]
fn the_text_lists_each_volume_before_its_chapters_and_marks_the_current_chapter() {
    let entries = [
        entry("1", "a", "The \"Harbor\"", 1_204),
        entry("1", "b", "Fog", 0),
        entry("2", "c", "雨", 37),
    ];

    assert_eq!(
        chapter_list_text(&entries, "chapter-b"),
        (
            "{\"volume\":\"Volume 1\"}\n\
             [1,1204,\"The \\\"Harbor\\\"\"]\n\
             [2,0,\"Fog\",\"current\"]\n\
             {\"volume\":\"Volume 2\"}\n\
             [3,37,\"雨\"]"
                .to_owned(),
            ProjectionMode::ExactRequired
        )
    );
}

/// The item records its text with the source version of the text, after the items of before.
#[test]
fn the_item_is_one_exact_chapter_list_item_with_the_digest_of_its_text() {
    let mut record = assemble_current_passage_context(&CurrentPassageAssembly {
        operation_requirement_id: "requirement-1".to_owned(),
        input_snapshot_id: "snapshot-1".to_owned(),
        run_id: "run-1".to_owned(),
        owner_user_id: "owner-1".to_owned(),
        project_id: "project-1".to_owned(),
        author_message: "Help.".to_owned(),
        chapter_id: "chapter-a".to_owned(),
        chapter_revision_id: Some("rev-1".to_owned()),
        proposal_target_block_ids: None,
        chapter_body: "Rain.".to_owned(),
        instruction: InstructionBindingInput::Absent,
        destination_identity: "destination-1".to_owned(),
    });
    let before = record.selected.clone();
    let entries = [entry("1", "a", "Harbor", 5)];
    let text = "{\"volume\":\"Volume 1\"}\n[1,5,\"Harbor\",\"current\"]".to_owned();
    let source_version = format!("sha256:{}", hex_sha256(text.as_bytes()));
    let token_count = count_context_item_tokens(&text);

    assemble_chapter_list(&mut record, &entries);

    assert_eq!(
        (
            record.selected.split_last(),
            record.considered.last().cloned()
        ),
        (
            Some((
                &SelectedProjection {
                    source_class: ContextSourceClass::ChapterList,
                    source_version: source_version.clone(),
                    projection_mode: ProjectionMode::ExactRequired,
                    token_count,
                    content: text,
                },
                before.as_slice()
            )),
            Some(ConsideredSource {
                source_class: ContextSourceClass::ChapterList,
                source_version,
                token_count,
                eligible: true,
            })
        )
    );
}

/// ADR 0052: a list of exactly the limit is exact, and one more character makes an excerpt in
/// the limit.
#[test]
fn a_list_at_the_limit_is_exact_and_one_more_character_makes_an_excerpt() {
    let with_last_title = |length: usize| {
        let mut entries = long_manuscript()[..9].to_vec();
        entries.push(entry("1", "10", &"t".repeat(length), 100));
        entries
    };
    let (short, _) = chapter_list_text(&with_last_title(1), "chapter-1");
    let at_limit =
        1 + usize::try_from(CONTEXT_ITEM_TOKEN_LIMIT - count_context_item_tokens(&short)).unwrap();
    let (text, mode) = chapter_list_text(&with_last_title(at_limit), "chapter-1");
    let (above, above_mode) = chapter_list_text(&with_last_title(at_limit + 1), "chapter-1");

    assert_eq!(
        (count_context_item_tokens(&text), mode, above_mode),
        (
            CONTEXT_ITEM_TOKEN_LIMIT,
            ProjectionMode::ExactRequired,
            ProjectionMode::DeterministicExcerpt
        )
    );
    assert!(count_context_item_tokens(&above) <= CONTEXT_ITEM_TOKEN_LIMIT);
}

/// ADR 0095 section 1: above the limit, the Host takes the Current Chapter, then the Chapters
/// before it from the nearest, then the Chapters after it from the nearest. The first line that
/// does not fit ends the selection. The excerpt keeps manuscript order and the line of each Volume
/// that has a kept Chapter, and its last line gives the Chapter count and each range that it does
/// not list.
#[test]
fn a_list_above_the_limit_keeps_the_chapters_nearest_the_current_chapter() {
    let entries = long_manuscript();
    let list = |current: usize| chapter_list_text(&entries, &format!("chapter-{current}"));
    let excerpts = [
        (1, excerpt(1, 10, 1, "[[11,100]]")),
        (50, excerpt(41, 50, 50, "[[1,40],[51,100]]")),
        (52, excerpt(43, 52, 52, "[[1,42],[53,100]]")),
        (5, excerpt(1, 10, 5, "[[11,100]]")),
        (100, excerpt(91, 100, 100, "[[1,90]]")),
    ];

    for (current, expected) in excerpts {
        let (text, mode) = list(current);
        assert!(count_context_item_tokens(&text) <= CONTEXT_ITEM_TOKEN_LIMIT);
        assert_eq!(
            (text, mode),
            (expected, ProjectionMode::DeterministicExcerpt)
        );
    }
}
