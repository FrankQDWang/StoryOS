use std::time::{Duration, SystemTime};

use super::*;
use crate::assemble_context::CONTEXT_ITEM_TOKEN_LIMIT;

/// The due time is the settlement time and the quiet time of 30 minutes.
#[test]
fn the_due_time_is_the_settlement_time_and_30_minutes() {
    let settled = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
    assert_eq!(
        (conversation_capture_due(settled), CONVERSATION_QUIET_TIME),
        (
            SystemTime::UNIX_EPOCH + Duration::from_secs(2_800),
            Duration::from_secs(1_800)
        )
    );
}

/// Only a settlement that moves the last read Run, with a later completed Run, makes the row due,
/// at the settlement time.
#[test]
fn only_a_settlement_that_moves_the_read_runs_with_a_later_run_makes_the_row_due() {
    use crate::CaptureOutcome::{Failed, NoObservation, ObservationsRecorded, StepLimit};
    let settled = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
    let due = |outcome| {
        [true, false].map(|later_run| capture_settlement_due(outcome, later_run, settled))
    };
    assert_eq!(
        [ObservationsRecorded, NoObservation, Failed, StepLimit].map(due),
        [
            [Some(settled), None],
            [Some(settled), None],
            [None, None],
            [None, None]
        ]
    );
}

/// The decision table of S7-D3e, in its order: the first fact that refuses gives the skip.
#[test]
fn the_decision_follows_the_table_in_its_order() {
    let start = ConversationCaptureFacts {
        author_run_unsettled: false,
        contribution_permitted: true,
        unread_runs: true,
        capture_in_progress: false,
    };
    let with = |change: fn(&mut ConversationCaptureFacts)| {
        let mut facts = start;
        change(&mut facts);
        decide_conversation_capture(&facts)
    };
    use ConversationCaptureSkip::*;
    assert_eq!(
        [
            with(|facts| facts.author_run_unsettled = true),
            with(|facts| {
                facts.author_run_unsettled = true;
                facts.contribution_permitted = false;
            }),
            with(|facts| facts.contribution_permitted = false),
            with(|facts| {
                facts.contribution_permitted = false;
                facts.unread_runs = false;
            }),
            with(|facts| facts.unread_runs = false),
            with(|facts| {
                facts.unread_runs = false;
                facts.capture_in_progress = true;
            }),
            with(|facts| facts.capture_in_progress = true),
            with(|_| {}),
        ],
        [
            Err(AuthorRunUnsettled),
            Err(AuthorRunUnsettled),
            Err(ContributionNotPermitted),
            Err(ContributionNotPermitted),
            Err(NothingUnread),
            Err(NothingUnread),
            Err(CaptureInProgress),
            Ok(()),
        ]
    );
    assert_eq!(
        [
            AuthorRunUnsettled,
            ContributionNotPermitted,
            NothingUnread,
            CaptureInProgress,
            StartRefused
        ]
        .map(ConversationCaptureSkip::label),
        [
            "author_run_unsettled",
            "contribution_not_permitted",
            "nothing_unread",
            "capture_in_progress",
            "start_refused"
        ]
    );
}

fn run(run_id: &str, messages: &[(ConversationRole, &str)]) -> ConversationRunMessages {
    ConversationRunMessages {
        run_id: run_id.to_owned(),
        messages: (messages.iter())
            .map(|(role, text)| (*role, (*text).to_owned()))
            .collect(),
    }
}

/// The input has each message, the oldest Run first, with its role and its Run.
#[test]
fn the_input_has_each_message_with_its_role_and_its_run() {
    let runs = [
        run(
            "run-1",
            &[
                (ConversationRole::Author, "林舟是谁？"),
                (ConversationRole::Author, "他是船夫。"),
                (ConversationRole::Assistant, "林舟是渡口的船夫。"),
            ],
        ),
        run("run-2", &[(ConversationRole::Author, "继续写。")]),
    ];
    assert_eq!(
        (
            conversation_capture_input(&runs, CONTEXT_ITEM_TOKEN_LIMIT),
            conversation_capture_input(&[], CONTEXT_ITEM_TOKEN_LIMIT)
        ),
        (
            Some(ConversationCaptureInput {
                text: "{\"role\":\"author\",\"run_id\":\"run-1\"}\n林舟是谁？\n\n\
                       {\"role\":\"author\",\"run_id\":\"run-1\"}\n他是船夫。\n\n\
                       {\"role\":\"assistant\",\"run_id\":\"run-1\"}\n林舟是渡口的船夫。\n\n\
                       {\"role\":\"author\",\"run_id\":\"run-2\"}\n继续写。"
                    .to_owned(),
                first_run_id: "run-1".to_owned(),
                last_run_id: "run-2".to_owned(),
                projection_mode: ProjectionMode::ExactRequired,
            }),
            None
        )
    );
}

/// An input of exactly the limit has each Run. With one token less, the input has the oldest Runs
/// that fit, and its last Run is the last of them.
#[test]
fn the_input_has_the_oldest_runs_that_fit_the_limit() {
    let runs = [
        run("run-1", &[(ConversationRole::Author, "林舟是谁？")]),
        run(
            "run-2",
            &[(ConversationRole::Assistant, "林舟是渡口的船夫。")],
        ),
        run("run-3", &[(ConversationRole::Author, "继续写。")]),
    ];
    let whole = conversation_capture_input(&runs, u64::MAX).unwrap();
    let two = conversation_capture_input(&runs[..2], u64::MAX).unwrap();
    let limit = count_context_item_tokens(&whole.text);
    let bounded = |limit| {
        let input = conversation_capture_input(&runs, limit).unwrap();
        (input.text, input.last_run_id, input.projection_mode)
    };
    assert_eq!(
        (bounded(limit), bounded(limit - 1)),
        (
            (
                whole.text,
                "run-3".to_owned(),
                ProjectionMode::ExactRequired
            ),
            (two.text, "run-2".to_owned(), ProjectionMode::ExactRequired)
        )
    );
}

