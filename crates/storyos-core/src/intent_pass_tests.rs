use super::admit_intent_pass;
use crate::CaptureRefusal;

/// The start uses the admission rule of a capture with the Purpose `intent_pass`, which the scope
/// `project_writing_assistance_v6` names first.
#[test]
fn the_start_of_an_intent_pass_needs_the_scope_that_names_its_purpose() {
    let scopes = [
        (false, true, Some("project_writing_assistance_v6")),
        (true, false, Some("project_writing_assistance_v6")),
        (true, true, Some("project_writing_assistance_v5")),
        (true, true, Some("project_writing_assistance_v6")),
    ];
    assert_eq!(
        scopes.map(|(binding, available, scope)| admit_intent_pass(binding, available, scope)),
        [
            Err(CaptureRefusal::NoModelUseBinding),
            Err(CaptureRefusal::AssistanceUnavailable),
            Err(CaptureRefusal::PurposeNotGranted),
            Ok(()),
        ]
    );
}

/// The source class and the content of each selected item of the context of `file`, with the
/// Purpose and the cause of the context.
fn context(file: super::CharacterFile<'_>) -> (Vec<(crate::ContextSourceClass, String)>, String) {
    let record = super::assemble_intent_pass_context(&super::IntentPassAssembly {
        operation_requirement_id: "requirement".to_owned(),
        run_id: "run".to_owned(),
        owner_user_id: "owner".to_owned(),
        project_id: "project".to_owned(),
        destination_identity: "destination".to_owned(),
        file,
    });
    let requirement = &record.operation_requirement;
    let selected = (record.selected.iter())
        .map(|item| (item.source_class, item.content.clone()))
        .collect();
    let kind = format!("{:?} {:?}", requirement.purpose, requirement.cause);
    (selected, kind)
}

/// ADR 0101 section 4: the context of a pass has one item of the class `character_file` and no
/// other item. The item has the name line and each part with text under its heading, and a file
/// with no entry of the author has no part `Told by the author`.
#[test]
fn the_context_of_a_pass_has_only_the_file_item() {
    let file = super::CharacterFile {
        name: "林舟",
        author_text: Some("林舟怕水。"),
        manuscript_text: "林舟是船夫。",
    };
    let class = crate::ContextSourceClass::CharacterFile;
    let kind = "IntentPass HostSchedule".to_owned();
    assert_eq!(
        [
            context(file),
            context(super::CharacterFile {
                author_text: None,
                ..file
            })
        ],
        [
            (
                vec![(
                    class,
                    "{\"name\":\"林舟\"}\n\nTold by the author:\n林舟怕水。\n\n\
                     From the manuscript:\n林舟是船夫。"
                        .to_owned()
                )],
                kind.clone()
            ),
            (
                vec![(
                    class,
                    "{\"name\":\"林舟\"}\n\nFrom the manuscript:\n林舟是船夫。".to_owned()
                )],
                kind
            )
        ]
    );
}

/// Only a refused call gives the second request. A reply with no call, and a settled call, end
/// the pass.
#[test]
fn only_a_refused_call_gives_the_second_request() {
    use crate::ToolCallState::{Refused, Settled};
    let states: [&[crate::ToolCallState]; 4] = [&[], &[Settled], &[Refused], &[Settled, Refused]];
    assert_eq!(
        states.map(|states| super::intent_pass_sends_again(states.iter().copied())),
        [false, false, true, true]
    );
}

/// The Chapter of a pass is the last Chapter in Chapter order with a block that has text. A
/// Chapter with no block, or with only white space, has no text.
#[test]
fn the_chapter_of_a_pass_is_the_last_chapter_with_text() {
    let chapter = |chapters: &[(&'static str, Vec<&str>)]| super::intent_pass_chapter(chapters);
    assert_eq!(
        [
            chapter(&[("a", vec!["林舟。"]), ("b", vec!["  \n"]), ("c", vec![])]),
            chapter(&[("a", vec!["林舟。"]), ("b", vec!["", "阿青。"])]),
            chapter(&[("a", vec![" "])]),
            chapter(&[]),
        ],
        [Some("a"), Some("b"), None, None]
    );
}

/// One entry from the words `id number state type author title version`. The word `author` is
/// `author` for an entry of the author.
fn entry(words: &str) -> super::IntentPassEntryFacts {
    let words = words.split(' ').collect::<Vec<_>>();
    super::IntentPassEntryFacts {
        memory_entry_id: words[0].to_owned(),
        entry_number: words[1].parse().unwrap(),
        state: words[2].to_owned(),
        entry_type: words[3].to_owned(),
        author: words[4] == "author",
        title: words[5].to_owned(),
        version: words[6].to_owned(),
        text: format!("{}。", words[5]),
    }
}

/// The pass that read `entry` at `version`, and the entry of the author `author`.
fn read(entry: &str, version: &str, author: Option<(&str, &str)>) -> super::IntentPassRead {
    super::IntentPassRead {
        entry: (entry.to_owned(), version.to_owned()),
        author: author.map(|(id, version)| (id.to_owned(), version.to_owned())),
    }
}

/// The selection takes the current character entry of the background work with the lowest number
/// that has no pass for its version pair. An entry of the author, a deleted entry, an entry of
/// another type, and a character with only the entry of the author get no pass. A new version of
/// the entry of the author needs a pass again.
#[test]
fn the_next_pass_is_for_the_lowest_entry_with_no_pass_for_its_versions() {
    let entries = [
        "a 1 current character author 阿青 1",
        "b 4 current character - 林舟 1",
        "c 3 current character - 阿青 2",
        "d 2 deleted character - 老周 2",
        "e 5 current place - 渡口 1",
        "f 6 current character author 小满 1",
    ]
    .map(entry);
    // The entry and the author entry of the selection, as one word.
    let pick = |passes: &[super::IntentPassRead]| {
        let needed = super::select_intent_pass_entry(&entries, passes)?;
        let author = needed.author.map_or("", |author| &author.memory_entry_id);
        Some(format!("{}{author}", needed.entry.memory_entry_id))
    };
    let c_read = read("c", "2", Some(("a", "1")));
    let b_read = read("b", "1", None);
    let picks = [
        pick(&[]),
        pick(std::slice::from_ref(&c_read)),
        pick(&[c_read.clone(), b_read.clone()]),
        pick(&[read("c", "2", Some(("a", "0"))), b_read]),
    ];
    assert_eq!(
        picks.each_ref().map(Option::as_deref),
        [Some("ca"), Some("b"), None, Some("ca")]
    );
}

/// The result of a valid call is a version only while the entry is a current character entry of
/// the background work.
#[test]
fn a_result_is_kept_only_for_a_current_character_entry_of_the_background_work() {
    let now = [
        ("current", "character", "chapter"),
        ("deleted", "character", "chapter"),
        ("current", "character", "author_request"),
        ("current", "place", "chapter"),
    ]
    .map(|(state, entry_type, origin)| super::settle_intent_result(state, entry_type, origin));
    let text = "The Host did not record the intent: the entry is no longer a current character \
                entry of the background work.";
    let not_current = || (Err("entry_not_current"), crate::host_tool_result(text));
    let recorded = (
        Ok(super::IntentPassOutcome::Recorded),
        crate::host_tool_result("The Host recorded the intent, and the pass is complete."),
    );
    assert_eq!(now, [recorded, not_current(), not_current(), not_current()]);
}
