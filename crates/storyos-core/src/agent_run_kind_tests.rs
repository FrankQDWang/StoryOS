use super::{AgentRunKind, MEMORY_CAPTURE_MAX_RUN_STEPS, yields_to_author};
use crate::{
    AGENT_PLAN_REGISTRATION, BudgetBounds, HOST_FAKE_TOOL_LOOP_REGISTRATION, MAX_RUN_STEPS,
    MemoryCaptureAssembly, ModelCapability, ProjectionMode, RunStepLimit, ToolChoice,
    ToolOperation, assemble_memory_capture_context,
};

#[test]
fn each_kind_has_its_persisted_name_and_the_worker_dispatches_each_kind() {
    assert_eq!(
        (
            AgentRunKind::ALL.map(AgentRunKind::name),
            AgentRunKind::DISPATCHED.map(AgentRunKind::name),
            AgentRunKind::ALL.map(|kind| AgentRunKind::parse(kind.name())),
        ),
        (
            ["author", "memory_capture", "memory_consolidation", "intent_pass"],
            ["author", "memory_capture", "memory_consolidation", "intent_pass"],
            AgentRunKind::ALL.map(Some)
        )
    );
}

/// The gate of the compaction: only a Run of the author compacts, and only with a Registration
/// whose mapping compacts.
#[test]
fn only_a_run_of_the_author_compacts_with_a_registration_that_compacts() {
    // The requirement of a mapping that streams and continues, and does not compact.
    const STREAMED_CONTINUATION: &[ModelCapability] = &[
        ModelCapability::Transport,
        ModelCapability::NativeText,
        ModelCapability::Continuation,
        ModelCapability::Streaming,
    ];
    let compacts = |revision| AgentRunKind::ALL.map(|kind| kind.compacts_before_decision(revision));
    assert_eq!(
        [
            compacts(HOST_FAKE_TOOL_LOOP_REGISTRATION.revision),
            compacts(AGENT_PLAN_REGISTRATION.revision),
            compacts("an unknown revision"),
            // A known Registration whose requirement has no HostCompaction.
            AgentRunKind::ALL.map(|kind| kind.compacts_on(Some(STREAMED_CONTINUATION))),
        ],
        [
            [true, false, false, false],
            [true, false, false, false],
            [false; 4],
            [false; 4]
        ]
    );
}

/// The branch of an expired Run and the branches of the create request and of the next RunStep:
/// a background Run rebuilds no prior response, reads no Chapter work and no steering input, and
/// requires a call in each request.
#[test]
fn a_background_run_has_no_chapter_work_and_requires_a_call_in_each_request() {
    assert_eq!(
        AgentRunKind::ALL.map(|kind| (
            kind.rebuilds_expired_reference(),
            kind.reads_chapter_work(),
            kind.reads_steering(),
            kind.fixed_tool_choice(),
        )),
        [
            (true, true, true, None),
            (false, false, false, Some(ToolChoice::Required)),
            (false, false, false, Some(ToolChoice::Required)),
            (false, false, false, Some(ToolChoice::Required)),
        ]
    );
}

/// A background Run has its own RunStep limit, and in its last RunStep the Host executes only its
/// output call. A consolidation pass has one RunStep, and an intent pass has two. A Run of the
/// author keeps 24 RunSteps and executes no call of its last RunStep.
#[test]
fn each_kind_has_its_run_step_limit_and_its_last_run_step_rule() {
    let last = |kind: AgentRunKind| kind.max_run_steps() - 1;
    let calls = [
        ToolOperation::RecordMemoryObservations,
        ToolOperation::ApplyMemoryOperations,
        ToolOperation::ReadMemoryEntry,
        ToolOperation::RecordCharacterIntent,
    ];
    assert_eq!(
        AgentRunKind::ALL.map(|kind| (
            kind.max_run_steps(),
            [0, last(kind)].map(|position| kind.run_step_limit(position)),
            calls.map(|operation| kind.executes_call(RunStepLimit::Last, operation)),
            calls.map(|operation| kind.executes_call(RunStepLimit::Open, operation)),
            kind.keeps_calls(RunStepLimit::Last),
        )),
        [
            (
                MAX_RUN_STEPS,
                [RunStepLimit::Open, RunStepLimit::Last],
                [false; 4],
                [true; 4],
                false
            ),
            (
                MEMORY_CAPTURE_MAX_RUN_STEPS,
                [RunStepLimit::Open, RunStepLimit::Last],
                [true, false, false, false],
                [true; 4],
                true
            ),
            (
                1,
                [RunStepLimit::Last, RunStepLimit::Last],
                [false, true, false, false],
                [true; 4],
                true
            ),
            (
                2,
                [RunStepLimit::Open, RunStepLimit::Last],
                [false, false, false, true],
                [true; 4],
                true
            ),
        ]
    );
}

/// Only background work gives its claim back, only between two RunSteps, and only when a Run of
/// the author is queued.
#[test]
fn a_background_run_gives_way_to_a_queued_run_of_the_author_between_two_run_steps() {
    assert_eq!(
        (
            AgentRunKind::ALL.map(AgentRunKind::gives_way_to_author),
            [(0, true), (1, true), (1, false)]
                .map(|(position, queued)| yields_to_author(position, queued)),
        ),
        ([false, true, true, true], [false, true, false])
    );
}

/// A capture does not compact, so the Host checks the full input of each of its requests against
/// the window after the output bound (ADR 0056). The compaction phase checks a Run of the author.
#[test]
fn the_full_request_of_a_background_run_is_refused_at_the_window() {
    let mut record = assemble_memory_capture_context(&MemoryCaptureAssembly {
        operation_requirement_id: "requirement".to_owned(),
        input_snapshot_id: "snapshot".to_owned(),
        run_id: "run".to_owned(),
        owner_user_id: "owner".to_owned(),
        project_id: "project".to_owned(),
        input: "林舟上了船。".to_owned(),
        projection_mode: ProjectionMode::ExactRequired,
        destination_identity: "destination".to_owned(),
    });
    let bounds = BudgetBounds::AgentPlanRequestV2.as_str();
    let mut too_long = |tokens| {
        record.selected[0].token_count = tokens;
        AgentRunKind::ALL.map(|kind| {
            [
                AGENT_PLAN_REGISTRATION.revision,
                HOST_FAKE_TOOL_LOOP_REGISTRATION.revision,
            ]
            .map(|revision| kind.full_request_too_long(revision, bounds, &record))
        })
    };
    assert_eq!(
        [too_long(1_015_808), too_long(1_015_807)],
        [
            [[false, false], [true, false], [true, false], [true, false]],
            [[false, false]; 4]
        ]
    );
}
