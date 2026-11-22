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

/// A pass with the outcome `outcome` that read `read`.
fn pass(read: &super::IntentPassRead, outcome: &str) -> super::IntentPassRecord {
    super::IntentPassRecord {
        read: read.clone(),
        outcome: Some(outcome.to_owned()),
    }
}

/// The selection takes the current character entry of the background work with the lowest number
/// that needs a pass. An entry of the author, a deleted entry, an entry of another type, and a
/// character with only the entry of the author get no pass. A new version of the entry of the
/// author needs a pass again, also when an earlier recorded pass read the same pair. A pair that
/// a failed pass read gets no second pass, also after a failed pass of another pair, while no
/// later result is recorded. After a recorded pass of another pair, the pair needs a pass again,
/// because the stored result is for the other pair. A pair that only a not-kept pass or a running
/// pass read needs a pass.
#[test]
fn the_next_pass_is_for_the_lowest_entry_that_needs_a_pass() {
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
    let pick = |passes: &[super::IntentPassRecord]| {
        let needed = super::select_intent_pass_entry(&entries, passes)?;
        let author = needed.author.map_or("", |author| &author.memory_entry_id);
        Some(format!("{}{author}", needed.entry.memory_entry_id))
    };
    let (c_read, b_read) = (read("c", "2", Some(("a", "1"))), read("b", "1", None));
    let (c_other, b_done) = (read("c", "2", None), pass(&b_read, "recorded"));
    let running = super::IntentPassRecord {
        read: c_read.clone(),
        outcome: None,
    };
    let picks = [
        pick(&[]),
        pick(&[pass(&c_read, "recorded")]),
        pick(&[pass(&c_read, "recorded"), b_done.clone()]),
        pick(&[
            pass(&read("c", "2", Some(("a", "0"))), "recorded"),
            b_done.clone(),
        ]),
        pick(&[
            pass(&c_read, "recorded"),
            pass(&c_other, "recorded"),
            b_done.clone(),
        ]),
        pick(&[pass(&c_read, "failed"), b_done.clone()]),
        pick(&[
            pass(&c_read, "failed"),
            pass(&c_other, "failed"),
            b_done.clone(),
        ]),
        pick(&[
            pass(&c_read, "failed"),
            pass(&c_other, "recorded"),
            b_done.clone(),
        ]),
        pick(&[
            pass(&c_other, "recorded"),
            pass(&c_read, "failed"),
            b_done.clone(),
        ]),
        pick(&[pass(&c_read, "not_kept"), b_done.clone()]),
        pick(&[running, b_done]),
    ];
    assert_eq!(
        picks.each_ref().map(Option::as_deref),
        [
            Some("ca"),
            Some("b"),
            None,
            Some("ca"),
            Some("ca"),
            None,
            None,
            Some("ca"),
            None,
            Some("ca"),
            Some("ca")
        ]
    );
    // #1790 item 5 with no part of the author now: (a) a failed pass on (v1, none), then a result
    // on (v1, a1), then a delete of the part of the author -> a pass. (b) A failed pass on
    // (v1, none), then a pass on (v1, a1) that is not kept, because the part of the author is
    // deleted during it -> no pass: no result was recorded after the failed pass.
    let alone = ["c 3 current character - 阿青 1"].map(entry);
    let (none, a1) = (read("c", "1", None), read("c", "1", Some(("a", "1"))));
    let decide = |later: &str| {
        let passes = [pass(&none, "failed"), pass(&a1, later)];
        super::select_intent_pass_entry(&alone, &passes).map(|needed| needed.entry.title.clone())
    };
    assert_eq!(
        [decide("recorded"), decide("not_kept")],
        [Some("阿青".to_owned()), None]
    );
}

/// A due mark is due now, or one minute after the last start when that is later. A write can
/// change a Character File when the version or the entry before it is a character entry.
#[test]
fn a_due_mark_obeys_the_rate_and_only_a_character_write_needs_one() {
    use std::time::{Duration, SystemTime};
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
    let started = |seconds| Some(now - Duration::from_secs(seconds));
    assert_eq!(
        [None, started(61), started(60), started(10)].map(|last| super::intent_pass_due(now, last)),
        [now, now, now, now + Duration::from_secs(50)]
    );
    let memory = |entry_type| crate::CurrentMemoryEntry {
        memory_entry_id: "a".to_owned(),
        number: 1,
        version: 1,
        entry_type,
        title: "林舟".to_owned(),
        description: String::new(),
        text: String::new(),
        flagged: false,
        author_part: false,
    };
    let write = |id: Option<&str>, entry_type| crate::MemoryVersionWrite {
        action: crate::MemoryWriteAction::Change,
        memory_entry_id: id.map(str::to_owned),
        number: 1,
        version: 2,
        state: crate::MemoryEntryState::Current,
        entry_type,
        title: "林舟".to_owned(),
        description: String::new(),
        text: String::new(),
        flag: None,
    };
    use crate::MemoryEntryType::{Character, Other};
    let can = |version: crate::MemoryVersionWrite, before| {
        super::write_can_change_character(&version, &[memory(before)])
    };
    assert_eq!(
        [
            can(write(Some("a"), Character), Other),
            can(write(Some("a"), Other), Character),
            can(write(Some("a"), Other), Other),
            can(write(None, Other), Character),
        ],
        [true, true, false, false]
    );
}

