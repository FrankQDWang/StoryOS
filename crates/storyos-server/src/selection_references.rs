use super::{ApiError, invalid_request_shape, valid_uuid};
use storyos_contracts::{SelectionCoordinateProfile, SelectionPoint, SelectionReference};

/// The core references of a request, after the shape checks that need no Project data. An
/// empty or reversed range goes on to the Context Assembly, which blocks it (ADR 0062).
pub(super) fn resolve(
    references: &[SelectionReference],
) -> Result<Vec<storyos_core::SelectionReference>, ApiError> {
    if references.is_empty() || references.len() > storyos_core::SELECTION_REFERENCE_LIMIT {
        return Err(invalid_request_shape());
    }
    let mut resolved = Vec::with_capacity(references.len());
    for reference in references {
        let core = match reference {
            SelectionReference::ChapterText {
                chapter_id,
                base_authoritative_revision_id,
                coordinate_profile: SelectionCoordinateProfile::Utf16CodeUnitV1,
                start,
                end,
            } => {
                for id in [
                    chapter_id,
                    base_authoritative_revision_id,
                    &start.manuscript_block_id,
                    &end.manuscript_block_id,
                ] {
                    valid_uuid(id)?;
                }
                storyos_core::SelectionReference::ChapterText {
                    chapter_id: chapter_id.clone(),
                    base_authoritative_revision_id: base_authoritative_revision_id.clone(),
                    start: core_point(start),
                    end: core_point(end),
                }
            }
        };
        if resolved.contains(&core) {
            return Err(invalid_request_shape());
        }
        resolved.push(core);
    }
    Ok(resolved)
}

/// The public form of one admitted reference.
pub(super) fn inspect(reference: &storyos_core::SelectionReference) -> SelectionReference {
    match reference {
        storyos_core::SelectionReference::ChapterText {
            chapter_id,
            base_authoritative_revision_id,
            start,
            end,
        } => SelectionReference::ChapterText {
            chapter_id: chapter_id.clone(),
            base_authoritative_revision_id: base_authoritative_revision_id.clone(),
            coordinate_profile: SelectionCoordinateProfile::Utf16CodeUnitV1,
            start: public_point(start),
            end: public_point(end),
        },
    }
}

fn core_point(point: &SelectionPoint) -> storyos_core::TextPoint {
    storyos_core::TextPoint {
        manuscript_block_id: point.manuscript_block_id.clone(),
        offset: point.offset,
    }
}

fn public_point(point: &storyos_core::TextPoint) -> SelectionPoint {
    SelectionPoint {
        manuscript_block_id: point.manuscript_block_id.clone(),
        offset: point.offset,
    }
}
