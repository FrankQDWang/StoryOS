//! The exact text ranges that the author adds to one assistant message (ADR 0062).

use serde::{Deserialize, Serialize};

/// The most references that one assistant message can hold.
pub const SELECTION_REFERENCE_LIMIT: usize = 8;

/// One block and one UTF-16 code unit offset in its text.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TextPoint {
    pub manuscript_block_id: String,
    pub offset: u32,
}

/// One exact text range of a Chapter Revision or of a pending Proposal candidate.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SelectionReference {
    ChapterText {
        chapter_id: String,
        base_authoritative_revision_id: String,
        start: TextPoint,
        end: TextPoint,
    },
    ProposalCandidateText {
        chapter_id: String,
        proposal_id: String,
        operation_id: String,
        revision_id: String,
        start_offset: u32,
        end_offset: u32,
    },
}

impl SelectionReference {
    pub fn chapter_id(&self) -> &str {
        match self {
            Self::ChapterText { chapter_id, .. }
            | Self::ProposalCandidateText { chapter_id, .. } => chapter_id,
        }
    }

    /// The pending candidate that a candidate reference names.
    pub fn candidate_target(&self) -> Option<super::ProposalCandidateTarget> {
        match self {
            Self::ChapterText { .. } => None,
            Self::ProposalCandidateText {
                proposal_id,
                operation_id,
                revision_id,
                ..
            } => Some(super::ProposalCandidateTarget {
                proposal_id: proposal_id.clone(),
                operation_id: operation_id.clone(),
                revision_id: revision_id.clone(),
            }),
        }
    }
}

/// One reference with the exact text and the blocks that the assembly read from its base.
/// An unavailable reference has no text and no blocks.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdmittedSelection {
    pub reference: SelectionReference,
    pub selected_text: Option<String>,
    pub manuscript_block_ids: Vec<String>,
    /// All of the candidate text of an available candidate reference. The Operations of one
    /// Proposal share a revision, so this text identifies the candidate of one Operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_text: Option<String>,
}

impl AdmittedSelection {
    /// The pending candidate and all of its text, when this reference is an available candidate.
    pub fn candidate(&self) -> Option<(super::ProposalCandidateTarget, &str)> {
        Some((
            self.reference.candidate_target()?,
            self.candidate_text.as_deref()?,
        ))
    }
}

/// Move the candidate references of a revised Proposal to its new revision. Each Operation of
/// the Proposal shares the revision. The revised candidate gets a new text, so its reference
/// covers all of that text. The ranges of the other Operations stay.
pub fn advance_candidate_references<'a>(
    references: impl IntoIterator<Item = &'a mut AdmittedSelection>,
    revised: &super::ProposalCandidateTarget,
    new_revision_id: &str,
    new_text: &str,
) {
    for admitted in references {
        if let SelectionReference::ProposalCandidateText {
            proposal_id,
            operation_id,
            revision_id,
            start_offset,
            end_offset,
            ..
        } = &mut admitted.reference
            && *proposal_id == revised.proposal_id
            && *revision_id == revised.revision_id
        {
            *revision_id = new_revision_id.to_owned();
            if *operation_id == revised.operation_id {
                *start_offset = 0;
                *end_offset = u32::try_from(new_text.encode_utf16().count()).unwrap_or(u32::MAX);
            }
        }
    }
}

/// Complete a passage collection record with the referenced candidate texts and the admitted
/// references. An unavailable reference blocks the assembly.
pub fn assemble_selection_references(
    mut record: super::CurrentPassageAssemblyRecord,
    source: &super::CurrentPassageAssembly,
    candidate_texts: &[(String, String)],
    references: Vec<AdmittedSelection>,
) -> super::CurrentPassageAssemblyRecord {
    crate::assemble_candidate_context::add_working_texts(&mut record, source, candidate_texts);
    if references
        .iter()
        .any(|reference| reference.selected_text.is_none())
    {
        crate::assemble_candidate_context::block(
            &mut record,
            super::ContextBlockReason::WorkingTargetRevisionUnavailable,
        );
    }
    record.operation_requirement.selection_references = Some(references);
    record
}

/// The exact text of a Chapter range over the `(block id, text)` pairs of its base, in block
/// order, with the blocks it covers. A reversed, empty, or out-of-range selection has none.
pub fn select_chapter_text(
    blocks: &[(String, String)],
    start: &TextPoint,
    end: &TextPoint,
) -> Option<(String, Vec<String>)> {
    let index = |point: &TextPoint| {
        blocks
            .iter()
            .position(|(id, _)| *id == point.manuscript_block_id)
    };
    let (first, last) = (index(start)?, index(end)?);
    if first > last {
        return None;
    }
    let covered = &blocks[first..=last];
    let mut parts = Vec::with_capacity(covered.len());
    for (position, (_, text)) in covered.iter().enumerate() {
        let from = if position == 0 { start.offset } else { 0 };
        let to = if position + 1 == covered.len() {
            end.offset
        } else {
            u32::try_from(text.encode_utf16().count()).ok()?
        };
        parts.push(utf16_range(text, from, to)?);
    }
    let text = parts.join("\n");
    (!text.is_empty()).then(|| (text, covered.iter().map(|(id, _)| id.clone()).collect()))
}

/// The exact non-empty text between two UTF-16 offsets of a candidate text.
pub fn select_candidate_text(text: &str, start: u32, end: u32) -> Option<String> {
    utf16_range(text, start, end).filter(|selected| !selected.is_empty())
}

/// A range that splits a surrogate pair or passes the end of the text has no text.
fn utf16_range(text: &str, start: u32, end: u32) -> Option<String> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let range = usize::try_from(start).ok()?..usize::try_from(end).ok()?;
    String::from_utf16(units.get(range)?).ok()
}

#[cfg(test)]
#[path = "selection_reference_tests.rs"]
mod tests;