/// When the oldest Run alone is above the limit, the input has only that Run, and its final
/// answer is the longest prefix that fits, with the mark of the cut.
#[test]
fn one_run_above_the_limit_gives_an_excerpt_of_its_final_answer() {
    let answer = "一二三四五六七八九十".repeat(30);
    let runs = [
        run(
            "run-1",
            &[
                (ConversationRole::Author, "继续写。"),
                (ConversationRole::Assistant, answer.as_str()),
            ],
        ),
        run("run-2", &[(ConversationRole::Author, "再写一章。")]),
    ];
    let excerpt = |kept: &str, count: usize| {
        format!(
            "{{\"role\":\"author\",\"run_id\":\"run-1\"}}\n继续写。\n\n\
             {{\"role\":\"assistant\",\"run_id\":\"run-1\"}}\n{kept} [The Host cut this message \
             here. It keeps {count} of its 300 characters. Later messages of the Run that the Host \
             omits: 0.]"
        )
    };
    let limit = count_context_item_tokens(&excerpt("一二三", 3));
    let bounded = |limit| {
        let input = conversation_capture_input(&runs, limit).unwrap();
        (
            input.text,
            input.first_run_id,
            input.last_run_id,
            input.projection_mode,
        )
    };
    let expected = |kept: &str, count| {
        (
            excerpt(kept, count),
            "run-1".to_owned(),
            "run-1".to_owned(),
            ProjectionMode::DeterministicExcerpt,
        )
    };
    assert_eq!(
        (bounded(limit), bounded(limit - 1)),
        (expected("一二三", 3), expected("一二", 2))
    );
}

/// When the author messages of the one Run are above the limit also with no answer text, the
/// excerpt keeps the whole messages that fit, cuts the next message, and omits the answer. The
/// input fits the limit.
#[test]
fn author_messages_above_the_limit_give_an_excerpt_that_fits() {
    let (message, steering, answer) = ("甲".repeat(200), "乙".repeat(200), "丙".repeat(200));
    let runs = [run(
        "run-1",
        &[
            (ConversationRole::Author, message.as_str()),
            (ConversationRole::Author, steering.as_str()),
            (ConversationRole::Assistant, answer.as_str()),
        ],
    )];
    let excerpt = |kept: usize| {
        format!(
            "{{\"role\":\"author\",\"run_id\":\"run-1\"}}\n{message}\n\n\
             {{\"role\":\"author\",\"run_id\":\"run-1\"}}\n{} [The Host cut this message here. \
             It keeps {kept} of its 200 characters. Later messages of the Run that the Host omits: \
             1.]",
            "乙".repeat(kept)
        )
    };
    let limit = count_context_item_tokens(&excerpt(5));
    let bounded = |limit| {
        let input = conversation_capture_input(&runs, limit).unwrap();
        (input.text, input.projection_mode)
    };
    assert_eq!(
        (bounded(limit), bounded(limit - 1)),
        (
            (excerpt(5), ProjectionMode::DeterministicExcerpt),
            (excerpt(4), ProjectionMode::DeterministicExcerpt)
        )
    );
}

/// The rule of the consolidation row: due now with no earlier pass, due at the start of the last
/// pass and 30 minutes when that is later, and no due time when no observation waits. The
/// settlement of a capture and of a pass with each outcome use this one rule.
#[test]
fn a_waiting_observation_makes_the_project_due_after_the_interval_of_the_last_pass() {
    let at = |seconds| SystemTime::UNIX_EPOCH + Duration::from_secs(seconds);
    let now = at(10_000);
    assert_eq!(
        (
            CONSOLIDATION_INTERVAL,
            [None, Some(at(9_000)), Some(at(8_200)), Some(at(1_000))]
                .map(|last_pass_start| consolidation_due(true, now, last_pass_start)),
            [None, Some(at(9_000))].map(|last_pass_start| consolidation_due(
                false,
                now,
                last_pass_start
            )),
        ),
        (
            Duration::from_secs(1_800),
            [Some(now), Some(at(10_800)), Some(now), Some(now)],
            [None, None],
        )
    );
}

/// The decision of a claimed consolidation row, in its order: nothing for a pass, then a pass in
/// progress, then an intent pass in progress. A waiting conflict is something for a pass only when
/// the Project has a current entry.
#[test]
fn the_consolidation_decision_follows_its_order() {
    let decide = |[
        observation_waits,
        conflict_waits,
        entry_current,
        pass_in_progress,
        intent_pass_in_progress,
    ]: [bool; 5]| {
        decide_consolidation(&ConsolidationFacts {
            observation_waits,
            conflict_waits,
            entry_current,
            pass_in_progress,
            intent_pass_in_progress,
        })
    };
    use ConsolidationSkip::*;
    assert_eq!(
        [
            [false, false, true, true, true],
            [false, false, true, false, false],
            [false, true, false, false, false],
            [false, true, true, false, false],
            [true, false, false, true, true],
            [true, false, false, false, true],
            [true, false, false, false, false],
        ]
        .map(decide),
        [
            Err(NothingWaits),
            Err(NothingWaits),
            Err(NothingWaits),
            Ok(()),
            Err(PassInProgress),
            Err(IntentPassInProgress),
            Ok(())
        ]
    );
    assert_eq!(
        [
            NothingWaits,
            PassInProgress,
            IntentPassInProgress,
            StartRefused
        ]
        .map(ConsolidationSkip::label),
        [
            "no_observation_waits",
            "pass_in_progress",
            "intent_pass_in_progress",
            "start_refused"
        ]
    );
}
