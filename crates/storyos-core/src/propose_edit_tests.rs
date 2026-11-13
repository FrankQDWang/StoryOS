use super::*;
use crate::{DeclaredTargetKind, ProseChangeRefusal};

fn declared(block: &str, kind: DeclaredTargetKind) -> DeclaredLocation {
    DeclaredLocation {
        chapter_id: "chapter-1".to_owned(),
        block_id: block.to_owned(),
        base_revision_id: "revision-1".to_owned(),
        kind,
    }
}

fn edit(blocks: &[&str]) -> ProposedEdit {
    ProposedEdit {
        summary: "Shorter.".to_owned(),
        changes: blocks
            .iter()
            .map(|block| BlockChange {
                block_id: (*block).to_owned(),
                candidate_text: "Rain hit the roof.".to_owned(),
                explanation: "Shorter.".to_owned(),
            })
            .collect(),
    }
}

fn refusal(targets: &[DeclaredLocation], blocks: &[&str]) -> Option<ProposeEditRefusal> {
    edit(blocks).validate(targets).err()
}

#[test]
fn a_change_can_change_a_subset_of_the_declared_targets_once_each() {
    let declared =
        ["block-1", "block-2"].map(|block| declared(block, DeclaredTargetKind::ChapterBlock));
    let rule = |rule| Some(ProposeEditRefusal::Validation(rule));

    assert_eq!(
        (
            edit(&["block-2"]).validate(&declared),
            [
                refusal(&declared, &["block-2", "block-2"]),
                refusal(&declared, &[]),
                refusal(&declared, &["block-2", "block-3"]),
                refusal(&[], &["block-1"]),
            ],
        ),
        (
            Ok(vec![ProseChangeCandidate {
                chapter_id: "chapter-1".to_owned(),
                manuscript_block_id: "block-2".to_owned(),
                base_authoritative_revision_id: "revision-1".to_owned(),
                candidate_text: "Rain hit the roof.".to_owned(),
                explanation: "Shorter.".to_owned(),
            }]),
            [
                rule(ProseChangeRefusal::RepeatedBlock),
                rule(ProseChangeRefusal::NoChange),
                rule(ProseChangeRefusal::UndeclaredBlock),
                rule(ProseChangeRefusal::UndeclaredBlock),
            ],
        )
    );
}

#[test]
fn a_change_of_a_pending_candidate_is_the_only_change_of_its_call() {
    let declared = [
        declared("block-1", DeclaredTargetKind::ChapterBlock),
        declared("block-2", DeclaredTargetKind::PendingCandidate),
        declared("block-3", DeclaredTargetKind::PendingCandidate),
    ];
    let accepted = |blocks: &[&str]| refusal(&declared, blocks).is_none();

    assert_eq!(
        [
            accepted(&["block-1"]),
            accepted(&["block-2"]),
            accepted(&["block-1", "block-2"]),
            accepted(&["block-2", "block-3"]),
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
    let accepted = |blocks: &[&str]| refusal(&declared, blocks).is_none();

    assert_eq!(
        [
            accepted(&["candidate", "below-1"]),
            accepted(&["below-2", "below-1", "candidate"]),
            accepted(&["above-1", "candidate", "below-1", "below-2"]),
            accepted(&["below-1", "below-2"]),
            accepted(&["candidate", "chapter"]),
            accepted(&["candidate", "below-1", "chapter"]),
            accepted(&["candidate", "below-2"]),
            accepted(&["above-2", "candidate", "below-1"]),
            accepted(&["candidate", "other-candidate", "below-1"]),
        ],
        [true, true, true, true, false, false, false, false, false]
    );
}

#[test]
fn a_text_with_u0000_is_refused_and_other_control_characters_stay() {
    let declared = [declared("block-1", DeclaredTargetKind::ChapterBlock)];
    let with = |change: fn(&mut ProposedEdit)| {
        let mut edit = edit(&["block-1"]);
        change(&mut edit);
        edit.validate(&declared).err()
    };

    assert_eq!(
        [
            with(|edit| edit.summary = "Short\0er.".to_owned()),
            with(|edit| edit.changes[0].candidate_text = "Rain\0hit.".to_owned()),
            with(|edit| edit.changes[0].explanation = "Short\0er.".to_owned()),
            with(|edit| edit.changes[0].candidate_text = "Rain\n\thit.".to_owned()),
        ],
        [
            Some(ProposeEditRefusal::NulCharacter),
            Some(ProposeEditRefusal::NulCharacter),
            Some(ProposeEditRefusal::NulCharacter),
            None,
        ]
    );
}
