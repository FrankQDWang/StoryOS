use super::{
    ACTIVE_COMPACTION_POSITIONS_PREFIX, ACTIVE_COMPACTION_REQUEST_PREFIX,
    AGENT_PLAN_COMPACTION_PRODUCER, COMPACTION_LOSS_SEMANTIC_PRESERVATION_UNKNOWN,
    COMPACTION_SUMMARY_UNAVAILABLE, CompactionInstallFacts, CompactionInstallRefusal,
    CompactionKnownInput, CompactionRecordState, CompactionSource, CompactionStep,
    CompactionTrigger, HOST_FAKE_COMPACTION_OUTPUT, HOST_FAKE_COMPACTION_PRODUCER,
    StagedCompaction, SummaryObservation, active_context_input_digest, compaction_due,
    compaction_threshold, conversation_too_long, decide_compaction_install, later_request_record,
    next_compaction_step, stage_compaction,
};
use crate::shared_tests::passage_assembly;

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
    let trigger = |window, output_bound, prior, sent_input_tokens| CompactionTrigger {
        window_tokens: window,
        output_bound,
        prior_reported_tokens: prior,
        sent_input_tokens,
    };
    let doubao = (Some(262_144), Some(32_768));

    assert_eq!(
        (
            compaction_threshold(/*window_tokens*/ 262_144, /*output_bound*/ 32_768),
            compaction_threshold(
                /*window_tokens*/ 1_048_576, /*output_bound*/ 32_768
            ),
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
            Some(812_646),
            None,
            [true, false, false, false, false],
            [true, false, false],
        )
    );
}

