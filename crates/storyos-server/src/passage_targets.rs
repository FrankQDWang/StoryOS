use super::{ApiError, invalid_request_shape, valid_uuid};

pub(super) fn resolve(
    targets: &[storyos_contracts::PassageTarget],
) -> Result<Vec<storyos_core::PassageContextTarget>, ApiError> {
    let mut chapters = std::collections::BTreeSet::new();
    let mut blocks = std::collections::BTreeSet::new();
    let mut resolved = Vec::new();
    if targets.is_empty() {
        return Err(invalid_request_shape());
    }
    for target in targets {
        valid_uuid(&target.chapter_id)?;
        valid_uuid(&target.base_authoritative_revision_id)?;
        if !chapters.insert(&target.chapter_id) || target.manuscript_block_ids.is_empty() {
            return Err(invalid_request_shape());
        }
        for block in &target.manuscript_block_ids {
            valid_uuid(block)?;
            if !blocks.insert(block) {
                return Err(invalid_request_shape());
            }
        }
        resolved.push(storyos_core::PassageContextTarget {
            chapter_id: target.chapter_id.clone(),
            base_authoritative_revision_id: target.base_authoritative_revision_id.clone(),
            manuscript_block_ids: target.manuscript_block_ids.clone(),
        });
    }
    if blocks.len() > storyos_core::CONTEXT_ITEM_TOKEN_LIMIT as usize + 1 {
        return Err(invalid_request_shape());
    }
    Ok(resolved)
}
