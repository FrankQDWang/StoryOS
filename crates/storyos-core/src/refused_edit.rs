//! Prove one complete ordered-source replacement before preserving it.

use crate::{
    ApplyAuthorEdit, ApplyAuthorEditResult, AuthorEditConflict, AuthorEditPrimitive,
    AuthorEditRefusal, EditSourceOwner, InlineTargetBlock, ManuscriptBlock, ManuscriptBlockKind,
    OpenInlineProposal, OpenInlineProposalAnchor, OpenInlineProposalResult,
    UTF16_COORDINATE_PROFILE, open_inline_proposal, utf16_offset_to_byte,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentOrderedSourceFacts {
    pub blocks: Vec<ManuscriptBlock>,
    pub proposals: Vec<ProposalEditSourceFacts>,
    pub complete: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalEditSourceFacts {
    pub owner: EditSourceOwner,
    pub kind: String,
    pub generation: String,
    pub validation: String,
    pub closure: String,
    pub resolution: String,
    pub reservation: String,
    pub base_revision_id: String,
    pub candidate_text: String,
    pub anchors: Vec<OpenInlineProposalAnchor>,
}

pub(super) fn classify(command: &ApplyAuthorEdit) -> ApplyAuthorEditResult {
    let conflict = || ApplyAuthorEditResult::Conflicted {
        reason: AuthorEditConflict::OwnershipChanged,
    };
    let invalid = || ApplyAuthorEditResult::Refused {
        reason: AuthorEditRefusal::InvalidSelection,
    };
    let Some(facts) = command.ordered_source_facts.as_ref() else {
        return conflict();
    };
    if !facts.complete {
        return conflict();
    }
    let [unit] = command.author_edit_units.as_slice() else {
        return invalid();
    };
    let [AuthorEditPrimitive::ReplaceStructuredSelection { replacement }] =
        unit.normalized_primitives.as_slice()
    else {
        return invalid();
    };
    let Some(selection) = unit.selection_snapshot.ordered_selection.as_ref() else {
        return invalid();
    };
    if command.target_refs != [format!("manuscript:{}", command.chapter_id)]
        || unit.selection_snapshot.coordinate_profile != "storyos.editor.ordered-source.v1"
        || replacement.is_empty()
        || selection.sources.is_empty()
        || unit.selection_snapshot.from != selection.anchor.source_offset
        || unit.selection_snapshot.to != selection.head.source_offset
    {
        return invalid();
    }
    let block_id = |owner: &EditSourceOwner| match owner {
        EditSourceOwner::Manuscript {
            manuscript_block_id,
        }
        | EditSourceOwner::Proposal {
            manuscript_block_id,
            ..
        } => manuscript_block_id.clone(),
    };
    let first_block = block_id(&selection.sources[0].owner);
    let last_block = block_id(&selection.sources[selection.sources.len() - 1].owner);
    let Some(start_block) = facts
        .blocks
        .iter()
        .position(|block| block.manuscript_block_id == first_block)
    else {
        return conflict();
    };
    let Some(end_block) = facts
        .blocks
        .iter()
        .position(|block| block.manuscript_block_id == last_block)
    else {
        return conflict();
    };
    let Some(covered_blocks) = facts.blocks.get(start_block..=end_block) else {
        return conflict();
    };
    let mut by_block = std::collections::BTreeMap::<String, Vec<&ProposalEditSourceFacts>>::new();
    for proposal in &facts.proposals {
        by_block
            .entry(block_id(&proposal.owner))
            .or_default()
            .push(proposal);
    }
    let selected_operations: std::collections::BTreeSet<(&str, &str)> = selection
        .sources
        .iter()
        .filter_map(|source| match &source.owner {
            EditSourceOwner::Manuscript { .. } => None,
            EditSourceOwner::Proposal {
                proposal_id,
                operation_id,
                ..
            } => Some((proposal_id.as_str(), operation_id.as_str())),
        })
        .collect();
    struct ProjectedSource<'a> {
        owner: EditSourceOwner,
        coordinate_profile: &'static str,
        from: u32,
        to: u32,
        block_kind: &'a ManuscriptBlockKind,
        source_text: &'a str,
    }
    let mut projection = Vec::new();
    for block in covered_blocks {
        let mut proposals = by_block
            .remove(&block.manuscript_block_id)
            .unwrap_or_default();
        proposals.sort_by_key(|proposal| proposal.anchors.first().map_or(0, |anchor| anchor.from));
        let has_proposals = !proposals.is_empty();
        let mut cursor = 0;
        let end = block.text.encode_utf16().count() as u32;
        let manuscript = ProjectedSource {
            owner: EditSourceOwner::Manuscript {
                manuscript_block_id: block.manuscript_block_id.clone(),
            },
            coordinate_profile: "prosemirror-token-utf16.v1",
            from: 0,
            to: end,
            block_kind: &block.block_kind,
            source_text: &block.text,
        };
        for proposal in proposals {
            let selected = match &proposal.owner {
                EditSourceOwner::Manuscript { .. } => false,
                EditSourceOwner::Proposal {
                    proposal_id,
                    operation_id,
                    ..
                } => selected_operations.contains(&(proposal_id.as_str(), operation_id.as_str())),
            };
            if selected
                && (proposal.generation != "ready"
                    || proposal.validation != "valid"
                    || proposal.closure != "open"
                    || proposal.resolution != "pending"
                    || proposal.reservation != "unresolved"
                    || proposal.base_revision_id != command.current_authoritative_revision_id)
            {
                return conflict();
            }
            let (from, to) = match proposal.kind.as_str() {
                "block_edit" => (0, end),
                "inline_edit" => {
                    let [anchor] = proposal.anchors.as_slice() else {
                        return conflict();
                    };
                    if selected
                        && open_inline_proposal(&OpenInlineProposal {
                            scope_matches: true,
                            target_block_present: true,
                            expected_base_revision_id: command
                                .current_authoritative_revision_id
                                .clone(),
                            current_base_revision_id: Some(
                                command.current_authoritative_revision_id.clone(),
                            ),
                            conflicting_reservation: false,
                            current_schema_version: 1,
                            current_coordinate_profile: "prosemirror-token-utf16.v1".to_owned(),
                            blocks: vec![InlineTargetBlock {
                                manuscript_block_id: block.manuscript_block_id.clone(),
                                block_kind: match block.block_kind {
                                    ManuscriptBlockKind::Paragraph => "paragraph",
                                    ManuscriptBlockKind::Heading => "heading",
                                }
                                .to_owned(),
                                text: block.text.clone(),
                            }],
                            anchors: proposal.anchors.clone(),
                        }) != OpenInlineProposalResult::Applied
                    {
                        return conflict();
                    }
                    (anchor.from, anchor.to)
                }
                _ => return conflict(),
            };
            if from < cursor || to > end {
                return conflict();
            }
            if cursor < from {
                projection.push(ProjectedSource {
                    owner: manuscript.owner.clone(),
                    from: cursor,
                    to: from,
                    ..manuscript
                });
            }
            projection.push(ProjectedSource {
                owner: proposal.owner.clone(),
                coordinate_profile: UTF16_COORDINATE_PROFILE,
                from: 0,
                to: proposal.candidate_text.encode_utf16().count() as u32,
                block_kind: &block.block_kind,
                source_text: &proposal.candidate_text,
            });
            cursor = to;
        }
        if cursor < end || (!has_proposals && end == 0) {
            projection.push(ProjectedSource {
                from: cursor,
                ..manuscript
            });
        }
    }
    let Some(first) = selection.sources.first() else {
        return invalid();
    };
    let Some(start) = projection.iter().position(|source| {
        source.owner == first.owner
            && first.from >= source.from
            && (first.from < source.to || source.from == source.to)
    }) else {
        return conflict();
    };
    let Some(covered) = projection.get(start..start + selection.sources.len()) else {
        return invalid();
    };
    let mut owners = std::collections::BTreeSet::new();
    for (index, (selected, current)) in selection.sources.iter().zip(covered).enumerate() {
        if selected.owner != current.owner
            || selected.source_text != current.source_text
            || &selected.block_kind != current.block_kind
            || selected.coordinate_profile != current.coordinate_profile
            || selected.from < current.from
            || selected.to > current.to
            || (selected.from > selected.to
                || (selected.from == selected.to && current.from != current.to))
            || (index > 0 && selected.from != current.from)
            || (index + 1 < covered.len() && selected.to != current.to)
            || utf16_offset_to_byte(current.source_text, selected.from).is_none()
            || utf16_offset_to_byte(current.source_text, selected.to).is_none()
        {
            return conflict();
        }
        owners.insert(match &current.owner {
            EditSourceOwner::Manuscript { .. } => "manuscript".to_owned(),
            EditSourceOwner::Proposal { proposal_id, .. } => format!("proposal:{proposal_id}"),
        });
    }
    let last_index = selection.sources.len() - 1;
    let last = &selection.sources[last_index];
    let forward = selection.anchor.source_index == 0
        && selection.anchor.source_offset == first.from
        && selection.head.source_index as usize == last_index
        && selection.head.source_offset == last.to;
    let backward = selection.head.source_index == 0
        && selection.head.source_offset == first.from
        && selection.anchor.source_index as usize == last_index
        && selection.anchor.source_offset == last.to;
    if (!forward && !backward) || owners.len() < 2 {
        return invalid();
    }
    ApplyAuthorEditResult::RefusedToDraft
}