#[test]
fn the_later_request_selects_the_summary_first_as_a_derived_summary() {
    let decision =
        crate::assemble_current_passage_context(&passage_assembly("Help again.", "Passage."));
    let later = later_request_record(&decision, "compaction-1", "The author keeps the rain.", &[]);
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
        native_type: None,
        name: None,
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

#[test]
fn the_phase_requests_compaction_from_the_threshold_or_the_host_fake_prefix() {
    let facts = |revision, message, prior| super::CompactionRequestFacts {
        model_registration_revision: revision,
        budget_bounds: crate::BudgetBounds::AgentPlanRequestV2.as_str(),
        author_message: message,
        decision: None,
        declared_target_texts: &[],
        prior_reported_tokens: prior,
        prior_decision_id: None,
        prior_input: crate::PriorInput::Current,
        decision_position: "0",
        compacted_earlier: false,
    };
    let agent_plan = crate::AGENT_PLAN_REGISTRATION.revision;
    let undated = crate::AGENT_PLAN_RECOVERY_REGISTRATION.revision;
    let host_fake = crate::HOST_FAKE_REGISTRATION.revision;
    let near_window = facts(agent_plan, "Help again.", Some(1_015_900));

    assert_eq!(
        (
            [
                facts(agent_plan, "Help again.", Some(812_646)),
                facts(agent_plan, "Help again.", Some(812_645)),
                facts(undated, "Help again.", Some(183_500)),
                facts(agent_plan, "Help again.", Some(183_500)),
                facts(agent_plan, ACTIVE_COMPACTION_REQUEST_PREFIX, Some(10)),
                facts(host_fake, ACTIVE_COMPACTION_REQUEST_PREFIX, Some(10)),
                facts(host_fake, "Help again.", Some(300_000)),
            ]
            .map(|facts| super::compaction_requested(&facts)),
            [
                super::decision_request_kind(true, true, Some(&near_window)),
                super::decision_request_kind(false, true, Some(&near_window)),
                super::decision_request_kind(false, false, Some(&near_window)),
                super::decision_request_kind(
                    false,
                    true,
                    Some(&facts(agent_plan, "Help again.", Some(1_015_807)))
                ),
                super::decision_request_kind(false, true, None),
            ],
        ),
        (
            [true, false, true, false, false, true, false],
            [
                super::DecisionRequestKind::Later,
                super::DecisionRequestKind::TooLong,
                super::DecisionRequestKind::Ordinary,
                super::DecisionRequestKind::Ordinary,
                super::DecisionRequestKind::Ordinary,
            ],
        )
    );
}

#[test]
fn the_text_of_overlapping_selection_references_counts_toward_compaction() {
    let chapter_body = "雨".repeat(/*n*/ 32_000);
    let mut decision =
        crate::assemble_current_passage_context(&passage_assembly("Help again.", &chapter_body));
    let due = |decision: &crate::CurrentPassageAssemblyRecord| {
        super::compaction_requested(&super::CompactionRequestFacts {
            model_registration_revision: crate::AGENT_PLAN_REGISTRATION.revision,
            budget_bounds: crate::BudgetBounds::AgentPlanRequestV2.as_str(),
            author_message: "Help again.",
            decision: Some(decision),
            declared_target_texts: &[],
            prior_reported_tokens: Some(780_000),
            prior_decision_id: None,
            prior_input: crate::PriorInput::Current,
            decision_position: "0",
            compacted_earlier: false,
        })
    };
    let without_references = due(&decision);
    let point = |offset| crate::TextPoint {
        manuscript_block_id: "block-1".to_owned(),
        offset,
    };
    decision.operation_requirement.selection_references = Some(
        (0..8)
            .map(|start| crate::AdmittedSelection {
                reference: crate::SelectionReference::ChapterText {
                    chapter_id: "chapter-1".to_owned(),
                    base_authoritative_revision_id: "rev-1".to_owned(),
                    start: point(start),
                    end: point(/*offset*/ 32_000),
                },
                selected_text: Some(chapter_body.chars().skip(start as usize).collect()),
                manuscript_block_ids: vec!["block-1".to_owned()],
                candidate_text: None,
            })
            .collect(),
    );

    assert_eq!((without_references, due(&decision)), (false, true));
}

#[test]
fn the_host_fake_compacts_one_time_or_at_each_named_run_step() {
    let requested = |message: &str, decision_position, compacted_earlier| {
        super::compaction_requested(&super::CompactionRequestFacts {
            model_registration_revision: crate::HOST_FAKE_REGISTRATION.revision,
            budget_bounds: crate::BudgetBounds::AgentPlanRequestV2.as_str(),
            author_message: message,
            decision: None,
            declared_target_texts: &[],
            prior_reported_tokens: None,
            prior_decision_id: None,
            prior_input: crate::PriorInput::Current,
            decision_position,
            compacted_earlier,
        })
    };
    let once = format!("{ACTIVE_COMPACTION_REQUEST_PREFIX} Help with this passage.");
    let named = format!(
        "{ACTIVE_COMPACTION_REQUEST_PREFIX} {ACTIVE_COMPACTION_POSITIONS_PREFIX} 0 2. Search the web: rain"
    );

    assert_eq!(
        [
            requested(&once, "1", false),
            requested(&once, "1", true),
            requested(&named, "0", false),
            requested(&named, "1", false),
            requested(&named, "2", true),
            requested("At RunSteps 0. Help.", "0", false),
        ],
        [true, false, true, false, true, false]
    );
}

#[test]
fn a_later_request_whose_full_input_reaches_the_window_is_too_long() {
    let decision =
        crate::assemble_current_passage_context(&passage_assembly("Help again.", "Rain."));
    let others: u64 = decision.selected.iter().map(|item| item.token_count).sum();
    let targets = ["Target.".to_owned()];
    // The window after the output bound is 1,048,576 - 32,768 = 1,015,808 tokens.
    let too_long = |revision, summary_tokens| {
        let mut later = later_request_record(&decision, "compaction-1", "Summary.", &[]);
        later.selected[0].token_count = summary_tokens;
        super::later_request_too_long(
            revision,
            crate::BudgetBounds::AgentPlanRequestV2.as_str(),
            &later,
            &targets,
        )
    };
    let at_window = 1_015_808 - others - 7;

    assert_eq!(
        [
            too_long(crate::AGENT_PLAN_REGISTRATION.revision, at_window),
            too_long(crate::AGENT_PLAN_REGISTRATION.revision, at_window - 1),
            too_long(crate::HOST_FAKE_REGISTRATION.revision, at_window),
        ],
        [true, false, false]
    );
}

/// ADR 0093 section 5: the full input of a later request counts the stubs, not the exact results.
#[test]
fn the_full_input_of_a_later_request_counts_each_stub_and_not_its_exact_result() {
    let mut decision =
        crate::assemble_current_passage_context(&passage_assembly("Research the tides.", "Rain."));
    // 32 results at the item limit are above the window after the output bound.
    let text = "Tide. ".repeat(6_000);
    let ids: Vec<String> = (0..32).map(|index| format!("call_{index:02}")).collect();
    let candidates = |form| -> Vec<crate::ToolResultCandidate<'_>> {
        ids.iter()
            .map(|id| crate::ToolResultCandidate {
                tool_call_id: id,
                call_id: id,
                owner_user_id: "owner-1",
                project_id: "project-1",
                run_id: "run-1",
                model_attempt_id: "attempt-1",
                decision_id: "decision-1",
                state: crate::ToolCallState::Settled,
                result_text: Some(&text),
                sources: &[],
                output: crate::ToolResultOutput::Research,
                function_name: "web_search",
                arguments: r#"{"query":"tides","result_count":null}"#,
                form,
                skill: None,
                resource_path: None,
            })
            .collect()
    };
    crate::assemble_tool_result_continuation(
        &mut decision,
        &candidates(crate::ToolResultForm::Exact),
    );
    let too_long = |form| {
        let later = later_request_record(&decision, "compaction-1", "Summary.", &candidates(form));
        super::later_request_too_long(
            crate::AGENT_PLAN_REGISTRATION.revision,
            crate::BudgetBounds::AgentPlanRequestV2.as_str(),
            &later,
            &[],
        )
    };

    assert_eq!(
        [
            too_long(crate::ToolResultForm::Exact),
            too_long(crate::ToolResultForm::Stub),
        ],
        [true, false]
    );
}

