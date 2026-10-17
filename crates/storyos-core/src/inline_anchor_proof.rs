use crate::{
    EXCLUSIVE_AUTHORITATIVE_EDGES_V1, MANUSCRIPT_SCHEMA_VERSION, ManuscriptBlock,
    ManuscriptBlockKind, OpenInlineProposalAnchor, PROSEMIRROR_TOKEN_UTF16_V1, RangeMapping,
    map_unchanged_range, proposal_anchor_base_slice_digest, utf16_slice,
};

/// The facts of one stored Anchor at an explicit Replan of an Inline Proposal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplanAnchorFacts {
    pub anchor: OpenInlineProposalAnchor,
    /// The Anchor Block on the base Authoritative Revision of the Anchor.
    pub base_block: Option<ManuscriptBlock>,
    /// The Anchor Block on the current Authoritative Revision.
    pub current_block: Option<ManuscriptBlock>,
    /// The ranges of the other unresolved ownership reservations on the current Block.
    pub other_reservations: Vec<(u32, u32)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InlineAnchorProofFailure {
    NoAnchor,
    BlockChanged,
    BaseSliceMismatch,
    RangeChanged,
    CoarseComparison,
    Reserved,
}

/// Prove each stored Anchor on the current Authoritative Revision, and give the new Anchor set.
pub fn prove_replanned_inline_anchors(
    current_revision_id: &str,
    anchors: &[ReplanAnchorFacts],
) -> Result<Vec<OpenInlineProposalAnchor>, InlineAnchorProofFailure> {
    if anchors.is_empty() {
        return Err(InlineAnchorProofFailure::NoAnchor);
    }
    anchors
        .iter()
        .map(|facts| prove_anchor(current_revision_id, facts))
        .collect()
}

fn prove_anchor(
    current_revision_id: &str,
    facts: &ReplanAnchorFacts,
) -> Result<OpenInlineProposalAnchor, InlineAnchorProofFailure> {
    let anchor = &facts.anchor;
    let (Some(base), Some(current)) = (
        paragraph(&facts.base_block, anchor),
        paragraph(&facts.current_block, anchor),
    ) else {
        return Err(InlineAnchorProofFailure::BlockChanged);
    };
    if anchor.manuscript_schema_version != MANUSCRIPT_SCHEMA_VERSION
        || anchor.coordinate_profile != PROSEMIRROR_TOKEN_UTF16_V1
        || anchor.boundary_profile != EXCLUSIVE_AUTHORITATIVE_EDGES_V1
    {
        return Err(InlineAnchorProofFailure::BlockChanged);
    }
    let digest = |from: u32, to: u32, slice: &str| {
        proposal_anchor_base_slice_digest(
            &anchor.manuscript_block_id,
            "paragraph",
            anchor.manuscript_schema_version,
            &anchor.coordinate_profile,
            from,
            to,
            slice,
        )
    };
    let base_slice = utf16_slice(&base.text, anchor.from, anchor.to)
        .filter(|slice| digest(anchor.from, anchor.to, slice) == anchor.base_slice_digest)
        .ok_or(InlineAnchorProofFailure::BaseSliceMismatch)?;
    let (from, to) = match map_unchanged_range(&base.text, &current.text, anchor.from, anchor.to) {
        RangeMapping::Unchanged { from, to } => (from, to),
        RangeMapping::Changed => return Err(InlineAnchorProofFailure::RangeChanged),
        RangeMapping::Coarse => return Err(InlineAnchorProofFailure::CoarseComparison),
    };
    if utf16_slice(&current.text, from, to) != Some(base_slice) {
        return Err(InlineAnchorProofFailure::RangeChanged);
    }
    if facts
        .other_reservations
        .iter()
        .any(|&(reserved_from, reserved_to)| reserved_from < to && from < reserved_to)
    {
        return Err(InlineAnchorProofFailure::Reserved);
    }
    Ok(OpenInlineProposalAnchor {
        manuscript_block_id: anchor.manuscript_block_id.clone(),
        base_authoritative_revision_id: current_revision_id.to_owned(),
        manuscript_schema_version: anchor.manuscript_schema_version,
        coordinate_profile: anchor.coordinate_profile.clone(),
        from,
        to,
        boundary_profile: anchor.boundary_profile.clone(),
        base_slice_digest: digest(from, to, base_slice),
    })
}

/// The Block when it is the Anchor paragraph with the same identity.
fn paragraph<'block>(
    block: &'block Option<ManuscriptBlock>,
    anchor: &OpenInlineProposalAnchor,
) -> Option<&'block ManuscriptBlock> {
    block.as_ref().filter(|block| {
        block.manuscript_block_id == anchor.manuscript_block_id
            && block.block_kind == ManuscriptBlockKind::Paragraph
    })
}

#[cfg(test)]
#[path = "inline_anchor_proof_tests.rs"]
mod tests;
