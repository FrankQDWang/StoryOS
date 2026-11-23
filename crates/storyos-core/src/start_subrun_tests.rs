use serde_json::json;

use super::{START_SUBRUN_TOOL_SPEC, SubrunEnd, SubrunRefusal, subrun_refusal, subrun_result};
use crate::{AgentRunStatus, IntakeRefusal, ToolIntake};

/// ADR 0103 section 3A, item 2 of #436: a call is accepted when the title has 1 through 40
/// characters on one line, the brief has 1 through 4000 characters, neither is only white space,
/// and the Run has fewer than 4 Subrun Requests. A refused call gets the sentence of the first
/// rule that failed.
#[test]
fn a_subrun_call_is_accepted_only_in_the_bounds_and_a_refusal_gives_the_first_rule() {
    let title = "雪".repeat(40);
    let brief = "雪".repeat(4000);
    let cases = [
        (title.as_str(), brief.as_str(), 3, None),
        ("读", "读", 0, None),
        ("", "读", 0, Some(SubrunRefusal::Title)),
        ("  ", "读", 0, Some(SubrunRefusal::Title)),
        (&*"雪".repeat(41), "读", 0, Some(SubrunRefusal::Title)),
        ("读\n第一章", "读", 0, Some(SubrunRefusal::Title)),
        ("读\u{2028}", "读", 0, Some(SubrunRefusal::Title)),
        ("", "", 4, Some(SubrunRefusal::Title)),
        ("读", "", 0, Some(SubrunRefusal::Brief)),
        ("读", " \n ", 0, Some(SubrunRefusal::Brief)),
        ("读", &*"雪".repeat(4001), 4, Some(SubrunRefusal::Brief)),
        ("读", "第一行\n第二行", 4, Some(SubrunRefusal::RequestCount)),
    ];
    let results: Vec<_> = (cases.iter())
        .map(|(title, brief, requests, _)| subrun_refusal(title, brief, *requests))
        .collect();
    let expected: Vec<_> = cases.iter().map(|case| case.3).collect();
    let sentences: Vec<_> = [
        SubrunRefusal::Title,
        SubrunRefusal::Brief,
        SubrunRefusal::RequestCount,
    ]
    .map(|refusal| (refusal.label(), refusal.result_text()))
    .into();

    assert_eq!(
        (results, sentences),
        (
            expected,
            vec![
                (
                    "subrun_title",
                    "The title must have 1 through 40 characters on one line."
                ),
                (
                    "subrun_brief",
                    "The brief must have 1 through 4000 characters."
                ),
                (
                    "subrun_request_count",
                    "One author request can start at most 4 Subruns."
                ),
            ]
        )
    );
}

/// The intake takes the two strings, and it refuses another field, a missing field, and a text
/// that the ToolCall record cannot hold.
#[test]
fn the_intake_takes_the_title_and_the_brief() {
    let intake = |arguments| START_SUBRUN_TOOL_SPEC.intake(&arguments);
    assert_eq!(
        (
            intake(json!({"title": "读", "brief": "读第一章"})),
            intake(json!({"title": "读"})),
            intake(json!({"title": "读", "brief": "读", "chapter": 1})),
            intake(json!({"title": "读\u{0}", "brief": "读"})),
        ),
        (
            Ok(ToolIntake::StartSubrun {
                title: "读".to_owned(),
                brief: "读第一章".to_owned()
            }),
            Err(IntakeRefusal::Malformed),
            Err(IntakeRefusal::Malformed),
            Err(IntakeRefusal::UnstorableText),
        )
    );
}

/// Item 8 of #436: a completed Subrun gives its final answer text. A Subrun whose request was too
/// long gives the sentence of that cause. Each other end with no final answer gives the sentence
/// of no result.
#[test]
fn the_result_is_the_final_answer_or_the_sentence_of_the_end() {
    let end = |status, final_answer, too_long| {
        subrun_result(SubrunEnd {
            status,
            final_answer,
            too_long,
        })
        .text
    };
    let no_result = "The Subrun ended with no result.";
    assert_eq!(
        [
            end(AgentRunStatus::Completed, Some("第一章有灯。"), false),
            end(AgentRunStatus::Completed, Some(""), false),
            end(AgentRunStatus::Completed, None, false),
            end(AgentRunStatus::Refused, None, true),
            end(AgentRunStatus::Refused, None, false),
            end(AgentRunStatus::Waiting, None, false),
            end(AgentRunStatus::Paused, None, false),
        ],
        [
            "第一章有灯。",
            no_result,
            no_result,
            "The Subrun ended with no result, because its reading was too long for one Subrun. \
             Give a smaller task.",
            no_result,
            no_result,
            no_result,
        ]
    );
}
