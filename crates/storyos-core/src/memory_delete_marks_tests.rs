use super::*;

fn mark(entry_type: MemoryEntryType, title: &str) -> MemoryDeleteMark {
    MemoryDeleteMark {
        entry_type,
        title: title.to_owned(),
        description: format!("{title}的描述"),
    }
}

/// The marks keep the order of the store, newest first, with one line for each mark.
#[test]
fn the_marks_are_one_line_for_each_deleted_entry_in_their_order() {
    let marks = [
        mark(MemoryEntryType::Character, "林舟"),
        mark(MemoryEntryType::WritingPreference, "短句"),
    ];

    assert_eq!(
        memory_delete_marks_text(&marks),
        (
            [
                r#"{"description":"林舟的描述","title":"林舟","type":"character"}"#,
                r#"{"description":"短句的描述","title":"短句","type":"writing_preference"}"#,
            ]
            .join("\n"),
            ProjectionMode::ExactRequired
        )
    );
}

/// Above the Context item limit, the item holds the newest marks that fit and the count of the
/// other marks, with the mark of a Deterministic Excerpt.
#[test]
fn marks_above_the_item_limit_keep_the_newest_marks_that_fit() {
    let long = "长".repeat(4_000);
    let marks: Vec<MemoryDeleteMark> = (0..40)
        .map(|index| MemoryDeleteMark {
            entry_type: MemoryEntryType::Other,
            title: format!("第{index}条"),
            description: long.clone(),
        })
        .collect();
    let (text, mode) = memory_delete_marks_text(&marks);
    let lines: Vec<&str> = text.lines().collect();
    let listed = lines.len() - 1;
    let longer = format!(
        "{}\n{}",
        lines[..listed].join("\n"),
        memory_delete_marks_text(&marks[listed..=listed]).0
    );

    assert_eq!(mode, ProjectionMode::DeterministicExcerpt);
    assert!(count_context_item_tokens(&text) <= CONTEXT_ITEM_TOKEN_LIMIT);
    assert!(count_context_item_tokens(&longer) > CONTEXT_ITEM_TOKEN_LIMIT);
    assert!(
        lines[0].contains("第0条") && lines[listed - 1].contains(&format!("第{}条", listed - 1))
    );
    assert_eq!(
        lines[listed],
        format!(r#"{{"not_listed":{}}}"#, marks.len() - listed)
    );
}
