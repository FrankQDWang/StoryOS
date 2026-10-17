use super::{InlineAnchorProofFailure, ReplanAnchorFacts, prove_replanned_inline_anchors};
use crate::{
    EXCLUSIVE_AUTHORITATIVE_EDGES_V1, MANUSCRIPT_SCHEMA_VERSION, ManuscriptBlock,
    ManuscriptBlockKind, OpenInlineProposalAnchor, PROSEMIRROR_TOKEN_UTF16_V1,
    proposal_anchor_base_slice_digest,
};

const BLOCK: &str = "block-1";

fn block(text: &str) -> Option<ManuscriptBlock> {
    Some(ManuscriptBlock {
        manuscript_block_id: BLOCK.to_owned(),
        block_kind: ManuscriptBlockKind::Paragraph,
        text: text.to_owned(),
    })
}

fn anchor(revision: &str, from: u32, to: u32, slice: &str) -> OpenInlineProposalAnchor {
    OpenInlineProposalAnchor {
        manuscript_block_id: BLOCK.to_owned(),
        base_authoritative_revision_id: revision.to_owned(),
        manuscript_schema_version: MANUSCRIPT_SCHEMA_VERSION,
        coordinate_profile: PROSEMIRROR_TOKEN_UTF16_V1.to_owned(),
        from,
        to,
        boundary_profile: EXCLUSIVE_AUTHORITATIVE_EDGES_V1.to_owned(),
        base_slice_digest: proposal_anchor_base_slice_digest(
            BLOCK,
            "paragraph",
            MANUSCRIPT_SCHEMA_VERSION,
            PROSEMIRROR_TOKEN_UTF16_V1,
            from,
            to,
            slice,
        ),
    }
}

fn facts(current: Option<ManuscriptBlock>) -> ReplanAnchorFacts {
    ReplanAnchorFacts {
        anchor: anchor("h0", /*from*/ 4, /*to*/ 7, "cat"),
        base_block: block("The cat sat."),
        current_block: current,
        other_reservations: Vec::new(),
    }
}

fn prove(
    facts: ReplanAnchorFacts,
) -> Result<Vec<OpenInlineProposalAnchor>, InlineAnchorProofFailure> {
    prove_replanned_inline_anchors("h1", &[facts])
}

#[test]
fn an_edge_insertion_gives_a_new_anchor_on_the_current_revision() {
    assert_eq!(
        prove(facts(block("The cats sat."))),
        Ok(vec![anchor("h1", 4, 7, "cat")])
    );
    assert_eq!(
        prove(facts(block("The big cat sat."))),
        Ok(vec![anchor("h1", 8, 11, "cat")])
    );
}

#[test]
fn a_changed_block_or_range_refuses() {
    assert_eq!(
        prove(facts(None)),
        Err(InlineAnchorProofFailure::BlockChanged)
    );
    let mut heading = facts(block("The cat sat."));
    if let Some(current) = heading.current_block.as_mut() {
        current.block_kind = ManuscriptBlockKind::Heading;
    }
    assert_eq!(prove(heading), Err(InlineAnchorProofFailure::BlockChanged));
    let mut other_block = facts(block("The cat sat."));
    if let Some(current) = other_block.current_block.as_mut() {
        current.manuscript_block_id = "block-2".to_owned();
    }
    assert_eq!(
        prove(other_block),
        Err(InlineAnchorProofFailure::BlockChanged)
    );
    let mut profile = facts(block("The cat sat."));
    profile.anchor.coordinate_profile = "other".to_owned();
    assert_eq!(prove(profile), Err(InlineAnchorProofFailure::BlockChanged));
    assert_eq!(
        prove(facts(block("The cot sat."))),
        Err(InlineAnchorProofFailure::RangeChanged)
    );
}

#[test]
fn a_base_slice_that_does_not_match_the_digest_refuses() {
    let mut mismatch = facts(block("The cat sat."));
    mismatch.base_block = block("The cot sat.");
    assert_eq!(
        prove(mismatch),
        Err(InlineAnchorProofFailure::BaseSliceMismatch)
    );
}

#[test]
fn a_coarse_comparison_refuses() {
    let coarse = ReplanAnchorFacts {
        anchor: anchor(
            "h0",
            /*from*/ 101,
            /*to*/ 111,
            &"a".repeat(/*n*/ 10),
        ),
        base_block: block(&format!("<{}>", "a".repeat(500))),
        current_block: block(&format!("<{}>", "ab".repeat(500))),
        other_reservations: Vec::new(),
    };
    assert_eq!(
        prove(coarse),
        Err(InlineAnchorProofFailure::CoarseComparison)
    );
}

#[test]
fn another_reservation_on_the_new_range_refuses() {
    let mut reserved = facts(block("The cat sat."));
    reserved.other_reservations = vec![(6, 9)];
    assert_eq!(prove(reserved), Err(InlineAnchorProofFailure::Reserved));
    let mut adjacent = facts(block("The cat sat."));
    adjacent.other_reservations = vec![(0, 4), (7, 11)];
    assert_eq!(prove(adjacent), Ok(vec![anchor("h1", 4, 7, "cat")]));
}

#[test]
fn an_empty_anchor_set_refuses() {
    assert_eq!(
        prove_replanned_inline_anchors("h1", &[]),
        Err(InlineAnchorProofFailure::NoAnchor)
    );
}