/// The result of a valid call is a version only while the versions that the pass read are the
/// current versions of the character and no consolidation pass is in progress. A change of either
/// part, a delete, a new entry of the author, a delete of the entry of the author, and a running
/// consolidation pass give `NotKept`.
#[test]
fn a_result_is_kept_only_for_the_versions_that_the_pass_read() {
    let entries = |words: &[&str]| words.iter().copied().map(entry).collect::<Vec<_>>();
    let author = "a 1 current character author 阿青 1";
    let read = read("c", "2", Some(("a", "1")));
    let outcomes = [
        entries(&[author, "c 3 current character - 阿青 2"]),
        entries(&[author, "c 3 current character - 阿青 3"]),
        entries(&[author, "c 3 deleted character - 阿青 3"]),
        entries(&[
            "a 1 current character author 阿青 2",
            "c 3 current character - 阿青 2",
        ]),
        entries(&[
            "a 1 deleted character author 阿青 2",
            "c 3 current character - 阿青 2",
        ]),
        entries(&["c 3 current character author 阿青 3"]),
    ]
    .map(|entries| super::settle_intent_result(&read, &entries, false));
    let recorded = (
        super::IntentPassOutcome::Recorded,
        crate::host_tool_result("The Host recorded the intent, and the pass is complete."),
    );
    let text =
        "The Host did not keep the intent: the character file changed after the pass read it.";
    let not_kept = || {
        (
            super::IntentPassOutcome::NotKept,
            crate::host_tool_result(text),
        )
    };
    assert_eq!(
        outcomes,
        [
            recorded,
            not_kept(),
            not_kept(),
            not_kept(),
            not_kept(),
            not_kept()
        ]
    );
    assert_eq!(not_kept().0.label(), "not_kept");
    let current = entries(&[author, "c 3 current character - 阿青 2"]);
    let text =
        "The Host did not keep the intent: a consolidation pass of the Memory is in progress.";
    assert_eq!(
        super::settle_intent_result(&read, &current, true),
        (
            super::IntentPassOutcome::NotKept,
            crate::host_tool_result(text)
        )
    );
}

/// Each reason of a claim that starts no pass, in the order of the rule, with its label. The
/// rate permits the next start one minute after the last start.
#[test]
fn each_claim_that_starts_no_pass_has_one_reason() {
    use std::time::{Duration, SystemTime};
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
    let ready = super::IntentPassFacts {
        entry_needs_pass: true,
        pass_in_progress: false,
        observation_waits: false,
        consolidation_in_progress: false,
        now,
        last_pass_start: Some(now - Duration::from_secs(60)),
    };
    let decide = |change: fn(&mut super::IntentPassFacts)| {
        let mut facts = ready;
        change(&mut facts);
        super::decide_intent_pass(&facts).map_err(|skip| (skip, skip.label()))
    };
    let due_at = now + Duration::from_secs(59);
    let skip = super::IntentPassSkip::RateLimited { due_at };
    assert_eq!(
        [
            decide(|facts| facts.entry_needs_pass = false),
            decide(|facts| (facts.pass_in_progress, facts.observation_waits) = (true, true)),
            decide(
                |facts| (facts.observation_waits, facts.consolidation_in_progress) = (true, true)
            ),
            decide(|facts| facts.consolidation_in_progress = true),
            decide(|facts| facts.last_pass_start = Some(facts.now - Duration::from_secs(1))),
            decide(|_| ()),
            decide(|facts| facts.last_pass_start = None),
        ],
        [
            Err((
                super::IntentPassSkip::NoEntryNeedsPass,
                "no_entry_needs_pass"
            )),
            Err((super::IntentPassSkip::PassInProgress, "pass_in_progress")),
            Err((super::IntentPassSkip::ObservationWaits, "observation_waits")),
            Err((
                super::IntentPassSkip::ConsolidationInProgress,
                "consolidation_in_progress"
            )),
            Err((skip, "rate_limited")),
            Ok(()),
            Ok(()),
        ]
    );
    assert_eq!(super::IntentPassSkip::StartRefused.label(), "start_refused");
}
