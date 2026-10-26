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
                coordinate_profile: unit.selection_snapshot.coordinate_profile.clone(),
                from: unit.selection_snapshot.from,
                to: unit.selection_snapshot.to,
            },
        })
        .collect()
}
