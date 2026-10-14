use super::*;

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
    let declared = [(
        "chapter-1".to_owned(),
        "block-1".to_owned(),
        "revision-1".to_owned(),
    )];
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
