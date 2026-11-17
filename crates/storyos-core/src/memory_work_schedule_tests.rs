use std::time::{Duration, SystemTime};

use super::*;

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

/// The input has each message, the oldest Run first, with its role and its Run.
#[test]
fn the_input_has_each_message_with_its_role_and_its_run() {
    let run = |run_id: &str, messages: &[(ConversationRole, &str)]| ConversationRunMessages {
        run_id: run_id.to_owned(),
        messages: (messages.iter())
            .map(|(role, text)| (*role, (*text).to_owned()))
            .collect(),
    };
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
            conversation_capture_input(&runs),
            conversation_capture_input(&[])
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
            }),
            None
        )
    );
}
