use serde_json::{Value, json};

use super::{
    MemoryConsolidationAssembly, PassObservation, PassOperationResult, admit_memory_consolidation,
    assemble_memory_consolidation_context, consolidation_failure, observation_returns,
    pass_observation_count,
};
use crate::assemble_context::{CONTEXT_ITEM_TOKEN_LIMIT, ContextPurpose};
use crate::{
    CaptureRefusal, CaptureRunEnd, ContextSourceClass, CurrentMemoryEntry, MemoryEntryOrigin,
    MemoryEntryType, MemorySourceReference, PassMemoryEntry, ProjectionMode,
};

/// The start uses the admission rule of a capture with the Purpose `memory_consolidation`.
#[test]
fn the_start_of_a_pass_needs_a_binding_an_available_assistance_and_the_purpose() {
    let scopes = [
        (false, true, Some("project_writing_assistance_v5")),
        (true, false, Some("project_writing_assistance_v5")),
        (true, true, Some("project_writing_assistance_v4")),
        (true, true, Some("project_writing_assistance_v5")),
    ];
    assert_eq!(
        scopes.map(|(binding, available, scope)| admit_memory_consolidation(
            binding, available, scope
        )),
        [
            Err(CaptureRefusal::NoModelUseBinding),
            Err(CaptureRefusal::AssistanceUnavailable),
            Err(CaptureRefusal::PurposeNotGranted),
            Ok(())
        ]
    );
}

/// The Context Assembly of a pass has the current entries and the observations as lines, the
/// oldest observation first. A set above the Context item limit is more than one item, and each
/// item fits the limit.
#[test]
fn the_input_of_a_pass_has_one_line_for_each_entry_and_each_observation() {
    let entry = PassMemoryEntry {
        entry: CurrentMemoryEntry {
            memory_entry_id: "entry".to_owned(),
            number: 3,
            version: 2,
            entry_type: MemoryEntryType::Character,
            title: "林舟".to_owned(),
            description: "船夫。".to_owned(),
            text: "沉默的船夫。".to_owned(),
            flagged: false,
        },
        origin: MemoryEntryOrigin::Chapter,
    };
    let observation = |text: String| PassObservation {
        entry_type: MemoryEntryType::Other,
        title: "渡口".to_owned(),
        text,
        source: MemorySourceReference::Chapter {
            chapter_id: "chapter".to_owned(),
        },
        refused: None,
    };
    let entries = [(entry, "2026-11-17T08:00:00.000Z".to_owned())];
    let observations = [
        observation("渡口在河边。".to_owned()),
        observation("长".repeat(1500)),
    ];
    let assemble = |observations: &[PassObservation]| {
        assemble_memory_consolidation_context(&MemoryConsolidationAssembly {
            operation_requirement_id: "requirement".to_owned(),
            run_id: "run".to_owned(),
            owner_user_id: "owner".to_owned(),
            project_id: "project".to_owned(),
            destination_identity: "destination".to_owned(),
            memory_revision: 4,
            entries: &entries,
            unused: &[],
            observations,
            marks: &[],
        })
    };
    let record = assemble(&observations[..1]);
    let lines: Vec<(ContextSourceClass, Vec<Value>)> = record
        .selected
        .iter()
        .map(|item| {
            let lines = item.content.lines();
            (
                item.source_class,
                lines
                    .map(|line| serde_json::from_str(line).unwrap())
                    .collect(),
            )
        })
        .collect();
    assert_eq!(
        (
            record.operation_requirement.purpose,
            record.operation_requirement.memory_revision,
            lines
        ),
        (
            ContextPurpose::MemoryConsolidation,
            Some(4),
            vec![
                (
                    ContextSourceClass::MemoryEntries,
                    vec![json!({
                        "id": 3, "type": "character", "title": "林舟", "description": "船夫。",
                        "text": "沉默的船夫。", "origin": "chapter",
                        "changed_at": "2026-11-17T08:00:00.000Z"
                    })]
                ),
                (
                    ContextSourceClass::MemoryObservations,
                    vec![json!({
                        "observation": 1, "type": "other", "title": "渡口", "text": "渡口在河边。",
                        "source": { "kind": "chapter", "chapter_id": "chapter" }
                    })]
                ),
            ]
        )
    );
    let many: Vec<PassObservation> = (0..40).map(|_| observations[1].clone()).collect();
    let large = assemble(&many);
    let items: Vec<_> = large
        .selected
        .iter()
        .filter(|item| item.source_class == ContextSourceClass::MemoryObservations)
        .collect();
    assert!(items.len() > 1);
    assert!(
        items
            .iter()
            .all(|item| item.token_count <= CONTEXT_ITEM_TOKEN_LIMIT
                && item.projection_mode == ProjectionMode::ExactRequired)
    );
    assert_eq!(
        items
            .iter()
            .map(|item| item.content.lines().count())
            .sum::<usize>(),
        40
    );
}

