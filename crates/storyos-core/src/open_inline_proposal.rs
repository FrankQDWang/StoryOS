//! Classify one InlineEditProposal open against exact Block-relative Anchors.

use crate::{canonical_json, hex_sha256, utf16_offset_to_byte};

pub const PROSEMIRROR_TOKEN_UTF16_V1: &str = "prosemirror-token-utf16.v1";
pub const EXCLUSIVE_AUTHORITATIVE_EDGES_V1: &str = "exclusive-authoritative-edges.v1";
pub const PROPOSAL_ANCHOR_BASE_SLICE_PROFILE: &str = "storyos.proposal-anchor-base-slice.jcs.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InlineTargetBlock {
    pub manuscript_block_id: String,
    pub block_kind: String,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenInlineProposalAnchor {
    pub manuscript_block_id: String,
    pub base_authoritative_revision_id: String,
    pub manuscript_schema_version: u32,
    pub coordinate_profile: String,
    pub from: u32,
    pub to: u32,
    pub boundary_profile: String,
    pub base_slice_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenInlineProposal {
    pub scope_matches: bool,
    pub target_block_present: bool,
    pub expected_base_revision_id: String,
    pub current_base_revision_id: Option<String>,
    pub conflicting_reservation: bool,
    pub current_schema_version: u32,
    pub current_coordinate_profile: String,
    pub blocks: Vec<InlineTargetBlock>,
    pub anchors: Vec<OpenInlineProposalAnchor>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpenInlineProposalResult {
    Applied,
    Refused { reason: OpenInlineProposalRefusal },
    Conflicted { reason: OpenInlineProposalConflict },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpenInlineProposalRefusal {
    WrongScope,
    UnavailableTarget,
    UnsupportedAnchorContract,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpenInlineProposalConflict {
    ChangedHead,
    ConflictingReservation,
    InvalidAnchor,
    BaseSliceMismatch,
    CoordinateInterpretationChanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InlineInputOwner {
    Proposal,
    Authoritative,
    Mixed,
}

/// Digest one structural Anchor base slice under the adopted JCS profile.
pub fn proposal_anchor_base_slice_digest(
    manuscript_block_id: &str,
    block_kind: &str,
    manuscript_schema_version: u32,
    coordinate_profile: &str,
    from: u32,
    to: u32,
    base_slice: &str,
) -> String {
    format!(
        "sha256:{}",
        hex_sha256(
            canonical_json(&serde_json::json!({
                "base_slice": base_slice,
                "block_kind": block_kind,
                "coordinate_profile": coordinate_profile,
                "from": from,
                "manuscript_block_id": manuscript_block_id,
                "manuscript_schema_version": manuscript_schema_version,
                "to": to,
            }))
            .as_bytes(),
        )
    )
}

/// Classify one InlineEditProposal against Scope, Head, reservation, and exact Anchors.
pub fn open_inline_proposal(command: &OpenInlineProposal) -> OpenInlineProposalResult {
    if !command.scope_matches {
        return OpenInlineProposalResult::Refused {
            reason: OpenInlineProposalRefusal::WrongScope,
        };
    }
    if !command.target_block_present {
        return OpenInlineProposalResult::Refused {
            reason: OpenInlineProposalRefusal::UnavailableTarget,
        };
    }
    match command.current_base_revision_id.as_deref() {
        Some(current) if current == command.expected_base_revision_id => {}
        Some(_) => {
            return OpenInlineProposalResult::Conflicted {
                reason: OpenInlineProposalConflict::ChangedHead,
            };
        }
        None => {
            return OpenInlineProposalResult::Refused {
                reason: OpenInlineProposalRefusal::UnavailableTarget,
            };
        }
    }
    if command.conflicting_reservation {
        return OpenInlineProposalResult::Conflicted {
            reason: OpenInlineProposalConflict::ConflictingReservation,
        };
    }
    if command.anchors.is_empty() {
        return OpenInlineProposalResult::Conflicted {
            reason: OpenInlineProposalConflict::InvalidAnchor,
        };
    }
    if command.current_coordinate_profile != PROSEMIRROR_TOKEN_UTF16_V1
        || command.current_schema_version != 1
    {
        return OpenInlineProposalResult::Conflicted {
            reason: OpenInlineProposalConflict::CoordinateInterpretationChanged,
        };
    }
    let mut previous: Option<(&str, u32)> = None;
    for anchor in &command.anchors {
        if anchor.coordinate_profile != PROSEMIRROR_TOKEN_UTF16_V1
            || anchor.boundary_profile != EXCLUSIVE_AUTHORITATIVE_EDGES_V1
        {
            return OpenInlineProposalResult::Refused {
                reason: OpenInlineProposalRefusal::UnsupportedAnchorContract,
            };
        }
        if anchor.manuscript_schema_version != command.current_schema_version
            || anchor.base_authoritative_revision_id != command.expected_base_revision_id
        {
            return OpenInlineProposalResult::Conflicted {
                reason: OpenInlineProposalConflict::CoordinateInterpretationChanged,
            };
        }
        let Some(block) = command
            .blocks
            .iter()
            .find(|block| block.manuscript_block_id == anchor.manuscript_block_id)
        else {
            return OpenInlineProposalResult::Conflicted {
                reason: OpenInlineProposalConflict::InvalidAnchor,
            };
        };
        if let Some((prior_block, prior_from)) = previous {
            let out_of_order = if prior_block == anchor.manuscript_block_id.as_str() {
                anchor.from <= prior_from
            } else {
                command
                    .blocks
                    .iter()
                    .position(|block| block.manuscript_block_id == prior_block)
                    >= command
                        .blocks
                        .iter()
                        .position(|block| block.manuscript_block_id == anchor.manuscript_block_id)
            };
            if out_of_order {
                return OpenInlineProposalResult::Conflicted {
                    reason: OpenInlineProposalConflict::InvalidAnchor,
                };
            }
        }
        previous = Some((&anchor.manuscript_block_id, anchor.from));
        if anchor.from >= anchor.to {
            return OpenInlineProposalResult::Conflicted {
                reason: OpenInlineProposalConflict::InvalidAnchor,
            };
        }
        let Some(from_byte) = utf16_offset_to_byte(&block.text, anchor.from) else {
            return OpenInlineProposalResult::Conflicted {
                reason: OpenInlineProposalConflict::InvalidAnchor,
            };
        };
        let Some(to_byte) = utf16_offset_to_byte(&block.text, anchor.to) else {
            return OpenInlineProposalResult::Conflicted {
                reason: OpenInlineProposalConflict::InvalidAnchor,
            };
        };
        let proven = proposal_anchor_base_slice_digest(
            &anchor.manuscript_block_id,
            &block.block_kind,
            anchor.manuscript_schema_version,
            &anchor.coordinate_profile,
            anchor.from,
            anchor.to,
            &block.text[from_byte..to_byte],
        );
        if proven != anchor.base_slice_digest {
            return OpenInlineProposalResult::Conflicted {
                reason: OpenInlineProposalConflict::BaseSliceMismatch,
            };
        }
    }
    OpenInlineProposalResult::Applied
}

/// Route one Block-relative input against exclusive-authoritative edges.
pub fn classify_inline_input_owner(anchors: &[(u32, u32)], from: u32, to: u32) -> InlineInputOwner {
    if from > to || anchors.is_empty() {
        return InlineInputOwner::Mixed;
    }
    if from == to
        && anchors
            .iter()
            .any(|(start, end)| from == *start || from == *end)
    {
        return InlineInputOwner::Authoritative;
    }
    if anchors
        .iter()
        .all(|(start, end)| to <= *start || from >= *end)
    {
        return InlineInputOwner::Authoritative;
    }
    if anchors
        .iter()
        .any(|(start, end)| from >= *start && to <= *end)
    {
        return InlineInputOwner::Proposal;
    }
    InlineInputOwner::Mixed
}

impl OpenInlineProposalResult {
    /// Core-issued Validation Receipt result for one successful open.
    pub fn validation_receipt_result(&self) -> Option<&'static str> {
        match self {
            Self::Applied => Some("valid"),
            Self::Refused { .. } | Self::Conflicted { .. } => None,
        }
    }
}

#[cfg(test)]
#[path = "open_inline_proposal_tests.rs"]
mod tests;