/// ADR 0056 and ADR 0095 section 1: the expected input counts the `chapter_list` item when the
/// request sends the current input.
#[test]
fn the_expected_input_counts_the_chapter_list_with_the_current_input() {
    let mut decision =
        crate::assemble_current_passage_context(&passage_assembly("Write the quarrel.", "Rain."));
    let without_list: u64 = decision.selected.iter().map(|item| item.token_count).sum();
    crate::assemble_chapter_list(
        &mut decision,
        &[crate::ChapterListEntry {
            volume_order: 1,
            volume_title: "Volume 1".to_owned(),
            chapter_id: "chapter-1".to_owned(),
            title: "The Harbor".to_owned(),
            length: 5,
        }],
    );
    let list_tokens = decision.selected.last().map(|item| item.token_count);
    let sent = CompactionTrigger::from_facts(&super::CompactionRequestFacts {
        model_registration_revision: crate::AGENT_PLAN_REGISTRATION.revision,
        budget_bounds: crate::BudgetBounds::AgentPlanRequestV2.as_str(),
        author_message: "Write the quarrel.",
        decision: Some(&decision),
        declared_target_texts: &[],
        prior_reported_tokens: Some(0),
        prior_decision_id: None,
        prior_input: crate::PriorInput::Current,
        decision_position: "0",
        compacted_earlier: false,
    })
    .sent_input_tokens;

    assert_eq!((sent, list_tokens), (without_list + 50, Some(50)));
}

/// ADR 0056 and ADR 0094 section 4: the expected input counts the `memory_index` item when the
/// request sends the current input.
#[test]
fn the_expected_input_counts_the_memory_index_with_the_current_input() {
    let mut decision =
        crate::assemble_current_passage_context(&passage_assembly("Write the quarrel.", "Rain."));
    let without_index: u64 = decision.selected.iter().map(|item| item.token_count).sum();
    crate::assemble_memory_index(
        &mut decision,
        Some(1),
        &[crate::MemoryIndexEntry {
            number: 1,
            entry_type: crate::MemoryEntryType::Character,
            title: "Lin".to_owned(),
            description: "A quiet ferryman.".to_owned(),
            flagged: false,
        }],
    );
    let index_tokens = decision.selected.last().map(|item| item.token_count);
    let sent = CompactionTrigger::from_facts(&super::CompactionRequestFacts {
        model_registration_revision: crate::AGENT_PLAN_REGISTRATION.revision,
        budget_bounds: crate::BudgetBounds::AgentPlanRequestV2.as_str(),
        author_message: "Write the quarrel.",
        decision: Some(&decision),
        declared_target_texts: &[],
        prior_reported_tokens: Some(0),
        prior_decision_id: None,
        prior_input: crate::PriorInput::Current,
        decision_position: "0",
        compacted_earlier: false,
    })
    .sent_input_tokens;

    assert_eq!((sent, index_tokens), (without_index + 77, Some(77)));
}

