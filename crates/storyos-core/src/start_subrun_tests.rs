use super::{SubrunEnd, SubrunRefusal, subrun_refusal, subrun_result};
use crate::AgentRunStatus;

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

/// Item 8 of #436: a queued or claimed Subrun gives no result, so the call continues it. A
/// completed Subrun gives its final answer text. A Subrun whose request was too long gives the
/// sentence of that cause. Each other end with no final answer, and an answer that the ToolCall
/// record cannot hold, give the sentence of no result.
#[test]
fn the_result_is_the_final_answer_or_the_sentence_of_the_end() {
    let end = |status, final_answer, too_long| {
        subrun_result(SubrunEnd {
            status,
            final_answer,
            too_long,
        })
        .map(|result| result.text)
    };
    let no_result = Some("The Subrun ended with no result.".to_owned());
    let long = "雪".repeat(crate::TOOL_RESULT_TEXT_MAX_BYTES / 3 + 1);
    assert_eq!(
        [
            end(AgentRunStatus::Queued, None, false),
            end(AgentRunStatus::Claimed, None, false),
            end(AgentRunStatus::Completed, Some("第一章有灯。"), false),
            end(AgentRunStatus::Completed, Some(&long), false),
            end(AgentRunStatus::Completed, Some(""), false),
            end(AgentRunStatus::Completed, None, false),
            end(AgentRunStatus::Refused, None, true),
            end(AgentRunStatus::Refused, None, false),
            end(AgentRunStatus::Waiting, None, false),
            end(AgentRunStatus::Paused, None, false),
        ],
        [
            None,
            None,
            Some("第一章有灯。".to_owned()),
            no_result.clone(),
            no_result.clone(),
            no_result.clone(),
            Some(
                "The Subrun ended with no result, because its reading was too long for one \
                 Subrun. Give a smaller task."
                    .to_owned()
            ),
            no_result.clone(),
            no_result.clone(),
            no_result,
        ]
    );
}
