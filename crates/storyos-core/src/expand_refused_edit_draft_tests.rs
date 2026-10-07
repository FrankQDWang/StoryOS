use std::convert::Infallible;

use super::{
    ExpandRefusedEditDraftConflict, ExpandRefusedEditDraftRefusal, expand_refused_edit_draft,
};
use crate::{
    AuthorEditPrimitive, AuthorEditUnit, DraftCloseSource, EXCLUSIVE_AUTHORITATIVE_EDGES_V1,
    InlineTargetBlock, ManuscriptBlockKind, OpenInlineProposal, OpenInlineProposalAnchor,
    PROSEMIRROR_TOKEN_UTF16_V1, RefusedEditPayload, ReplacementBlock, SelectionSnapshot,
    TransitionOutcome,
};

const SOURCE: DraftCloseSource<'static> = DraftCloseSource {
    revision: "revision-a",
    digest: "digest-a",
    reopen_event_id: None,
};

fn replacement() -> Vec<ReplacementBlock> {
    vec![ReplacementBlock {
        block_kind: ManuscriptBlockKind::Paragraph,
        text: "A steadier narrator voice.".to_owned(),
    }]
}

fn payload(primitives: Vec<AuthorEditPrimitive>) -> RefusedEditPayload {
    RefusedEditPayload {
        schema_revision: "storyos.refused-edit-payload.v1".to_owned(),
        chapter_id: "chapter-a".to_owned(),
        expected_authoritative_revision_id: "rev-1".to_owned(),
        expected_proposal_head_revision_ids: Vec::new(),
        target_refs: Vec::new(),
        author_edit_units: vec![AuthorEditUnit {
            normalized_primitives: primitives,
            selection_snapshot: SelectionSnapshot {
                ordered_selection: None,
                coordinate_profile: PROSEMIRROR_TOKEN_UTF16_V1.to_owned(),
                from: 10,
                to: 24,
            },
        }],
        undo_group_id: "undo-group-a".to_owned(),
        completed_intent_record_id: "intent-a".to_owned(),
        local_intent_sequence: "1".to_owned(),
    }
}

fn structured() -> RefusedEditPayload {
    payload(vec![AuthorEditPrimitive::ReplaceStructuredSelection {
        replacement: replacement(),
    }])
}

fn target() -> OpenInlineProposal {
    OpenInlineProposal {
        scope_matches: true,
        target_block_present: true,
        expected_base_revision_id: "rev-1".to_owned(),
        current_base_revision_id: Some("rev-1".to_owned()),
        conflicting_reservation: false,
        current_schema_version: 1,
        current_coordinate_profile: PROSEMIRROR_TOKEN_UTF16_V1.to_owned(),
        blocks: vec![InlineTargetBlock {
            manuscript_block_id: "block-a".to_owned(),
            block_kind: "paragraph".to_owned(),
            text: "Guard the narrator voice in this passage.".to_owned(),
        }],
        anchors: vec![OpenInlineProposalAnchor {
            manuscript_block_id: "block-a".to_owned(),
            base_authoritative_revision_id: "rev-1".to_owned(),
            manuscript_schema_version: 1,
            coordinate_profile: PROSEMIRROR_TOKEN_UTF16_V1.to_owned(),
            from: 10,
            to: 24,
            boundary_profile: EXCLUSIVE_AUTHORITATIVE_EDGES_V1.to_owned(),
            base_slice_digest:
                "sha256:bc395ed925fa201c3c0ecd550ed362f89e6069001d6c23af2ce214b107d9ce40".to_owned(),
        }],
    }
}

#[test]
fn an_open_retained_draft_with_one_structured_replacement_creates_a_proposal() {
    assert_eq!(
        expand_refused_edit_draft(
            &SOURCE,
            &SOURCE,
            "open",
            "retained",
            || Ok::<_, Infallible>(structured()),
            &target(),
        ),
        Ok(TransitionOutcome::Applied(replacement()))
    );
}

#[test]
fn a_changed_target_head_conflicts() {
    let moved = OpenInlineProposal {
        current_base_revision_id: Some("rev-2".to_owned()),
        ..target()
    };
    assert_eq!(
        expand_refused_edit_draft(
            &SOURCE,
            &SOURCE,
            "open",
            "retained",
            || Ok::<_, Infallible>(structured()),
            &moved,
        ),
        Ok(TransitionOutcome::Conflicted(
            ExpandRefusedEditDraftConflict::SourceOrTargetChanged
        ))
    );
}

#[test]
fn a_closed_draft_is_refused_without_reading_its_payload() {
    assert_eq!(
        expand_refused_edit_draft(
            &SOURCE,
            &SOURCE,
            "closed",
            "retained",
            || Err("the payload is not read"),
            &target(),
        ),
        Ok(TransitionOutcome::Refused(
            ExpandRefusedEditDraftRefusal::SourceDraftNotOpen
        ))
    );
}

#[test]
fn an_archived_draft_is_refused_as_unavailable() {
    assert_eq!(
        expand_refused_edit_draft(
            &SOURCE,
            &SOURCE,
            "open",
            "archived",
            || Err("the payload is not read"),
            &target(),
        ),
        Ok(TransitionOutcome::Refused(
            ExpandRefusedEditDraftRefusal::SourceUnavailable
        ))
    );
}

#[test]
fn a_payload_without_one_structured_replacement_is_unsupported() {
    let unstructured = payload(vec![AuthorEditPrimitive::ReplaceSelection {
        from: 10,
        to: 24,
        text: "voice".to_owned(),
    }]);
    assert_eq!(
        expand_refused_edit_draft(
            &SOURCE,
            &SOURCE,
            "open",
            "retained",
            || Ok::<_, Infallible>(unstructured),
            &target(),
        ),
        Ok(TransitionOutcome::Refused(
            ExpandRefusedEditDraftRefusal::UnsupportedPayload
        ))
    );
}

#[test]
fn a_missing_target_block_is_refused_as_unavailable() {
    let missing = OpenInlineProposal {
        target_block_present: false,
        ..target()
    };
    assert_eq!(
        expand_refused_edit_draft(
            &SOURCE,
            &SOURCE,
            "open",
            "retained",
            || Ok::<_, Infallible>(structured()),
            &missing,
        ),
        Ok(TransitionOutcome::Refused(
            ExpandRefusedEditDraftRefusal::TargetUnavailable
        ))
    );
}
