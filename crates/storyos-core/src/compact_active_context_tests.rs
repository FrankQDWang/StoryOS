use super::{
    ACTIVE_COMPACTION_REQUEST_PREFIX, AGENT_PLAN_COMPACTION_PRODUCER,
    COMPACTION_LOSS_SEMANTIC_PRESERVATION_UNKNOWN, COMPACTION_SUMMARY_UNAVAILABLE,
    CompactionInstallFacts, CompactionInstallRefusal, CompactionKnownInput, CompactionRecordState,
    CompactionSource, CompactionStep, CompactionTrigger, HOST_FAKE_COMPACTION_OUTPUT,
    HOST_FAKE_COMPACTION_PRODUCER, StagedCompaction, SummaryObservation,
    active_context_input_digest, compaction_due, compaction_threshold, conversation_too_long,
    decide_compaction_install, later_request_record, next_compaction_step,
    requests_active_compaction, stage_compaction,
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
        summary_within_item_limit: true,
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
fn install_refuses_a_restricted_changed_or_incomplete_exact_input_or_a_large_summary() {
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
    assert_eq!(
        decide_compaction_install(&CompactionInstallFacts {
            summary_within_item_limit: false,
            ..facts(
                /*source_restricted*/ false, /*exact_required_satisfied*/ true, staged,
                staged,
            )
        }),
        Err(CompactionInstallRefusal::SummaryOverItemLimit)
    );
}

#[test]
fn a_requested_compaction_summarizes_the_prior_response_once_before_the_decision_request() {
    let states = [
        CompactionRecordState::Absent,
        CompactionRecordState::SummaryClaimed,
        CompactionRecordState::Staged,
        CompactionRecordState::Accepted,
        CompactionRecordState::Installed,
        CompactionRecordState::Refused,
    ];

    assert_eq!(
        (
            states.map(|state| {
                next_compaction_step(/*requested*/ true, /*prior_response*/ true, state)
            }),
            next_compaction_step(
                /*requested*/ true,
                /*prior_response*/ false,
                CompactionRecordState::Absent
            ),
            states.map(|state| {
                next_compaction_step(
                    /*requested*/ false, /*prior_response*/ true, state,
                )
            }),
        ),
        (
            [
                CompactionStep::RequestSummary,
                CompactionStep::ReobserveSummary,
                CompactionStep::InstallOrRefuse,
                CompactionStep::LaterRequest,
                CompactionStep::LaterRequest,
                CompactionStep::OrdinaryRequest,
            ],
            CompactionStep::OrdinaryRequest,
            // A compaction with a record continues when the request changes.
            [
                CompactionStep::OrdinaryRequest,
                CompactionStep::ReobserveSummary,
                CompactionStep::InstallOrRefuse,
                CompactionStep::LaterRequest,
                CompactionStep::LaterRequest,
                CompactionStep::OrdinaryRequest,
            ],
        )
    );
}

#[test]
fn the_threshold_is_eighty_percent_of_the_window_after_the_output_bound() {
    let trigger = |window, output_bound, prior, new_turn_tokens| CompactionTrigger {
        window_tokens: window,
        output_bound,
        prior_reported_tokens: prior,
        new_turn_tokens,
    };
    let doubao = (Some(262_144), Some(32_768));

    assert_eq!(
        (
            compaction_threshold(/*window_tokens*/ 262_144, /*output_bound*/ 32_768),
            compaction_threshold(/*window_tokens*/ 8_192, /*output_bound*/ 8_192),
            [
                trigger(doubao.0, doubao.1, Some(180_000), 3_500),
                trigger(doubao.0, doubao.1, Some(180_000), 3_499),
                trigger(None, doubao.1, Some(260_000), 3_500),
                trigger(doubao.0, None, Some(260_000), 3_500),
                trigger(doubao.0, doubao.1, None, 3_500),
            ]
            .map(|facts| compaction_due(&facts)),
            [
                trigger(doubao.0, doubao.1, Some(229_000), 376),
                trigger(doubao.0, doubao.1, Some(229_000), 375),
                trigger(None, doubao.1, Some(300_000), 0),
            ]
            .map(|facts| conversation_too_long(&facts)),
        ),
        (
            Some(183_500),
            None,
            [true, false, false, false, false],
            [true, false, false],
        )
    );
}

#[test]
fn the_later_request_selects_the_summary_first_as_a_derived_summary() {
    let decision = crate::assemble_current_passage_context(&crate::CurrentPassageAssembly {
        operation_requirement_id: "requirement-1".to_owned(),
        input_snapshot_id: "snapshot-1".to_owned(),
        run_id: "run-1".to_owned(),
        owner_user_id: "owner-1".to_owned(),
        project_id: "project-1".to_owned(),
        author_message: "Help again.".to_owned(),
        chapter_id: "chapter-1".to_owned(),
        chapter_revision_id: Some("rev-1".to_owned()),
        proposal_target_block_ids: None,
        chapter_body: "Passage.".to_owned(),
        instruction: crate::InstructionBindingInput::Absent,
        destination_identity: "destination-1".to_owned(),
    });
    let later = later_request_record(&decision, "compaction-1", "The author keeps the rain.");
    let first = &later.selected[0];

    assert_eq!(
        (
            first.source_class,
            first.source_version.as_str(),
            first.projection_mode,
            first.content.as_str(),
            later.selected.len(),
            later.considered[0].eligible,
            later.sufficiency == decision.sufficiency,
        ),
        (
            crate::ContextSourceClass::ConversationSummary,
            "compaction-1",
            crate::ProjectionMode::DerivedSummary,
            "The author keeps the rain.",
            decision.selected.len() + 1,
            true,
            true,
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