/// A pass whose one RunStep ends with no settled call fails, and a queued or claimed Run continues.
#[test]
fn a_pass_with_no_settled_call_fails() {
    let status = |status, reason| CaptureRunEnd::Status { status, reason };
    assert_eq!(
        [
            CaptureRunEnd::LastRunStepEnded,
            status("completed", None),
            status("claimed", None),
            status("failed", Some("destination_rejected")),
            status("cancelled", None),
        ]
        .map(consolidation_failure),
        [
            Some("no_operation_call"),
            Some("no_operation_call"),
            None,
            Some("destination_rejected"),
            Some("cancelled")
        ]
    );
}

/// A pass takes the oldest observations whose expected input stays below the window after the
/// output bound. With one more observation, the request is too long. The Host fake has no known
/// window, so its pass takes each observation.
#[test]
fn a_pass_takes_the_oldest_observations_that_fit_the_request() {
    let observations: Vec<PassObservation> = (0..40)
        .map(|_| PassObservation {
            entry_type: MemoryEntryType::Other,
            title: "渡口".to_owned(),
            text: "长".repeat(30_000),
            source: MemorySourceReference::Chapter {
                chapter_id: "chapter".to_owned(),
            },
            refused: None,
        })
        .collect();
    let input = |observations| MemoryConsolidationAssembly {
        operation_requirement_id: "requirement".to_owned(),
        run_id: "run".to_owned(),
        owner_user_id: "owner".to_owned(),
        project_id: "project".to_owned(),
        destination_identity: "destination".to_owned(),
        memory_revision: 1,
        entries: &[],
        unused: &[],
        observations,
        marks: &[],
    };
    let agent_plan = crate::AGENT_PLAN_REGISTRATION.revision;
    let bounds = crate::BudgetBounds::AgentPlanRequestV2.as_str();
    let taken = pass_observation_count(&input(&observations), agent_plan, bounds);
    let too_long = |count: usize| {
        let record = assemble_memory_consolidation_context(&input(&observations[..count]));
        crate::later_request_too_long(agent_plan, bounds, &record, &[])
    };
    let host_fake = crate::HOST_FAKE_TOOL_LOOP_REGISTRATION.revision;

    assert_eq!(
        (
            taken > 0 && taken < observations.len(),
            too_long(taken),
            too_long(taken + 1),
            pass_observation_count(&input(&observations), host_fake, "not_required"),
        ),
        (true, false, true, 40)
    );
}

