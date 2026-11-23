use super::{
    EARLIER_TURNS_TOKEN_LIMIT, EarlierMessageKind, EarlierTurn, EarlierTurnMessage,
    EarlierTurnSource, EarlierTurns, assemble_earlier_turns, earlier_turns_of,
    select_earlier_turns,
};
use crate::shared_tests::passage_assembly;
use crate::{ContextSourceClass, assemble_current_passage_context};

fn turn(run_id: &str, messages: &[(EarlierMessageKind, String)]) -> EarlierTurn {
    EarlierTurn {
        messages: (messages.iter().enumerate())
            .map(|(index, (message_kind, text))| EarlierTurnMessage {
                source: EarlierTurnSource {
                    run_id: run_id.to_owned(),
                    message_id: format!("{run_id}-{index}"),
                    message_kind: *message_kind,
                },
                text: text.clone(),
            })
            .collect(),
    }
}

fn author(text: &str) -> (EarlierMessageKind, String) {
    (EarlierMessageKind::Author, text.to_owned())
}

fn answer(text: &str) -> (EarlierMessageKind, String) {
    (EarlierMessageKind::Answer, text.to_owned())
}

/// ADR 0102 section 4: the Host keeps each turn when all of them fit together.
#[test]
fn the_host_keeps_each_earlier_turn_when_all_fit() {
    let turns = vec![
        turn(
            "run-1",
            &[author("Who is Mara?"), answer("Mara keeps the inn.")],
        ),
        turn(
            "run-2",
            &[
                author("Make her older."),
                author("Keep the voice."),
                answer("Done."),
            ],
        ),
    ];

    assert_eq!(
        select_earlier_turns(turns.clone()),
        EarlierTurns {
            turns,
            older_turns_left_out: false,
        }
    );
}

/// ADR 0102 section 4: the Host takes whole turns from the newest. The first turn that does not
/// fit ends the selection, so an older turn that alone would fit is also left out, and the result
/// says so.
#[test]
fn the_first_turn_that_does_not_fit_ends_the_selection_and_the_result_says_so() {
    let newest = turn(
        "run-4",
        &[
            author(&"d".repeat(25_000)),
            author(&"d".repeat(25_000)),
            author(&"d".repeat(25_000)),
            answer(&"e".repeat(25_000)),
        ],
    );
    let turns = vec![
        turn("run-1", &[author("a")]),
        turn("run-2", &[author("b"), answer("c")]),
        turn("run-3", &[author("x")]),
        newest.clone(),
    ];
    let kept = select_earlier_turns(turns);
    let tokens: u64 = (kept.turns.iter())
        .flat_map(|turn| &turn.messages)
        .map(|message| crate::count_context_item_tokens(&message.text))
        .sum();

    assert_eq!(
        (kept, tokens),
        (
            EarlierTurns {
                turns: vec![newest],
                older_turns_left_out: true,
            },
            EARLIER_TURNS_TOKEN_LIMIT
        )
    );
}

/// ADR 0052 and ADR 0102 section 4: a turn with a message above the Context item limit does not
/// fit, so it ends the selection although the total fits.
#[test]
fn a_turn_with_a_message_above_the_item_limit_ends_the_selection() {
    let newest = turn("run-3", &[author("Make it shorter."), answer("Done.")]);
    let turns = vec![
        turn("run-1", &[author("a")]),
        turn(
            "run-2",
            &[
                author("Write the chapter."),
                answer(&"c".repeat(crate::CONTEXT_ITEM_TOKEN_LIMIT as usize + 1)),
            ],
        ),
        newest.clone(),
    ];

    assert_eq!(
        select_earlier_turns(turns),
        EarlierTurns {
            turns: vec![newest],
            older_turns_left_out: true,
        }
    );
}

