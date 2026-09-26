use storyos_contracts as wire;

/// Map the exact public unit list to the Core domain input.
pub fn author_edit_units_from_wire(
    units: &[wire::AuthorEditUnit],
) -> Vec<storyos_core::AuthorEditUnit> {
    units
        .iter()
        .map(|unit| storyos_core::AuthorEditUnit {
            normalized_primitives: unit
                .normalized_primitives
                .iter()
                .map(|primitive| match primitive {
                    wire::AuthorEditPrimitive::ReplaceStructuredSelection { replacement } => {
                        storyos_core::AuthorEditPrimitive::ReplaceStructuredSelection {
                            replacement: replacement
                                .iter()
                                .map(|block| storyos_core::ReplacementBlock {
                                    block_kind: match block.block_kind {
                                        wire::ManuscriptBlockKind::Paragraph => {
                                            storyos_core::ManuscriptBlockKind::Paragraph
                                        }
                                        wire::ManuscriptBlockKind::Heading => {
                                            storyos_core::ManuscriptBlockKind::Heading
                                        }
                                    },
                                    text: block.text.clone(),
                                })
                                .collect(),
                        }
                    }
                    wire::AuthorEditPrimitive::ReplaceSelection { from, to, text } => {
                        storyos_core::AuthorEditPrimitive::ReplaceSelection {
                            from: *from,
                            to: *to,
                            text: text.clone(),
                        }
                    }
                    wire::AuthorEditPrimitive::ReplaceBlockSelection {
                        manuscript_block_id,
                        from,
                        to,
                        text,
                    } => storyos_core::AuthorEditPrimitive::ReplaceBlockSelection {
                        manuscript_block_id: manuscript_block_id.clone(),
                        from: *from,
                        to: *to,
                        text: text.clone(),
                    },
                    wire::AuthorEditPrimitive::SplitBlock {
                        manuscript_block_id,
                        offset,
                        new_manuscript_block_id,
                    } => storyos_core::AuthorEditPrimitive::SplitBlock {
                        manuscript_block_id: manuscript_block_id.clone(),
                        offset: *offset,
                        new_manuscript_block_id: new_manuscript_block_id.clone(),
                    },
                    wire::AuthorEditPrimitive::JoinBlocks {
                        left_manuscript_block_id,
                        right_manuscript_block_id,
                    } => storyos_core::AuthorEditPrimitive::JoinBlocks {
                        left_manuscript_block_id: left_manuscript_block_id.clone(),
                        right_manuscript_block_id: right_manuscript_block_id.clone(),
                    },
                    wire::AuthorEditPrimitive::MoveBlock {
                        manuscript_block_id,
                        to_index,
                    } => storyos_core::AuthorEditPrimitive::MoveBlock {
                        manuscript_block_id: manuscript_block_id.clone(),
                        to_index: *to_index,
                    },
                    wire::AuthorEditPrimitive::RetypeBlock {
                        manuscript_block_id,
                        block_kind,
                    } => storyos_core::AuthorEditPrimitive::RetypeBlock {
                        manuscript_block_id: manuscript_block_id.clone(),
                        block_kind: match block_kind {
                            wire::ManuscriptBlockKind::Paragraph => {
                                storyos_core::ManuscriptBlockKind::Paragraph
                            }
                            wire::ManuscriptBlockKind::Heading => {
                                storyos_core::ManuscriptBlockKind::Heading
                            }
                        },
                    },
                })
                .collect(),
            selection_snapshot: storyos_core::SelectionSnapshot {
                ordered_selection: unit.selection_snapshot.ordered_selection.as_ref().map(
                    |selection| storyos_core::OrderedSourceSelection {
                        sources: selection
                            .sources
                            .iter()
                            .map(|source| storyos_core::SelectedEditSource {
                                owner: match &source.owner {
                                    wire::EditSourceOwner::Manuscript {
                                        manuscript_block_id,
                                    } => storyos_core::EditSourceOwner::Manuscript {
                                        manuscript_block_id: manuscript_block_id.clone(),
                                    },
                                    wire::EditSourceOwner::Proposal {
                                        proposal_id,
                                        operation_id,
                                        revision_id,
                                        manuscript_block_id,
                                    } => storyos_core::EditSourceOwner::Proposal {
                                        proposal_id: proposal_id.clone(),
                                        operation_id: operation_id.clone(),
                                        revision_id: revision_id.clone(),
                                        manuscript_block_id: manuscript_block_id.clone(),
                                    },
                                },
                                coordinate_profile: source.coordinate_profile.clone(),
                                from: source.from,
                                to: source.to,
                                block_kind: match source.block_kind {
                                    wire::ManuscriptBlockKind::Paragraph => {
                                        storyos_core::ManuscriptBlockKind::Paragraph
                                    }
                                    wire::ManuscriptBlockKind::Heading => {
                                        storyos_core::ManuscriptBlockKind::Heading
                                    }
                                },
                                source_text: source.source_text.clone(),
                            })
                            .collect(),
                        anchor: storyos_core::SourceSelectionEndpoint {
                            source_index: selection.anchor.source_index,
                            source_offset: selection.anchor.source_offset,
                        },
                        head: storyos_core::SourceSelectionEndpoint {
                            source_index: selection.head.source_index,
                            source_offset: selection.head.source_offset,
                        },
                    },
                ),
                coordinate_profile: unit.selection_snapshot.coordinate_profile.clone(),
                from: unit.selection_snapshot.from,
                to: unit.selection_snapshot.to,
            },
        })
        .collect()
}

