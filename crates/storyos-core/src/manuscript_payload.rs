//! Versioned stable manuscript Blocks for one Chapter payload.

use crate::{
    AuthorEditConflict, AuthorEditNoEffect, AuthorEditPrimitive, AuthorEditRefusal, AuthorEditUnit,
    CurrentOwnershipFacts, UTF16_COORDINATE_PROFILE, utf16_offset_to_byte,
};

pub const MANUSCRIPT_SCHEMA_VERSION: u32 = 1;
pub const COORDINATE_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManuscriptPayload {
    pub schema_version: u32,
    pub coordinate_version: u32,
    pub blocks: Vec<ManuscriptBlock>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManuscriptBlock {
    pub manuscript_block_id: String,
    pub block_kind: ManuscriptBlockKind,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManuscriptBlockKind {
    Paragraph,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplyVersionedAuthorEdit {
    pub chapter_id: String,
    pub current_authoritative_revision_id: String,
    pub current_payload: ManuscriptPayload,
    pub expected_authoritative_revision_id: String,
    pub expected_proposal_head_revision_ids: Vec<String>,
    pub current_ownership: CurrentOwnershipFacts,
    pub target_refs: Vec<String>,
    pub observed_ownership_partition: String,
    pub author_edit_units: Vec<AuthorEditUnit>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplyVersionedAuthorEditResult {
    AuthoritativeApplied { payload: ManuscriptPayload },
    Conflicted { reason: AuthorEditConflict },
    NoEffect { reason: AuthorEditNoEffect },
    Refused { reason: AuthorEditRefusal },
}

/// Wrap one legacy UTF-8 Chapter body as one stable paragraph Block.
pub fn upgrade_legacy_manuscript(text: &str, manuscript_block_id: &str) -> ManuscriptPayload {
    ManuscriptPayload {
        schema_version: MANUSCRIPT_SCHEMA_VERSION,
        coordinate_version: COORDINATE_VERSION,
        blocks: vec![ManuscriptBlock {
            manuscript_block_id: manuscript_block_id.to_owned(),
            block_kind: ManuscriptBlockKind::Paragraph,
            text: text.to_owned(),
        }],
    }
}

pub fn apply_versioned_author_edit(
    command: &ApplyVersionedAuthorEdit,
) -> ApplyVersionedAuthorEditResult {
    if command.expected_authoritative_revision_id != command.current_authoritative_revision_id {
        return ApplyVersionedAuthorEditResult::Conflicted {
            reason: AuthorEditConflict::StaleAuthoritativeHead,
        };
    }
    if command.expected_proposal_head_revision_ids
        != command.current_ownership.proposal_head_revision_ids
    {
        return ApplyVersionedAuthorEditResult::Conflicted {
            reason: AuthorEditConflict::ProposalHeadPresent,
        };
    }
    let current_partition = if command
        .current_ownership
        .proposal_head_revision_ids
        .is_empty()
        && command.current_ownership.anchor_refs.is_empty()
        && command
            .current_ownership
            .unresolved_reservation_refs
            .is_empty()
    {
        "authoritative"
    } else {
        "mixed"
    };
    if command.observed_ownership_partition != current_partition
        || current_partition != "authoritative"
    {
        return ApplyVersionedAuthorEditResult::Conflicted {
            reason: AuthorEditConflict::OwnershipChanged,
        };
    }
    if command.target_refs != [format!("manuscript:{}", command.chapter_id)] {
        return ApplyVersionedAuthorEditResult::Refused {
            reason: AuthorEditRefusal::TargetMismatch,
        };
    }
    if command.current_payload.schema_version != MANUSCRIPT_SCHEMA_VERSION
        || command.current_payload.coordinate_version != COORDINATE_VERSION
        || command.current_payload.blocks.len() != 1
        || command.current_payload.blocks[0].block_kind != ManuscriptBlockKind::Paragraph
    {
        return ApplyVersionedAuthorEditResult::Refused {
            reason: AuthorEditRefusal::UnsupportedIntentShape,
        };
    }
    if command.author_edit_units.is_empty() {
        return ApplyVersionedAuthorEditResult::Refused {
            reason: AuthorEditRefusal::UnsupportedIntentShape,
        };
    }
    let mut payload = command.current_payload.clone();
    for unit in &command.author_edit_units {
        let [
            AuthorEditPrimitive::ReplaceBlockSelection {
                manuscript_block_id,
                from,
                to,
                text,
            },
        ] = unit.normalized_primitives.as_slice()
        else {
            return ApplyVersionedAuthorEditResult::Refused {
                reason: AuthorEditRefusal::UnsupportedIntentShape,
            };
        };
        if unit.selection_snapshot.coordinate_profile != UTF16_COORDINATE_PROFILE
            || unit.selection_snapshot.from != *from
            || unit.selection_snapshot.to != *to
        {
            return ApplyVersionedAuthorEditResult::Refused {
                reason: AuthorEditRefusal::InvalidSelection,
            };
        }
        let Some(block) = payload
            .blocks
            .iter_mut()
            .find(|block| block.manuscript_block_id == *manuscript_block_id)
        else {
            return ApplyVersionedAuthorEditResult::Refused {
                reason: AuthorEditRefusal::InvalidSelection,
            };
        };
        if let Err(reason) = replace_block_text(block, *from, *to, text) {
            return ApplyVersionedAuthorEditResult::Refused { reason };
        }
    }
    if payload == command.current_payload {
        ApplyVersionedAuthorEditResult::NoEffect {
            reason: AuthorEditNoEffect::ContentUnchanged,
        }
    } else {
        ApplyVersionedAuthorEditResult::AuthoritativeApplied { payload }
    }
}

fn replace_block_text(
    block: &mut ManuscriptBlock,
    from: u32,
    to: u32,
    text: &str,
) -> Result<(), AuthorEditRefusal> {
    let Some(from_byte) = utf16_offset_to_byte(&block.text, from) else {
        return Err(AuthorEditRefusal::InvalidSelection);
    };
    let to_byte = if from == to {
        from_byte
    } else if let Some(to_byte) = utf16_offset_to_byte(&block.text, to) {
        to_byte
    } else {
        return Err(AuthorEditRefusal::InvalidSelection);
    };
    if from_byte > to_byte {
        return Err(AuthorEditRefusal::InvalidSelection);
    }
    block.text = format!(
        "{}{text}{}",
        &block.text[..from_byte],
        &block.text[to_byte..]
    );
    Ok(())
}
