use super::{
    EXCLUSIVE_AUTHORITATIVE_EDGES_V1, InlineInputOwner, InlineTargetBlock, OpenInlineProposal,
    OpenInlineProposalAnchor, OpenInlineProposalConflict, OpenInlineProposalRefusal,
    OpenInlineProposalResult, PROSEMIRROR_TOKEN_UTF16_V1, classify_inline_input_owner,
    open_inline_proposal,
};

const BLOCK_TEXT: &str = "Guard the narrator voice in this passage.";
const SLICE: &str = "narrator voice";
const GOLDEN_SLICE_DIGEST: &str =
    "sha256:bc395ed925fa201c3c0ecd550ed362f89e6069001d6c23af2ce214b107d9ce40";

fn exact_inline() -> OpenInlineProposal {
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
            text: BLOCK_TEXT.to_owned(),
        }],
        anchors: vec![OpenInlineProposalAnchor {
            manuscript_block_id: "block-a".to_owned(),
            base_authoritative_revision_id: "rev-1".to_owned(),
            manuscript_schema_version: 1,
            coordinate_profile: PROSEMIRROR_TOKEN_UTF16_V1.to_owned(),
            from: 10,
            to: 24,
            boundary_profile: EXCLUSIVE_AUTHORITATIVE_EDGES_V1.to_owned(),
            base_slice_digest: GOLDEN_SLICE_DIGEST.to_owned(),
        }],
    }
}

#[test]
fn opens_one_inline_proposal_for_exact_anchors() {
    assert_eq!(&BLOCK_TEXT[10..24], SLICE);
    let opened = open_inline_proposal(&exact_inline());
    assert_eq!(opened, OpenInlineProposalResult::Applied);
    assert_eq!(opened.validation_receipt_result(), Some("valid"));
}

#[test]
fn refuses_wrong_scope_unavailable_target_and_unsupported_profiles() {
    let mut wrong_scope = exact_inline();
    wrong_scope.scope_matches = false;
    assert_eq!(
        open_inline_proposal(&wrong_scope),
        OpenInlineProposalResult::Refused {
            reason: OpenInlineProposalRefusal::WrongScope,
        }
    );
    let mut missing = exact_inline();
    missing.target_block_present = false;
    assert_eq!(
        open_inline_proposal(&missing),
        OpenInlineProposalResult::Refused {
            reason: OpenInlineProposalRefusal::UnavailableTarget,
        }
    );
    let mut no_head = exact_inline();
    no_head.current_base_revision_id = None;
    assert_eq!(
        open_inline_proposal(&no_head),
        OpenInlineProposalResult::Refused {
            reason: OpenInlineProposalRefusal::UnavailableTarget,
        }
    );
    let mut bad_profile = exact_inline();
    bad_profile.anchors[0].coordinate_profile = "utf8-bytes".to_owned();
    assert_eq!(
        open_inline_proposal(&bad_profile),
        OpenInlineProposalResult::Refused {
            reason: OpenInlineProposalRefusal::UnsupportedAnchorContract,
        }
    );
}

#[test]
fn conflicts_changed_head_outside_the_displayed_range() {
    let mut changed = exact_inline();
    changed.current_base_revision_id = Some("rev-2".to_owned());
    assert_eq!(
        open_inline_proposal(&changed),
        OpenInlineProposalResult::Conflicted {
            reason: OpenInlineProposalConflict::ChangedHead,
        }
    );
}

#[test]
fn conflicts_reservation_invalid_anchor_digest_and_coordinate_drift() {
    let mut reserved = exact_inline();
    reserved.conflicting_reservation = true;
    assert_eq!(
        open_inline_proposal(&reserved),
        OpenInlineProposalResult::Conflicted {
            reason: OpenInlineProposalConflict::ConflictingReservation,
        }
    );
    let mut inverted = exact_inline();
    inverted.anchors[0].from = 24;
    inverted.anchors[0].to = 10;
    assert_eq!(
        open_inline_proposal(&inverted),
        OpenInlineProposalResult::Conflicted {
            reason: OpenInlineProposalConflict::InvalidAnchor,
        }
    );
    let mut digest = exact_inline();
    digest.anchors[0].base_slice_digest =
        "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_owned();
    assert_eq!(
        open_inline_proposal(&digest),
        OpenInlineProposalResult::Conflicted {
            reason: OpenInlineProposalConflict::BaseSliceMismatch,
        }
    );
    let mut drifted = exact_inline();
    drifted.current_coordinate_profile = "storyos.editor.utf16-code-unit.v1".to_owned();
    assert_eq!(
        open_inline_proposal(&drifted),
        OpenInlineProposalResult::Conflicted {
            reason: OpenInlineProposalConflict::CoordinateInterpretationChanged,
        }
    );
}

#[test]
fn preserves_ordered_anchors_across_two_blocks() {
    let mut multi = exact_inline();
    multi.blocks.push(InlineTargetBlock {
        manuscript_block_id: "block-b".to_owned(),
        block_kind: "paragraph".to_owned(),
        text: "Second block.".to_owned(),
    });
    multi.anchors.push(OpenInlineProposalAnchor {
        manuscript_block_id: "block-b".to_owned(),
        base_authoritative_revision_id: "rev-1".to_owned(),
        manuscript_schema_version: 1,
        coordinate_profile: PROSEMIRROR_TOKEN_UTF16_V1.to_owned(),
        from: 0,
        to: 6,
        boundary_profile: EXCLUSIVE_AUTHORITATIVE_EDGES_V1.to_owned(),
        base_slice_digest:
            "sha256:de57203ec93bc120d202c0e6d69ea2f4d5ac9651a5f76cc88ccfb24d6e7ee6c9".to_owned(),
    });
    assert_eq!(
        open_inline_proposal(&multi),
        OpenInlineProposalResult::Applied
    );
    multi.anchors.reverse();
    assert_eq!(
        open_inline_proposal(&multi),
        OpenInlineProposalResult::Conflicted {
            reason: OpenInlineProposalConflict::InvalidAnchor,
        }
    );
}

#[test]
fn routes_interior_edge_and_crossing_input_to_the_owner() {
    let anchors = [(10_u32, 24_u32)];
    assert_eq!(
        classify_inline_input_owner(&anchors, 12, 20),
        InlineInputOwner::Proposal
    );
    assert_eq!(
        classify_inline_input_owner(&anchors, 10, 24),
        InlineInputOwner::Proposal
    );
    assert_eq!(
        classify_inline_input_owner(&anchors, 10, 10),
        InlineInputOwner::Authoritative
    );
    assert_eq!(
        classify_inline_input_owner(&anchors, 24, 24),
        InlineInputOwner::Authoritative
    );
    assert_eq!(
        classify_inline_input_owner(&anchors, 0, 5),
        InlineInputOwner::Authoritative
    );
    assert_eq!(
        classify_inline_input_owner(&anchors, 8, 16),
        InlineInputOwner::Mixed
    );
}
