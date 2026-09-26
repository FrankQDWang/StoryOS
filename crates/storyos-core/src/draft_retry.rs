use crate::{AuthorEditPrimitive, RefusedEditPayload, ReplacementBlock, utf16_offset_to_byte};

/// Selects contiguous replacement blocks at exact Unicode scalar boundaries.
pub fn select_draft_replacement(
    payload: &RefusedEditPayload,
    from: (u32, u32),
    to: (u32, u32),
) -> Option<Vec<ReplacementBlock>> {
    let [unit] = payload.author_edit_units.as_slice() else {
        return None;
    };
    let [AuthorEditPrimitive::ReplaceStructuredSelection { replacement }] =
        unit.normalized_primitives.as_slice()
    else {
        return None;
    };
    if from > to {
        return None;
    }
    let blocks = replacement.get(from.0 as usize..=to.0 as usize)?;
    let mut selected = Vec::with_capacity(blocks.len());
    for (index, block) in blocks.iter().enumerate() {
        let start = if index == 0 { from.1 } else { 0 };
        let end = if index + 1 == blocks.len() {
            to.1
        } else {
            block.text.encode_utf16().count().try_into().ok()?
        };
        let start = utf16_offset_to_byte(&block.text, start)?;
        let end = utf16_offset_to_byte(&block.text, end)?;
        selected.push(ReplacementBlock {
            block_kind: block.block_kind.clone(),
            text: block.text.get(start..end)?.to_owned(),
        });
    }
    Some(selected)
}
