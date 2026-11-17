use super::AgentRunKind;
use crate::{AGENT_PLAN_STREAMING_REGISTRATION, HOST_FAKE_REGISTRATION, RunStepLimit, ToolChoice};

#[test]
fn each_kind_has_its_persisted_name_and_a_consolidation_is_not_dispatched() {
    assert_eq!(
        (
            AgentRunKind::ALL.map(AgentRunKind::name),
            AgentRunKind::DISPATCHED.map(AgentRunKind::name),
            AgentRunKind::ALL.map(|kind| AgentRunKind::parse(kind.name())),
        ),
        (
            ["author", "memory_capture", "memory_consolidation"],
            ["author", "memory_capture"],
            AgentRunKind::ALL.map(Some)
        )
    );
}

/// The gate of the compaction: only a Run of the author compacts, and only with a Registration
/// whose mapping compacts.
#[test]
fn only_a_run_of_the_author_compacts_with_a_registration_that_compacts() {
    let compacts = |revision| AgentRunKind::ALL.map(|kind| kind.compacts_before_decision(revision));
    assert_eq!(
        [
            compacts(HOST_FAKE_REGISTRATION.revision),
            compacts(AGENT_PLAN_STREAMING_REGISTRATION.revision),
            compacts("an unknown revision"),
        ],
        [[true, false, false], [false; 3], [false; 3]]
    );
}

/// The branch of an expired Run and the branches of the create request and of the next RunStep:
/// a background Run rebuilds no prior response, reads no Chapter work and no steering input, and
/// requires a call in each request before the last RunStep, which sends `none`.
#[test]
fn a_background_run_has_no_chapter_work_and_requires_a_call_in_each_request() {
    assert_eq!(
        AgentRunKind::ALL.map(|kind| (
            kind.rebuilds_expired_reference(),
            kind.reads_chapter_work(),
            kind.reads_steering(),
            [RunStepLimit::Open, RunStepLimit::Last].map(|limit| kind.fixed_tool_choice(limit)),
        )),
        [
            (true, true, true, [None, None]),
            (
                false,
                false,
                false,
                [Some(ToolChoice::Required), Some(ToolChoice::None)]
            ),
            (
                false,
                false,
                false,
                [Some(ToolChoice::Required), Some(ToolChoice::None)]
            ),
        ]
    );
}
