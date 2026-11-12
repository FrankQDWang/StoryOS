use super::*;
use crate::{CitableSources, DeclaredLocation, DeclaredTargetKind, ToolCallBatch};

fn declared(block: &str, kind: DeclaredTargetKind) -> DeclaredLocation {
    DeclaredLocation {
        chapter_id: "chapter-1".to_owned(),
        block_id: block.to_owned(),
        base_revision_id: "revision-1".to_owned(),
        kind,
    }
}

fn prose_change(block_id: &str) -> ModelOutput {
    ModelOutput {
        phase: OutputPhase::FinalAnswer,
        candidate: DecisionCandidate::ProseChange {
            text: "Shorter.".to_owned(),
        },
        prose_changes: Some(vec![ProseChangeCandidate {
            chapter_id: "chapter-1".to_owned(),
            base_authoritative_revision_id: "revision-1".to_owned(),
            manuscript_block_id: block_id.to_owned(),
            candidate_text: "Rain hit the roof.".to_owned(),
            explanation: "Shorter.".to_owned(),
        }]),
    }
}

#[test]
fn a_prose_change_is_selected_only_for_the_declared_targets() {
    let declared = [declared("block-1", DeclaredTargetKind::ChapterBlock)];
    let matching = prose_change("block-1");

    assert_eq!(
        (
            validate_agent_decision(
                Some(&matching),
                &declared,
                &CitableSources::default(),
                ToolCallBatch::none()
            ),
            validate_agent_decision(
                Some(&prose_change("block-2")),
                &declared,
                &CitableSources::default(),
                ToolCallBatch::none()
            ),
            validate_agent_decision(
                Some(&matching),
                &[],
                &CitableSources::default(),
                ToolCallBatch::none()
            ),
        ),
        (
            AgentDecisionOutcome::Decision {
                kind: AgentDecisionKind::ProseChange {
                    text: "Shorter.".to_owned(),
                    locations: matching.prose_changes.clone(),
                },
                selected: true,
                advances_continuation: true,
            },
            AgentDecisionOutcome::NoDecision,
            AgentDecisionOutcome::NoDecision,
        )
    );
}

#[test]
fn a_prose_change_can_change_a_subset_of_the_declared_targets_once_each() {
    let declared =
        ["block-1", "block-2"].map(|block| declared(block, DeclaredTargetKind::ChapterBlock));
    let with_changes = |blocks: &[&str]| {
        let mut output = prose_change("block-1");
        let template = output.prose_changes.take().unwrap().pop().unwrap();
        output.prose_changes = Some(
            blocks
                .iter()
                .map(|block| ProseChangeCandidate {
                    manuscript_block_id: (*block).to_owned(),
                    ..template.clone()
                })
                .collect(),
        );
        output
    };
    let subset = with_changes(&["block-2"]);

    assert_eq!(
        [
            validate_agent_decision(
                Some(&subset),
                &declared,
                &CitableSources::default(),
                ToolCallBatch::none()
            ),
            validate_agent_decision(
                Some(&with_changes(&["block-2", "block-2"])),
                &declared,
                &CitableSources::default(),
                ToolCallBatch::none()
            ),
            validate_agent_decision(
                Some(&with_changes(&[])),
                &declared,
                &CitableSources::default(),
                ToolCallBatch::none()
            ),
            validate_agent_decision(
                Some(&with_changes(&["block-2", "block-3"])),
                &declared,
                &CitableSources::default(),
                ToolCallBatch::none()
            ),
        ],
        [
            AgentDecisionOutcome::Decision {
                kind: AgentDecisionKind::ProseChange {
                    text: "Shorter.".to_owned(),
                    locations: subset.prose_changes.clone(),
                },
                selected: true,
                advances_continuation: true,
            },
            AgentDecisionOutcome::NoDecision,
            AgentDecisionOutcome::NoDecision,
            AgentDecisionOutcome::NoDecision,
        ]
    );
}

#[test]
fn a_prose_change_of_a_pending_candidate_is_the_only_change_of_its_result() {
    let declared = [
        declared("block-1", DeclaredTargetKind::ChapterBlock),
        declared("block-2", DeclaredTargetKind::PendingCandidate),
        declared("block-3", DeclaredTargetKind::PendingCandidate),
    ];
    let with_changes = |blocks: &[&str]| {
        let mut output = prose_change("block-1");
        let template = output.prose_changes.take().unwrap().pop().unwrap();
        output.prose_changes = Some(
            blocks
                .iter()
                .map(|block| ProseChangeCandidate {
                    manuscript_block_id: (*block).to_owned(),
                    ..template.clone()
                })
                .collect(),
        );
        output
    };
    let selected = |blocks: &[&str]| {
        matches!(
            validate_agent_decision(
                Some(&with_changes(blocks)),
                &declared,
                &CitableSources::default(),
                ToolCallBatch::none()
            ),
            AgentDecisionOutcome::Decision { .. }
        )
    };

    assert_eq!(
        [
            selected(&["block-1"]),
            selected(&["block-2"]),
            selected(&["block-1", "block-2"]),
            selected(&["block-2", "block-3"]),
        ],
        [true, true, false, false]
    );
}

