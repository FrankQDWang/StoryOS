use super::*;
use crate::{DeclaredLocation, DeclaredTargetKind};

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
            validate_agent_decision(Some(&matching), &declared),
            validate_agent_decision(Some(&prose_change("block-2")), &declared),
            validate_agent_decision(Some(&matching), &[]),
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
            validate_agent_decision(Some(&subset), &declared),
            validate_agent_decision(Some(&with_changes(&["block-2", "block-2"])), &declared),
            validate_agent_decision(Some(&with_changes(&[])), &declared),
            validate_agent_decision(Some(&with_changes(&["block-2", "block-3"])), &declared),
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
            validate_agent_decision(Some(&with_changes(blocks)), &declared),
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
