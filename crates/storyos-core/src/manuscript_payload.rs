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
    if !payload_is_supported(&command.current_payload) {
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
        if unit.selection_snapshot.coordinate_profile != UTF16_COORDINATE_PROFILE {
            return ApplyVersionedAuthorEditResult::Refused {
                reason: AuthorEditRefusal::InvalidSelection,
            };
        }
        if let Err(reason) = apply_unit(&mut payload, unit) {
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

/// Flatten current Block texts for the Chapter body wire field.
pub fn chapter_display_body(blocks: &[ManuscriptBlock]) -> String {
    blocks
        .iter()
        .map(|block| block.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn payload_is_supported(payload: &ManuscriptPayload) -> bool {
    if payload.schema_version != MANUSCRIPT_SCHEMA_VERSION
        || payload.coordinate_version != COORDINATE_VERSION
        || payload.blocks.is_empty()
        || payload
            .blocks
            .iter()
            .any(|block| block.block_kind != ManuscriptBlockKind::Paragraph)
    {
        return false;
    }
    let mut ids: Vec<&str> = payload
        .blocks
        .iter()
        .map(|block| block.manuscript_block_id.as_str())
        .collect();
    ids.sort_unstable();
    ids.windows(2).all(|pair| pair[0] != pair[1])
}

fn apply_unit(
    payload: &mut ManuscriptPayload,
    unit: &AuthorEditUnit,
) -> Result<(), AuthorEditRefusal> {
    if unit.normalized_primitives.is_empty() {
        return Err(AuthorEditRefusal::UnsupportedIntentShape);
    }
    if unit.normalized_primitives.len() > 1
        && unit.selection_snapshot.from > unit.selection_snapshot.to
    {
        return Err(AuthorEditRefusal::InvalidSelection);
    }
    for primitive in &unit.normalized_primitives {
        let snapshot = if unit.normalized_primitives.len() == 1 {
            Some(&unit.selection_snapshot)
        } else {
            None
        };
        apply_primitive(payload, primitive, snapshot)?;
    }
    Ok(())
}

fn apply_primitive(
    payload: &mut ManuscriptPayload,
    primitive: &AuthorEditPrimitive,
    snapshot: Option<&crate::SelectionSnapshot>,
) -> Result<(), AuthorEditRefusal> {
    match primitive {
        AuthorEditPrimitive::ReplaceBlockSelection {
            manuscript_block_id,
            from,
            to,
            text,
        } => {
            if let Some(snapshot) = snapshot {
                if snapshot.from != *from || snapshot.to != *to {
                    return Err(AuthorEditRefusal::InvalidSelection);
                }
            }
            let Some(block) = payload
                .blocks
                .iter_mut()
                .find(|block| block.manuscript_block_id == *manuscript_block_id)
            else {
                return Err(AuthorEditRefusal::InvalidSelection);
            };
            replace_block_text(block, *from, *to, text)
        }
        AuthorEditPrimitive::SplitBlock {
            manuscript_block_id,
            offset,
            new_manuscript_block_id,
        } => {
            if let Some(snapshot) = snapshot {
                if snapshot.from != *offset || snapshot.to != *offset {
                    return Err(AuthorEditRefusal::InvalidSelection);
                }
            }
            split_block(
                payload,
                manuscript_block_id,
                *offset,
                new_manuscript_block_id,
            )
        }
        AuthorEditPrimitive::JoinBlocks {
            left_manuscript_block_id,
            right_manuscript_block_id,
        } => join_blocks(
            payload,
            left_manuscript_block_id,
            right_manuscript_block_id,
            snapshot,
        ),
        AuthorEditPrimitive::ReplaceSelection { .. } => {
            Err(AuthorEditRefusal::UnsupportedIntentShape)
        }
    }
}

fn split_block(
    payload: &mut ManuscriptPayload,
    manuscript_block_id: &str,
    offset: u32,
    new_manuscript_block_id: &str,
) -> Result<(), AuthorEditRefusal> {
    if new_manuscript_block_id == manuscript_block_id
        || payload
            .blocks
            .iter()
            .any(|block| block.manuscript_block_id == new_manuscript_block_id)
    {
        return Err(AuthorEditRefusal::InvalidSelection);
    }
    let Some(index) = payload
        .blocks
        .iter()
        .position(|block| block.manuscript_block_id == manuscript_block_id)
    else {
        return Err(AuthorEditRefusal::InvalidSelection);
    };
    let Some(byte) = utf16_offset_to_byte(&payload.blocks[index].text, offset) else {
        return Err(AuthorEditRefusal::InvalidSelection);
    };
    let right_text = payload.blocks[index].text[byte..].to_owned();
    payload.blocks[index].text.truncate(byte);
    payload.blocks.insert(
        index + 1,
        ManuscriptBlock {
            manuscript_block_id: new_manuscript_block_id.to_owned(),
            block_kind: ManuscriptBlockKind::Paragraph,
            text: right_text,
        },
    );
    Ok(())
}

fn join_blocks(
    payload: &mut ManuscriptPayload,
    left_manuscript_block_id: &str,
    right_manuscript_block_id: &str,
    snapshot: Option<&crate::SelectionSnapshot>,
) -> Result<(), AuthorEditRefusal> {
    let Some(left_index) = payload
        .blocks
        .iter()
        .position(|block| block.manuscript_block_id == left_manuscript_block_id)
    else {
        return Err(AuthorEditRefusal::InvalidSelection);
    };
    let Some(right_index) = payload
        .blocks
        .iter()
        .position(|block| block.manuscript_block_id == right_manuscript_block_id)
    else {
        return Err(AuthorEditRefusal::InvalidSelection);
    };
    if right_index != left_index + 1 {
        return Err(AuthorEditRefusal::InvalidSelection);
    }
    let left_utf16 = payload.blocks[left_index].text.encode_utf16().count() as u32;
    if let Some(snapshot) = snapshot {
        if snapshot.from != left_utf16 || snapshot.to != left_utf16 {
            return Err(AuthorEditRefusal::InvalidSelection);
        }
    }
    let right_text = payload.blocks[right_index].text.clone();
    payload.blocks[left_index].text.push_str(&right_text);
    payload.blocks.remove(right_index);
    Ok(())
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
