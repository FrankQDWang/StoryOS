use super::{
    ACTIVE_COMPACTION_REQUEST_PREFIX, AGENT_PLAN_COMPACTION_PRODUCER,
    COMPACTION_LOSS_SEMANTIC_PRESERVATION_UNKNOWN, COMPACTION_SUMMARY_UNAVAILABLE,
    CompactionInstallFacts, CompactionInstallRefusal, CompactionKnownInput, CompactionRecordState,
    CompactionSource, CompactionStep, HOST_FAKE_COMPACTION_OUTPUT, HOST_FAKE_COMPACTION_PRODUCER,
    StagedCompaction, SummaryObservation, active_context_input_digest, decide_compaction_install,
    next_compaction_step, requests_active_compaction, stage_compaction,
};

fn facts(
    source_restricted: bool,
    exact_required_satisfied: bool,
    staged_input_digest: &str,
    current_input_digest: &str,
) -> CompactionInstallFacts {
    CompactionInstallFacts {
        source_restricted,
        exact_required_satisfied,
        staged_input_digest: staged_input_digest.to_owned(),
        current_input_digest: current_input_digest.to_owned(),
    }
}

#[test]
fn ordinary_assistance_does_not_request_active_compaction() {
    assert!(requests_active_compaction(ACTIVE_COMPACTION_REQUEST_PREFIX));
    assert!(!requests_active_compaction("Help with this passage."));
}

#[test]
fn input_digest_binds_the_exact_message_revision_and_body() {
    assert_eq!(
        active_context_input_digest("Compact active context between calls.", "rev-1", "Passage.",),
        "sha256:e6b1c44b026b3300a9ad38ee204c31fa81ff4e24145cea477caad05c6c2e484f"
    );
}

#[test]
fn install_refuses_a_restricted_changed_or_incomplete_exact_input() {
    let staged = "sha256:staged";
    assert_eq!(
        decide_compaction_install(&facts(
            /*source_restricted*/ false, /*exact_required_satisfied*/ true, staged,
            staged,
        )),
        Ok(())
    );
    assert_eq!(
        decide_compaction_install(&facts(
            /*source_restricted*/ true, /*exact_required_satisfied*/ true, staged,
            staged,
        )),
        Err(CompactionInstallRefusal::RestrictedSource)
    );
    assert_eq!(
        decide_compaction_install(&facts(
            /*source_restricted*/ false,
            /*exact_required_satisfied*/ false,
            staged,
            "sha256:other",
        )),
        Err(CompactionInstallRefusal::ExactRequiredUnsatisfied)
    );
    assert_eq!(
        decide_compaction_install(&facts(
            /*source_restricted*/ false,
            /*exact_required_satisfied*/ true,
            staged,
            "sha256:other",
        )),
        Err(CompactionInstallRefusal::ChangedInput)
    );
}

#[test]
fn a_requested_compaction_requests_one_summary_then_installs_or_refuses_once() {
    let requested = ACTIVE_COMPACTION_REQUEST_PREFIX;
    let states = [
        CompactionRecordState::Absent,
        CompactionRecordState::SummaryClaimed,
        CompactionRecordState::SummaryRefused,
        CompactionRecordState::Staged,
        CompactionRecordState::Installed,
        CompactionRecordState::Refused,
    ];

    assert_eq!(
        (
            states.map(|state| next_compaction_step(requested, state)),
            states.map(|state| next_compaction_step("Help with this passage.", state)),
        ),
        (
            [
                CompactionStep::RequestSummary,
                CompactionStep::ReobserveSummary,
                CompactionStep::Complete,
                CompactionStep::InstallOrRefuse,
                CompactionStep::Complete,
                CompactionStep::Complete,
            ],
            [CompactionStep::Complete; 6],
        )
    );
}

#[test]
fn a_summary_result_names_its_producer_and_known_inputs_and_no_summary_refuses() {
    let source = CompactionSource {
        model_attempt_id: "attempt-1".to_owned(),
        manifest_id: "manifest-1".to_owned(),
        selected: vec![("working_target".to_owned(), "rev-1".to_owned())],
        item_ids: vec!["item-1".to_owned()],
    };
    let observation = |adapter, summary: Option<&str>| SummaryObservation {
        adapter,
        mapping_revision: "mapping".to_owned(),
        summary: summary.map(str::to_owned),
    };
    let staged = |producer, output_text: &str, refusal| StagedCompaction {
        producer,
        mapping_revision: "mapping".to_owned(),
        output_text: output_text.to_owned(),
        loss_facts: vec![COMPACTION_LOSS_SEMANTIC_PRESERVATION_UNKNOWN],
        known_inputs: vec![
            CompactionKnownInput::ModelAttempt {
                id: "attempt-1".to_owned(),
            },
            CompactionKnownInput::Manifest {
                id: "manifest-1".to_owned(),
            },
            CompactionKnownInput::Projection {
                source_class: "working_target".to_owned(),
                source_version: "rev-1".to_owned(),
            },
        ],
        preserved_item_ids: vec!["item-1".to_owned()],
        refusal,
    };

    assert_eq!(
        [
            stage_compaction(
                &source,
                observation(
                    crate::ModelAdapter::HostFake,
                    Some(HOST_FAKE_COMPACTION_OUTPUT)
                )
            ),
            stage_compaction(
                &source,
                observation(crate::ModelAdapter::VolcengineAgentPlanResponses, None)
            ),
        ],
        [
            staged(
                HOST_FAKE_COMPACTION_PRODUCER,
                HOST_FAKE_COMPACTION_OUTPUT,
                None
            ),
            staged(
                AGENT_PLAN_COMPACTION_PRODUCER,
                "",
                Some(COMPACTION_SUMMARY_UNAVAILABLE)
            ),
        ]
    );
}

#[test]
fn a_summary_is_the_one_complete_assistant_message() {
    let message = |text: &str, state| crate::NativeStreamItem {
        item_id: "msg".to_owned(),
        role: crate::StreamItemRole::Assistant,
        state,
        text: Some(text.to_owned()),
        summary: None,
        call_id: None,
        arguments: None,
        refusal: None,
        hosted_report: None,
    };
    let reasoning = crate::NativeStreamItem {
        text: None,
        summary: Some("Plan.".to_owned()),
        ..message("", crate::StreamItemState::Complete)
    };
    let complete = message(
        "The author keeps the rain.",
        crate::StreamItemState::Complete,
    );

    assert_eq!(
        [
            super::summary_text(&[reasoning.clone(), complete.clone()]),
            super::summary_text(&[message("Half", crate::StreamItemState::Incomplete)]),
            super::summary_text(&[complete.clone(), complete]),
            super::summary_text(&[reasoning]),
        ],
        [
            Some("The author keeps the rain.".to_owned()),
            None,
            None,
            None
        ]
    );
}