/// ADR 0056 and ADR 0089 section 3: the expected input counts the catalog item with the current
/// input and each load output and read output of the prior response. A later full input counts
/// each such output in its exact form, because a load output and a read output have no stub.
#[test]
fn the_expected_input_counts_the_skill_catalog_and_each_exact_load_and_read_output() {
    let mut decision =
        crate::assemble_current_passage_context(&passage_assembly("Write the quarrel.", "Rain."));
    let without_catalog: u64 = decision.selected.iter().map(|item| item.token_count).sum();
    let references = ["dialogue", "plot-notes"].map(|name| crate::SkillPackageReference {
        name: name.to_owned(),
        package_digest: format!("sha256:{}", "d".repeat(64)),
    });
    let catalog = references
        .clone()
        .map(|reference| crate::SkillCatalogEntry {
            reference,
            description: "Use it for a scene.".to_owned(),
        });
    crate::assemble_skill_catalog(&mut decision, &catalog);
    let current: u64 = decision.selected.iter().map(|item| item.token_count).sum();
    let catalog_tokens =
        crate::assemble_context::count_context_item_tokens(&crate::skill_catalog_text(&catalog));
    fn sent(decision: &crate::CurrentPassageAssemblyRecord, prior_input: crate::PriorInput) -> u64 {
        CompactionTrigger::from_facts(&super::CompactionRequestFacts {
            model_registration_revision: crate::AGENT_PLAN_REGISTRATION.revision,
            budget_bounds: crate::BudgetBounds::AgentPlanRequestV2.as_str(),
            author_message: "Write the quarrel.",
            decision: Some(decision),
            declared_target_texts: &[],
            prior_reported_tokens: Some(0),
            prior_decision_id: Some("decision-1"),
            prior_input,
            decision_position: "1",
            compacted_earlier: false,
        })
        .sent_input_tokens
    }
    let first = sent(&decision, crate::PriorInput::Current);
    // One long load and one long read of the prior response.
    let texts = [
        "Keep each line short. ".repeat(1_400),
        "Name each place. ".repeat(1_800),
    ];
    let candidates = |form| -> Vec<crate::ToolResultCandidate<'_>> {
        ["load-1", "read-2"]
            .iter()
            .zip(&texts)
            .zip(&references)
            .map(|((id, text), reference)| crate::ToolResultCandidate {
                tool_call_id: id,
                call_id: id,
                owner_user_id: "owner-1",
                project_id: "project-1",
                run_id: "run-1",
                model_attempt_id: "attempt-1",
                decision_id: "decision-1",
                state: crate::ToolCallState::Settled,
                result_text: Some(text),
                sources: &[],
                output: crate::ToolResultOutput::HostTool(crate::ContextSourceClass::ToolResult),
                function_name: "load_skill",
                arguments: "{}",
                form,
                skill: Some(reference),
                resource_path: (*id == "read-2").then_some("references/places.md"),
            })
            .collect()
    };
    crate::assemble_tool_result_continuation(
        &mut decision,
        &candidates(crate::ToolResultForm::Exact),
    );
    let loads: u64 = texts
        .iter()
        .map(|text| crate::assemble_context::count_context_item_tokens(text))
        .sum();
    // The window after the output bound is 1,048,576 - 32,768 = 1,015,808 tokens.
    let too_long = |summary_tokens| {
        let mut later = later_request_record(
            &decision,
            "compaction-1",
            "Summary.",
            &candidates(crate::ToolResultForm::Stub),
        );
        later.selected[0].token_count = summary_tokens;
        super::later_request_too_long(
            crate::AGENT_PLAN_REGISTRATION.revision,
            crate::BudgetBounds::AgentPlanRequestV2.as_str(),
            &later,
            &[],
        )
    };
    let at_window = 1_015_808 - current - loads;

    assert_eq!(
        (
            current - without_catalog,
            first,
            sent(&decision, crate::PriorInput::Current),
            sent(&decision, crate::PriorInput::Replaced),
            [too_long(at_window), too_long(at_window - 1)],
        ),
        (
            catalog_tokens,
            current,
            loads,
            current + loads,
            [true, false]
        )
    );
}