#[test]
fn a_growth_changes_the_candidate_and_one_adjacent_run_of_joinable_blocks() {
    let declared = [
        declared(
            "above-2",
            DeclaredTargetKind::JoinableBlock { distance: -2 },
        ),
        declared(
            "above-1",
            DeclaredTargetKind::JoinableBlock { distance: -1 },
        ),
        declared("candidate", DeclaredTargetKind::PendingCandidate),
        declared("below-1", DeclaredTargetKind::JoinableBlock { distance: 1 }),
        declared("below-2", DeclaredTargetKind::JoinableBlock { distance: 2 }),
        declared("chapter", DeclaredTargetKind::ChapterBlock),
        declared("other-candidate", DeclaredTargetKind::PendingCandidate),
    ];
    let with_changes = |blocks: &[&str]| {
        let mut output = prose_change("candidate");
        let template = output.prose_changes.take().unwrap().pop().unwrap();
        output.prose_changes = Some(
            blocks
                .iter()
                .map(|block| ProseChangeCandidate {
                    manuscript_block_id: (*block).to_owned(),
                    ..template.clone()
                })
                .collect(),
        );
        output
    };
    let selected = |blocks: &[&str]| {
        matches!(
            validate_agent_decision(
                Some(&with_changes(blocks)),
                &declared,
                &CitableSources::default(),
                ToolCallBatch::none()
            ),
            AgentDecisionOutcome::Decision { .. }
        )
    };

    assert_eq!(
        [
            selected(&["candidate", "below-1"]),
            selected(&["below-2", "below-1", "candidate"]),
            selected(&["above-1", "candidate", "below-1", "below-2"]),
            selected(&["below-1", "below-2"]),
            selected(&["candidate", "chapter"]),
            selected(&["candidate", "below-1", "chapter"]),
            selected(&["candidate", "below-2"]),
            selected(&["above-2", "candidate", "below-1"]),
            selected(&["candidate", "other-candidate", "below-1"]),
        ],
        [true, true, true, true, false, false, false, false, false]
    );
}

#[test]
fn a_decision_text_with_u0000_makes_no_decision_and_other_control_characters_stay() {
    let declared = [declared("block-1", DeclaredTargetKind::ChapterBlock)];
    let decide = |output: &ModelOutput| {
        validate_agent_decision(
            Some(output),
            &declared,
            &CitableSources::default(),
            ToolCallBatch::none(),
        )
    };
    let with_candidate = |candidate: DecisionCandidate| ModelOutput {
        phase: OutputPhase::FinalAnswer,
        candidate,
        prose_changes: None,
    };
    let with_change = |edit: fn(&mut ProseChangeCandidate)| {
        let mut output = prose_change("block-1");
        output
            .prose_changes
            .as_mut()
            .unwrap()
            .iter_mut()
            .for_each(edit);
        output
    };
    let with_nul = [
        with_candidate(DecisionCandidate::Advisory {
            text: "Keep\0it.".to_owned(),
        }),
        with_candidate(DecisionCandidate::Clarification {
            question: "Which\0scene?".to_owned(),
        }),
        ModelOutput {
            candidate: DecisionCandidate::ProseChange {
                text: "Short\0er.".to_owned(),
            },
            ..prose_change("block-1")
        },
        with_change(|change| change.candidate_text = "Rain\0hit.".to_owned()),
        with_change(|change| change.explanation = "Short\0er.".to_owned()),
    ];

    assert_eq!(
        (
            with_nul.iter().map(decide).collect::<Vec<_>>(),
            decide(&with_candidate(DecisionCandidate::Advisory {
                text: "Keep\n\tit.".to_owned(),
            })),
        ),
        (
            vec![AgentDecisionOutcome::NoDecision; 5],
            AgentDecisionOutcome::Decision {
                kind: AgentDecisionKind::Advisory {
                    text: "Keep\n\tit.".to_owned(),
                },
                selected: true,
                advances_continuation: true,
            },
        )
    );
}