/// Map the exact Core unit list to the public retained payload.
pub fn author_edit_units_to_wire(
    units: &[storyos_core::AuthorEditUnit],
) -> Vec<wire::AuthorEditUnit> {
    units
        .iter()
        .map(|unit| wire::AuthorEditUnit {
            normalized_primitives: unit
                .normalized_primitives
                .iter()
                .map(|primitive| match primitive {
                    storyos_core::AuthorEditPrimitive::ReplaceStructuredSelection {
                        replacement,
                    } => wire::AuthorEditPrimitive::ReplaceStructuredSelection {
                        replacement: replacement
                            .iter()
                            .map(|block| wire::ReplacementBlock {
                                block_kind: match block.block_kind {
                                    storyos_core::ManuscriptBlockKind::Paragraph => {
                                        wire::ManuscriptBlockKind::Paragraph
                                    }
                                    storyos_core::ManuscriptBlockKind::Heading => {
                                        wire::ManuscriptBlockKind::Heading
                                    }
                                },
                                text: block.text.clone(),
                            })
                            .collect(),
                    },
                    storyos_core::AuthorEditPrimitive::ReplaceSelection { from, to, text } => {
                        wire::AuthorEditPrimitive::ReplaceSelection {
                            from: *from,
                            to: *to,
                            text: text.clone(),
                        }
                    }
                    storyos_core::AuthorEditPrimitive::ReplaceBlockSelection {
                        manuscript_block_id,
                        from,
                        to,
                        text,
                    } => wire::AuthorEditPrimitive::ReplaceBlockSelection {
                        manuscript_block_id: manuscript_block_id.clone(),
                        from: *from,
                        to: *to,
                        text: text.clone(),
                    },
                    storyos_core::AuthorEditPrimitive::SplitBlock {
                        manuscript_block_id,
                        offset,
                        new_manuscript_block_id,
                    } => wire::AuthorEditPrimitive::SplitBlock {
                        manuscript_block_id: manuscript_block_id.clone(),
                        offset: *offset,
                        new_manuscript_block_id: new_manuscript_block_id.clone(),
                    },
                    storyos_core::AuthorEditPrimitive::JoinBlocks {
                        left_manuscript_block_id,
                        right_manuscript_block_id,
                    } => wire::AuthorEditPrimitive::JoinBlocks {
                        left_manuscript_block_id: left_manuscript_block_id.clone(),
                        right_manuscript_block_id: right_manuscript_block_id.clone(),
                    },
                    storyos_core::AuthorEditPrimitive::MoveBlock {
                        manuscript_block_id,
                        to_index,
                    } => wire::AuthorEditPrimitive::MoveBlock {
                        manuscript_block_id: manuscript_block_id.clone(),
                        to_index: *to_index,
                    },
                    storyos_core::AuthorEditPrimitive::RetypeBlock {
                        manuscript_block_id,
                        block_kind,
                    } => wire::AuthorEditPrimitive::RetypeBlock {
                        manuscript_block_id: manuscript_block_id.clone(),
                        block_kind: match block_kind {
                            storyos_core::ManuscriptBlockKind::Paragraph => {
                                wire::ManuscriptBlockKind::Paragraph
                            }
                            storyos_core::ManuscriptBlockKind::Heading => {
                                wire::ManuscriptBlockKind::Heading
                            }
                        },
                    },
                })
                .collect(),
            selection_snapshot: wire::SelectionSnapshot {
                ordered_selection: unit.selection_snapshot.ordered_selection.as_ref().map(
                    |selection| wire::OrderedSourceSelection {
                        sources: selection
                            .sources
                            .iter()
                            .map(|source| wire::SelectedEditSource {
                                owner: match &source.owner {
                                    storyos_core::EditSourceOwner::Manuscript {
                                        manuscript_block_id,
                                    } => wire::EditSourceOwner::Manuscript {
                                        manuscript_block_id: manuscript_block_id.clone(),
                                    },
                                    storyos_core::EditSourceOwner::Proposal {
                                        proposal_id,
                                        operation_id,
                                        revision_id,
                                        manuscript_block_id,
                                    } => wire::EditSourceOwner::Proposal {
                                        proposal_id: proposal_id.clone(),
                                        operation_id: operation_id.clone(),
                                        revision_id: revision_id.clone(),
                                        manuscript_block_id: manuscript_block_id.clone(),
                                    },
                                },
                                coordinate_profile: source.coordinate_profile.clone(),
                                from: source.from,
                                to: source.to,
                                block_kind: match source.block_kind {
                                    storyos_core::ManuscriptBlockKind::Paragraph => {
                                        wire::ManuscriptBlockKind::Paragraph
                                    }
                                    storyos_core::ManuscriptBlockKind::Heading => {
                                        wire::ManuscriptBlockKind::Heading
                                    }
                                },
                                source_text: source.source_text.clone(),
                            })
                            .collect(),
                        anchor: wire::SourceSelectionEndpoint {
                            source_index: selection.anchor.source_index,
                            source_offset: selection.anchor.source_offset,
                        },
                        head: wire::SourceSelectionEndpoint {
                            source_index: selection.head.source_index,
                            source_offset: selection.head.source_offset,
                        },
                    },
                ),
                coordinate_profile: unit.selection_snapshot.coordinate_profile.clone(),
                from: unit.selection_snapshot.from,
                to: unit.selection_snapshot.to,
            },
        })
        .collect()
}