/// ADR 0056: the expected input of a pass counts each section that the request sends: the
/// entries, the observations, and the delete marks. `pass_observation_count` uses this count, so
/// it stays the one bound of the observations.
#[test]
fn the_expected_input_of_a_pass_counts_each_section() {
    let entries = [(
        PassMemoryEntry {
            entry: CurrentMemoryEntry {
                memory_entry_id: "entry".to_owned(),
                number: 1,
                version: 1,
                entry_type: MemoryEntryType::Character,
                title: "林舟".to_owned(),
                description: "船夫。".to_owned(),
                text: "沉默的船夫。".to_owned(),
                flagged: false,
            },
            origin: MemoryEntryOrigin::Conversation,
        },
        "2026-11-17T08:00:00.000Z".to_owned(),
    )];
    let observations = [PassObservation {
        entry_type: MemoryEntryType::Other,
        title: "渡口".to_owned(),
        text: "渡口在河边。".to_owned(),
        source: MemorySourceReference::Chapter {
            chapter_id: "chapter".to_owned(),
        },
        refused: None,
    }];
    let marks = [crate::MemoryDeleteMark {
        entry_type: MemoryEntryType::Other,
        title: "旧船".to_owned(),
        description: "作者不要的内容。".to_owned(),
    }];
    let record = assemble_memory_consolidation_context(&MemoryConsolidationAssembly {
        operation_requirement_id: "requirement".to_owned(),
        run_id: "run".to_owned(),
        owner_user_id: "owner".to_owned(),
        project_id: "project".to_owned(),
        destination_identity: "destination".to_owned(),
        memory_revision: 1,
        entries: &entries,
        unused: &[1],
        observations: &observations,
        marks: &marks,
    });
    let trigger = crate::CompactionTrigger::from_facts(&crate::CompactionRequestFacts {
        model_registration_revision: crate::AGENT_PLAN_REGISTRATION.revision,
        budget_bounds: "",
        author_message: "",
        decision: Some(&record),
        declared_target_texts: &[],
        prior_reported_tokens: None,
        prior_decision_id: None,
        prior_input: crate::PriorInput::Current,
        decision_position: "0",
        compacted_earlier: false,
    });
    let sections: Vec<(ContextSourceClass, u64)> = (record.selected.iter())
        .map(|item| {
            let tokens = crate::assemble_context::count_context_item_tokens(&item.content);
            (item.source_class, tokens)
        })
        .collect();

    assert_eq!(
        (
            sections.iter().map(|(class, _)| *class).collect::<Vec<_>>(),
            trigger.sent_input_tokens
        ),
        (
            vec![
                ContextSourceClass::MemoryEntries,
                ContextSourceClass::MemoryObservations,
                ContextSourceClass::MemoryDeleteMarks
            ],
            sections.iter().map(|(_, tokens)| tokens).sum::<u64>()
        )
    );
}

/// ADR 0094 section 6: an observation returns when a refused operation of the pass names it, no
/// applied operation of the pass names it, and no refused operation of an earlier pass names it.
#[test]
fn an_observation_returns_one_time_when_only_a_refused_operation_names_it() {
    let results: Vec<PassOperationResult> = serde_json::from_value(json!([
        { "operation": { "observations": [1, 2] }, "result": "refused",
          "refusal": "memory_entry_invalid" },
        { "operation": { "observations": [2] }, "result": "applied", "refusal": null },
    ]))
    .unwrap();

    assert!(observation_returns(&results, 1, false));
    assert!(!observation_returns(&results, 1, true));
    assert!(!observation_returns(&results, 2, false));
    assert!(!observation_returns(&results, 3, false));
}

/// ADR 0094 section 6: the line of an observation that returned has the key `refused` with the
/// label of its refusal, and each other line has no such key.
#[test]
fn the_line_of_an_observation_that_returned_has_the_label_of_its_refusal() {
    let observation = |refused: Option<&str>| PassObservation {
        entry_type: MemoryEntryType::Other,
        title: "渡口".to_owned(),
        text: "渡口在河边。".to_owned(),
        source: MemorySourceReference::Chapter {
            chapter_id: "chapter".to_owned(),
        },
        refused: refused.map(str::to_owned),
    };
    let observations = [observation(Some("memory_entry_invalid")), observation(None)];
    let record = assemble_memory_consolidation_context(&MemoryConsolidationAssembly {
        operation_requirement_id: "requirement".to_owned(),
        run_id: "run".to_owned(),
        owner_user_id: "owner".to_owned(),
        project_id: "project".to_owned(),
        destination_identity: "destination".to_owned(),
        memory_revision: 4,
        entries: &[],
        unused: &[],
        observations: &observations,
        marks: &[],
    });
    let source = json!({ "kind": "chapter", "chapter_id": "chapter" });
    let lines: Vec<Value> = record.selected[0]
        .content
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();

    assert_eq!(
        lines,
        [
            json!({
                "observation": 1, "type": "other", "title": "渡口", "text": "渡口在河边。",
                "source": source, "refused": "memory_entry_invalid"
            }),
            json!({
                "observation": 2, "type": "other", "title": "渡口", "text": "渡口在河边。",
                "source": source
            }),
        ]
    );
}