/// ADR 0056 and ADR 0095 section 1: the expected input counts each `read_chapter` and
/// `search_manuscript` output of the prior response, with its boundary. A later full input counts
/// each such output in its exact form, because a read or a search of the manuscript has no stub.
#[test]
fn the_expected_input_counts_each_exact_chapter_read_and_search_output() {
    let mut decision =
        crate::assemble_current_passage_context(&passage_assembly("Write the quarrel.", "Rain."));
    let current: u64 = decision.selected.iter().map(|item| item.token_count).sum();
    let sent = |decision: &crate::CurrentPassageAssemblyRecord, prior_input| {
        CompactionTrigger::from_facts(&super::CompactionRequestFacts {
            model_registration_revision: crate::AGENT_PLAN_REGISTRATION.revision,
            budget_bounds: crate::BudgetBounds::AgentPlanRequestV2.as_str(),
            author_message: "Write the quarrel.",
            decision: Some(decision),
            declared_target_texts: &[],
            prior_reported_tokens: Some(0),
            prior_decision_id: Some("decision-1"),
            prior_input,
            decision_position: "1",
            compacted_earlier: false,
        })
        .sent_input_tokens
    };
    // A long read and a long search of the prior response, above the stub limit.
    let texts = ["Rain fell. ".repeat(1_400), "The quay. ".repeat(1_800)];
    let specs = [
        crate::READ_CHAPTER_TOOL_SPEC,
        crate::SEARCH_MANUSCRIPT_TOOL_SPEC,
    ];
    let candidates = |form| -> Vec<crate::ToolResultCandidate<'_>> {
        (["read-1", "search-1"].iter().zip(&texts).zip(&specs))
            .map(|((id, text), spec)| crate::ToolResultCandidate {
                tool_call_id: id,
                call_id: id,
                owner_user_id: "owner-1",
                project_id: "project-1",
                run_id: "run-1",
                model_attempt_id: "attempt-1",
                decision_id: "decision-1",
                state: crate::ToolCallState::Settled,
                result_text: Some(text),
                sources: &[],
                output: crate::ToolResultOutput::of_spec(Some(spec.revision)),
                function_name: spec.operation.function_name(),
                arguments: "{}",
                form,
                skill: None,
                resource_path: None,
            })
            .collect()
    };
    crate::assemble_tool_result_continuation(
        &mut decision,
        &candidates(crate::ToolResultForm::Exact),
    );
    let reads: u64 = texts
        .iter()
        .zip(["read-1", "search-1"])
        .map(|(text, id)| {
            crate::assemble_context::count_context_item_tokens(
                &crate::ToolResultProjection::Ok(text).continuation_text(id),
            )
        })
        .sum();
    // The window after the output bound is 1,048,576 - 32,768 = 1,015,808 tokens.
    let too_long = |summary_tokens| {
        let mut later = later_request_record(
            &decision,
            "compaction-1",
            "Summary.",
            &candidates(crate::ToolResultForm::Stub),
        );
        later.selected[0].token_count = summary_tokens;
        super::later_request_too_long(
            crate::AGENT_PLAN_REGISTRATION.revision,
            crate::BudgetBounds::AgentPlanRequestV2.as_str(),
            &later,
            &[],
        )
    };
    let at_window = 1_015_808 - current - reads;

    assert_eq!(
        (
            sent(&decision, crate::PriorInput::Current),
            sent(&decision, crate::PriorInput::Replaced),
            [too_long(at_window), too_long(at_window - 1)],
        ),
        (reads, current + reads, [true, false])
    );
}
