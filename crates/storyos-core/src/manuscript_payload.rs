//! Versioned stable manuscript Blocks for one Chapter payload.

use crate::{
    AuthorEditConflict, AuthorEditNoEffect, AuthorEditPrimitive, AuthorEditRefusal, AuthorEditUnit,
    CurrentOwnershipFacts, InlineInputOwner, TransitionOutcome, UTF16_COORDINATE_PROFILE,
    classify_inline_input_owner, replace_checked_utf16_range, utf16_offset_to_byte,
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

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManuscriptBlockKind {
    Paragraph,
    Heading,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplyVersionedAuthorEdit {
    pub chapter_id: String,
    pub current_authoritative_revision_id: String,
    pub current_payload: ManuscriptPayload,
    pub expected_authoritative_revision_id: String,
    pub expected_proposal_head_revision_ids: Vec<String>,
    pub current_ownership: CurrentOwnershipFacts,
    pub current_target_ownership: VersionedTargetOwnership,
    pub target_refs: Vec<String>,
    pub observed_ownership_partition: String,
    pub author_edit_units: Vec<AuthorEditUnit>,
}

pub type ApplyVersionedAuthorEditOutcome =
    TransitionOutcome<ManuscriptPayload, AuthorEditNoEffect, AuthorEditConflict, AuthorEditRefusal>;

/// Current reservation facts for one versioned edit target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VersionedTargetOwnership {
    Chapter,
    /// An unresolved Inline reservation on one current paragraph Block.
    InlineReservation {
        manuscript_block_id: String,
        proposal_head_revision_id: String,
        reserved_ranges: Vec<(u32, u32)>,
    },
    Block {
        manuscript_block_id: String,
        reservation: BlockReservation,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockReservation {
    Present,
    Absent,
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
) -> ApplyVersionedAuthorEditOutcome {
    if command.expected_authoritative_revision_id != command.current_authoritative_revision_id {
        return TransitionOutcome::Conflicted(AuthorEditConflict::StaleAuthoritativeHead);
    }
    if command.expected_proposal_head_revision_ids
        != command.current_ownership.proposal_head_revision_ids
    {
        return TransitionOutcome::Conflicted(AuthorEditConflict::ProposalHeadPresent);
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
    let owns_target = match &command.current_target_ownership {
        VersionedTargetOwnership::Chapter => current_partition == "authoritative",
        VersionedTargetOwnership::InlineReservation {
            manuscript_block_id,
            proposal_head_revision_id,
            reserved_ranges,
        } => {
            let [unit] = command.author_edit_units.as_slice() else {
                return TransitionOutcome::Conflicted(AuthorEditConflict::OwnershipChanged);
            };
            let [
                AuthorEditPrimitive::ReplaceBlockSelection {
                    manuscript_block_id: target,
                    from,
                    to,
                    ..
                },
            ] = unit.normalized_primitives.as_slice()
            else {
                return TransitionOutcome::Conflicted(AuthorEditConflict::OwnershipChanged);
            };
            let [range] = reserved_ranges.as_slice() else {
                return TransitionOutcome::Conflicted(AuthorEditConflict::OwnershipChanged);
            };
            current_partition == "mixed"
                && command.current_ownership.proposal_head_revision_ids
                    == [proposal_head_revision_id.clone()]
                && target == manuscript_block_id
                && from == to
                && (*from == range.0 || *from == range.1)
                && unit.selection_snapshot.ordered_selection.is_none()
                && unit.selection_snapshot.from == *from
                && unit.selection_snapshot.to == *to
                && classify_inline_input_owner(reserved_ranges, *from, *to)
                    == InlineInputOwner::Authoritative
                && command
                    .current_payload
                    .blocks
                    .iter()
                    .filter(|block| {
                        block.manuscript_block_id == *manuscript_block_id
                            && block.block_kind == ManuscriptBlockKind::Paragraph
                            && range.0 < range.1
                            && utf16_offset_to_byte(&block.text, range.0).is_some()
                            && utf16_offset_to_byte(&block.text, range.1).is_some()
                    })
                    .count()
                    == 1
        }
        VersionedTargetOwnership::Block {
            manuscript_block_id,
            reservation,
        } => {
            let [unit] = command.author_edit_units.as_slice() else {
                return TransitionOutcome::Conflicted(AuthorEditConflict::OwnershipChanged);
            };
            *reservation == BlockReservation::Absent
                && unit.selection_snapshot.ordered_selection.is_none()
                && matches!(unit.normalized_primitives.as_slice(),
                    [AuthorEditPrimitive::ReplaceBlockSelection { manuscript_block_id: target, from, to, .. }]
                    if target == manuscript_block_id && unit.selection_snapshot.from == *from
                        && unit.selection_snapshot.to == *to)
                && command
                    .current_payload
                    .blocks
                    .iter()
                    .filter(|block| {
                        block.manuscript_block_id == *manuscript_block_id
                            && block.block_kind == ManuscriptBlockKind::Paragraph
                    })
                    .count()
                    == 1
        }
    };
    if command.observed_ownership_partition != current_partition || !owns_target {
        return TransitionOutcome::Conflicted(AuthorEditConflict::OwnershipChanged);
    }
    if command.target_refs != [format!("manuscript:{}", command.chapter_id)] {
        return TransitionOutcome::Refused(AuthorEditRefusal::TargetMismatch);
    }
    if !payload_is_supported(&command.current_payload) {
        return TransitionOutcome::Refused(AuthorEditRefusal::UnsupportedIntentShape);
    }
    if command.author_edit_units.is_empty() {
        return TransitionOutcome::Refused(AuthorEditRefusal::UnsupportedIntentShape);
    }
    let mut payload = command.current_payload.clone();
    for unit in &command.author_edit_units {
        if unit.selection_snapshot.coordinate_profile != UTF16_COORDINATE_PROFILE {
            return TransitionOutcome::Refused(AuthorEditRefusal::InvalidSelection);
        }
        if let Err(reason) = apply_unit(&mut payload, unit) {
            return TransitionOutcome::Refused(reason);
        }
    }
    if payload == command.current_payload {
        TransitionOutcome::NoEffect(AuthorEditNoEffect::ContentUnchanged)
    } else {
        TransitionOutcome::Applied(payload)
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
    let replace_count = unit
        .normalized_primitives
        .iter()
        .filter(|primitive| matches!(primitive, AuthorEditPrimitive::ReplaceBlockSelection { .. }))
        .count();
    if replace_count >= 2
        && unit
            .normalized_primitives
            .iter()
            .all(|primitive| matches!(primitive, AuthorEditPrimitive::ReplaceBlockSelection { .. }))
    {
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
            if let Some(snapshot) = snapshot
                && (snapshot.from != *from || snapshot.to != *to)
            {
                return Err(AuthorEditRefusal::InvalidSelection);
            }
            let Some(block) = payload
                .blocks
                .iter_mut()
                .find(|block| block.manuscript_block_id == *manuscript_block_id)
            else {
                return Err(AuthorEditRefusal::InvalidSelection);
            };
            replace_checked_utf16_range(&mut block.text, *from, *to, text)
        }
        AuthorEditPrimitive::SplitBlock {
            manuscript_block_id,
            offset,
            new_manuscript_block_id,
        } => {
            if let Some(snapshot) = snapshot
                && (snapshot.from != *offset || snapshot.to != *offset)
            {
                return Err(AuthorEditRefusal::InvalidSelection);
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
        AuthorEditPrimitive::MoveBlock {
            manuscript_block_id,
            to_index,
        } => move_block(payload, manuscript_block_id, *to_index),
        AuthorEditPrimitive::RetypeBlock {
            manuscript_block_id,
            block_kind,
        } => retype_block(payload, manuscript_block_id, block_kind),
        AuthorEditPrimitive::ReplaceSelection { .. }
        | AuthorEditPrimitive::ReplaceStructuredSelection { .. } => {
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
    if let Some(snapshot) = snapshot
        && (snapshot.from != left_utf16 || snapshot.to != left_utf16)
    {
        return Err(AuthorEditRefusal::InvalidSelection);
    }
    let right_text = payload.blocks[right_index].text.clone();
    payload.blocks[left_index].text.push_str(&right_text);
    payload.blocks.remove(right_index);
    Ok(())
}

fn move_block(
    payload: &mut ManuscriptPayload,
    manuscript_block_id: &str,
    to_index: u32,
) -> Result<(), AuthorEditRefusal> {
    let to_index = to_index as usize;
    let Some(from_index) = payload
        .blocks
        .iter()
        .position(|block| block.manuscript_block_id == manuscript_block_id)
    else {
        return Err(AuthorEditRefusal::InvalidSelection);
    };
    if to_index >= payload.blocks.len() {
        return Err(AuthorEditRefusal::InvalidSelection);
    }
    if from_index == to_index {
        return Ok(());
    }
    let block = payload.blocks.remove(from_index);
    payload.blocks.insert(to_index, block);
    Ok(())
}

fn retype_block(
    payload: &mut ManuscriptPayload,
    manuscript_block_id: &str,
    block_kind: &ManuscriptBlockKind,
) -> Result<(), AuthorEditRefusal> {
    let Some(block) = payload
        .blocks
        .iter_mut()
        .find(|block| block.manuscript_block_id == manuscript_block_id)
    else {
        return Err(AuthorEditRefusal::InvalidSelection);
    };
    block.block_kind = block_kind.clone();
    Ok(())
}