/// ADR 0102 section 2: a completed Run with no final answer text gives only its messages of the
/// author, and the record of the request gives the same turns back.
#[test]
fn a_turn_with_no_final_answer_has_only_the_messages_of_the_author() {
    let turns = vec![
        turn(
            "run-1",
            &[author("Look at the inn."), author("Only the inn.")],
        ),
        turn(
            "run-2",
            &[author("Who is Mara?"), answer("Mara keeps the inn.")],
        ),
    ];
    let kept = select_earlier_turns(turns.clone());
    let mut record = assemble_current_passage_context(&passage_assembly("Help.", "Passage."));
    assemble_earlier_turns(&mut record, &kept);
    let classes: Vec<_> = (record.selected.iter())
        .filter(|item| super::is_earlier_turn(&record, item))
        .map(|item| (item.source_class, item.content.as_str()))
        .collect();

    assert_eq!(
        (earlier_turns_of(&record), classes),
        (
            Some(EarlierTurns {
                turns,
                older_turns_left_out: false,
            }),
            vec![
                (ContextSourceClass::AuthorInstruction, "Look at the inn."),
                (ContextSourceClass::AuthorInstruction, "Only the inn."),
                (ContextSourceClass::AuthorInstruction, "Who is Mara?"),
                (ContextSourceClass::EarlierAnswer, "Mara keeps the inn."),
            ]
        )
    );
}

/// ADR 0056 and ADR 0102 section 3: a request that continues a prior response does not send the
/// earlier turns, so its expected input does not count them. A full input sends them, so its
/// check counts them.
#[test]
fn the_expected_input_counts_the_earlier_turns_only_in_a_full_input() {
    let plain = assemble_current_passage_context(&passage_assembly("Help.", "Passage."));
    let mut with_turns = plain.clone();
    assemble_earlier_turns(
        &mut with_turns,
        &select_earlier_turns(vec![turn("run-1", &[author("Who?"), answer("Mara.")])]),
    );
    let sent = |record: &crate::CurrentPassageAssemblyRecord| {
        crate::CompactionTrigger::from_facts(&crate::CompactionRequestFacts {
            model_registration_revision: crate::AGENT_PLAN_REGISTRATION.revision,
            budget_bounds: crate::BudgetBounds::AgentPlanRequestV2.as_str(),
            author_message: "Help.",
            decision: Some(record),
            declared_target_texts: &[],
            prior_reported_tokens: Some(1_000),
            prior_decision_id: None,
            prior_input: crate::PriorInput::Replaced,
            decision_position: "0",
            compacted_earlier: false,
        })
        .sent_input_tokens
    };
    // The window after the output bound is 1,048,576 - 32,768 = 1,015,808 tokens. The working
    // text fills it to one token below the limit without the earlier turns and the declared
    // target texts.
    let too_long = |record: &crate::CurrentPassageAssemblyRecord, target_texts: &[String]| {
        let mut full = record.clone();
        let others: u64 = plain.selected.iter().map(|item| item.token_count).sum();
        full.selected[1].token_count += 1_015_808 - others - 1;
        [false, true].map(|prior_response| {
            crate::full_decision_request_too_long(
                prior_response,
                crate::AGENT_PLAN_REGISTRATION.revision,
                crate::BudgetBounds::AgentPlanRequestV2.as_str(),
                &full,
                target_texts,
            )
        })
    };
    let target = ["The target block.".to_owned()];

    // Only a full input with the earlier turns or a declared target text is too long. The check
    // of a request that continues a prior response is the compaction trigger, which does not
    // count the earlier turns.
    assert_eq!(
        (
            sent(&with_turns),
            [
                too_long(&plain, &[]),
                too_long(&with_turns, &[]),
                too_long(&plain, &target),
            ]
        ),
        (sent(&plain), [[false, false], [true, false], [true, false]])
    );
}

/// ADR 0102 section 3: the later request of a compaction has the summary in the place of the
/// earlier turns.
#[test]
fn the_later_request_of_a_compaction_has_no_earlier_turn() {
    let plain = assemble_current_passage_context(&passage_assembly("Help.", "Passage."));
    let mut with_turns = plain.clone();
    assemble_earlier_turns(
        &mut with_turns,
        &select_earlier_turns(vec![turn("run-1", &[author("Who?"), answer("Mara.")])]),
    );

    assert_eq!(
        crate::later_request_record(&with_turns, "compaction-1", "Summary.", &[]),
        crate::later_request_record(&plain, "compaction-1", "Summary.", &[])
    );
}
