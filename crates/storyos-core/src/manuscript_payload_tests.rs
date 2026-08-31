use super::*;

#[test]
fn upgrade_legacy_string_preserves_exact_unicode_and_one_stable_block() {
    assert_eq!(
        upgrade_legacy_manuscript("A😀B\n雨落在窗沿。", "block-1"),
        ManuscriptPayload {
            schema_version: MANUSCRIPT_SCHEMA_VERSION,
            coordinate_version: COORDINATE_VERSION,
            blocks: vec![ManuscriptBlock {
                manuscript_block_id: "block-1".to_owned(),
                block_kind: ManuscriptBlockKind::Paragraph,
                text: "A😀B\n雨落在窗沿。".to_owned(),
            }],
        }
    );
}

#[test]
fn empty_legacy_string_becomes_one_empty_paragraph_block() {
    assert_eq!(
        upgrade_legacy_manuscript("", "block-empty"),
        ManuscriptPayload {
            schema_version: MANUSCRIPT_SCHEMA_VERSION,
            coordinate_version: COORDINATE_VERSION,
            blocks: vec![ManuscriptBlock {
                manuscript_block_id: "block-empty".to_owned(),
                block_kind: ManuscriptBlockKind::Paragraph,
                text: String::new(),
            }],
        }
    );
}

fn versioned_command() -> ApplyVersionedAuthorEdit {
    ApplyVersionedAuthorEdit {
        chapter_id: "chapter".to_owned(),
        current_authoritative_revision_id: "revision-1".to_owned(),
        current_payload: upgrade_legacy_manuscript("A😀B\n雨", "block-1"),
        expected_authoritative_revision_id: "revision-1".to_owned(),
        expected_proposal_head_revision_ids: Vec::new(),
        current_ownership: CurrentOwnershipFacts {
            proposal_head_revision_ids: Vec::new(),
            anchor_refs: Vec::new(),
            unresolved_reservation_refs: Vec::new(),
        },
        target_refs: vec!["manuscript:chapter".to_owned()],
        observed_ownership_partition: "authoritative".to_owned(),
        author_edit_units: vec![AuthorEditUnit {
            normalized_primitives: vec![AuthorEditPrimitive::ReplaceBlockSelection {
                manuscript_block_id: "block-1".to_owned(),
                from: 1,
                to: 3,
                text: "!".to_owned(),
            }],
            selection_snapshot: SelectionSnapshot {
                coordinate_profile: UTF16_COORDINATE_PROFILE.to_owned(),
                from: 1,
                to: 3,
            },
        }],
    }
}

#[test]
fn versioned_replace_preserves_unicode_line_breaks_and_block_identity() {
    assert_eq!(
        apply_versioned_author_edit(&versioned_command()),
        ApplyVersionedAuthorEditResult::AuthoritativeApplied {
            payload: ManuscriptPayload {
                schema_version: MANUSCRIPT_SCHEMA_VERSION,
                coordinate_version: COORDINATE_VERSION,
                blocks: vec![ManuscriptBlock {
                    manuscript_block_id: "block-1".to_owned(),
                    block_kind: ManuscriptBlockKind::Paragraph,
                    text: "A!B\n雨".to_owned(),
                }],
            }
        }
    );
}

#[test]
fn legacy_replace_against_versioned_payload_is_unsupported() {
    let mut command = versioned_command();
    command.author_edit_units[0].normalized_primitives =
        vec![AuthorEditPrimitive::ReplaceSelection {
            from: 1,
            to: 3,
            text: "!".to_owned(),
        }];
    assert_eq!(
        apply_versioned_author_edit(&command),
        ApplyVersionedAuthorEditResult::Refused {
            reason: AuthorEditRefusal::UnsupportedIntentShape
        }
    );
}

#[test]
fn replace_on_unknown_block_is_invalid_selection() {
    let mut command = versioned_command();
    let AuthorEditPrimitive::ReplaceBlockSelection {
        manuscript_block_id,
        ..
    } = &mut command.author_edit_units[0].normalized_primitives[0]
    else {
        panic!("versioned command must target one block")
    };
    *manuscript_block_id = "block-other".to_owned();
    assert_eq!(
        apply_versioned_author_edit(&command),
        ApplyVersionedAuthorEditResult::Refused {
            reason: AuthorEditRefusal::InvalidSelection
        }
    );
}

fn two_paragraphs() -> ManuscriptPayload {
    ManuscriptPayload {
        schema_version: MANUSCRIPT_SCHEMA_VERSION,
        coordinate_version: COORDINATE_VERSION,
        blocks: vec![
            ManuscriptBlock {
                manuscript_block_id: "block-left".to_owned(),
                block_kind: ManuscriptBlockKind::Paragraph,
                text: "Hello".to_owned(),
            },
            ManuscriptBlock {
                manuscript_block_id: "block-right".to_owned(),
                block_kind: ManuscriptBlockKind::Paragraph,
                text: "World".to_owned(),
            },
        ],
    }
}

fn split_command() -> ApplyVersionedAuthorEdit {
    ApplyVersionedAuthorEdit {
        chapter_id: "chapter".to_owned(),
        current_authoritative_revision_id: "revision-1".to_owned(),
        current_payload: upgrade_legacy_manuscript("HelloWorld", "block-left"),
        expected_authoritative_revision_id: "revision-1".to_owned(),
        expected_proposal_head_revision_ids: Vec::new(),
        current_ownership: CurrentOwnershipFacts {
            proposal_head_revision_ids: Vec::new(),
            anchor_refs: Vec::new(),
            unresolved_reservation_refs: Vec::new(),
        },
        target_refs: vec!["manuscript:chapter".to_owned()],
        observed_ownership_partition: "authoritative".to_owned(),
        author_edit_units: vec![AuthorEditUnit {
            normalized_primitives: vec![AuthorEditPrimitive::SplitBlock {
                manuscript_block_id: "block-left".to_owned(),
                offset: 5,
                new_manuscript_block_id: "block-right".to_owned(),
            }],
            selection_snapshot: SelectionSnapshot {
                coordinate_profile: UTF16_COORDINATE_PROFILE.to_owned(),
                from: 5,
                to: 5,
            },
        }],
    }
}

#[test]
fn split_keeps_the_starting_fragment_identity_and_assigns_the_new_right_id() {
    assert_eq!(
        apply_versioned_author_edit(&split_command()),
        ApplyVersionedAuthorEditResult::AuthoritativeApplied {
            payload: two_paragraphs()
        }
    );
}

#[test]
fn join_keeps_the_left_identity_and_drops_the_right_from_current_payload() {
    let mut command = split_command();
    command.current_payload = two_paragraphs();
    command.author_edit_units = vec![AuthorEditUnit {
        normalized_primitives: vec![AuthorEditPrimitive::JoinBlocks {
            left_manuscript_block_id: "block-left".to_owned(),
            right_manuscript_block_id: "block-right".to_owned(),
        }],
        selection_snapshot: SelectionSnapshot {
            coordinate_profile: UTF16_COORDINATE_PROFILE.to_owned(),
            from: 5,
            to: 5,
        },
    }];
    assert_eq!(
        apply_versioned_author_edit(&command),
        ApplyVersionedAuthorEditResult::AuthoritativeApplied {
            payload: upgrade_legacy_manuscript("HelloWorld", "block-left")
        }
    );
}

#[test]
fn join_of_nonadjacent_blocks_is_invalid() {
    let mut command = split_command();
    command.current_payload = ManuscriptPayload {
        schema_version: MANUSCRIPT_SCHEMA_VERSION,
        coordinate_version: COORDINATE_VERSION,
        blocks: vec![
            ManuscriptBlock {
                manuscript_block_id: "block-left".to_owned(),
                block_kind: ManuscriptBlockKind::Paragraph,
                text: "Hello".to_owned(),
            },
            ManuscriptBlock {
                manuscript_block_id: "block-mid".to_owned(),
                block_kind: ManuscriptBlockKind::Paragraph,
                text: "Mid".to_owned(),
            },
            ManuscriptBlock {
                manuscript_block_id: "block-right".to_owned(),
                block_kind: ManuscriptBlockKind::Paragraph,
                text: "World".to_owned(),
            },
        ],
    };
    command.author_edit_units = vec![AuthorEditUnit {
        normalized_primitives: vec![AuthorEditPrimitive::JoinBlocks {
            left_manuscript_block_id: "block-left".to_owned(),
            right_manuscript_block_id: "block-right".to_owned(),
        }],
        selection_snapshot: SelectionSnapshot {
            coordinate_profile: UTF16_COORDINATE_PROFILE.to_owned(),
            from: 5,
            to: 5,
        },
    }];
    assert_eq!(
        apply_versioned_author_edit(&command),
        ApplyVersionedAuthorEditResult::Refused {
            reason: AuthorEditRefusal::InvalidSelection
        }
    );
}

#[test]
fn replace_still_targets_one_block_inside_a_split_payload() {
    let mut command = split_command();
    command.current_payload = two_paragraphs();
    command.author_edit_units = vec![AuthorEditUnit {
        normalized_primitives: vec![AuthorEditPrimitive::ReplaceBlockSelection {
            manuscript_block_id: "block-right".to_owned(),
            from: 0,
            to: 5,
            text: "StoryOS".to_owned(),
        }],
        selection_snapshot: SelectionSnapshot {
            coordinate_profile: UTF16_COORDINATE_PROFILE.to_owned(),
            from: 0,
            to: 5,
        },
    }];
    assert_eq!(
        apply_versioned_author_edit(&command),
        ApplyVersionedAuthorEditResult::AuthoritativeApplied {
            payload: ManuscriptPayload {
                schema_version: MANUSCRIPT_SCHEMA_VERSION,
                coordinate_version: COORDINATE_VERSION,
                blocks: vec![
                    ManuscriptBlock {
                        manuscript_block_id: "block-left".to_owned(),
                        block_kind: ManuscriptBlockKind::Paragraph,
                        text: "Hello".to_owned(),
                    },
                    ManuscriptBlock {
                        manuscript_block_id: "block-right".to_owned(),
                        block_kind: ManuscriptBlockKind::Paragraph,
                        text: "StoryOS".to_owned(),
                    },
                ],
            }
        }
    );
}

#[test]
fn split_at_the_end_keeps_an_empty_right_block_and_the_starting_identity() {
    let mut command = split_command();
    command.current_payload = upgrade_legacy_manuscript("Hello", "block-left");
    assert_eq!(
        apply_versioned_author_edit(&command),
        ApplyVersionedAuthorEditResult::AuthoritativeApplied {
            payload: ManuscriptPayload {
                schema_version: MANUSCRIPT_SCHEMA_VERSION,
                coordinate_version: COORDINATE_VERSION,
                blocks: vec![
                    ManuscriptBlock {
                        manuscript_block_id: "block-left".to_owned(),
                        block_kind: ManuscriptBlockKind::Paragraph,
                        text: "Hello".to_owned(),
                    },
                    ManuscriptBlock {
                        manuscript_block_id: "block-right".to_owned(),
                        block_kind: ManuscriptBlockKind::Paragraph,
                        text: String::new(),
                    },
                ],
            }
        }
    );
}

#[test]
fn one_unit_joins_then_replaces_across_adjacent_blocks_atomically() {
    let mut command = split_command();
    command.current_payload = two_paragraphs();
    command.author_edit_units = vec![AuthorEditUnit {
        normalized_primitives: vec![
            AuthorEditPrimitive::JoinBlocks {
                left_manuscript_block_id: "block-left".to_owned(),
                right_manuscript_block_id: "block-right".to_owned(),
            },
            AuthorEditPrimitive::ReplaceBlockSelection {
                manuscript_block_id: "block-left".to_owned(),
                from: 2,
                to: 8,
                text: "X".to_owned(),
            },
        ],
        selection_snapshot: SelectionSnapshot {
            coordinate_profile: UTF16_COORDINATE_PROFILE.to_owned(),
            from: 2,
            to: 8,
        },
    }];
    assert_eq!(
        apply_versioned_author_edit(&command),
        ApplyVersionedAuthorEditResult::AuthoritativeApplied {
            payload: upgrade_legacy_manuscript("HeXld", "block-left")
        }
    );
}

#[test]
fn a_later_invalid_primitive_in_the_same_unit_refuses_the_complete_range() {
    let mut command = split_command();
    command.current_payload = two_paragraphs();
    command.author_edit_units = vec![AuthorEditUnit {
        normalized_primitives: vec![
            AuthorEditPrimitive::JoinBlocks {
                left_manuscript_block_id: "block-left".to_owned(),
                right_manuscript_block_id: "block-right".to_owned(),
            },
            AuthorEditPrimitive::ReplaceBlockSelection {
                manuscript_block_id: "block-left".to_owned(),
                from: 0,
                to: 99,
                text: "X".to_owned(),
            },
        ],
        selection_snapshot: SelectionSnapshot {
            coordinate_profile: UTF16_COORDINATE_PROFILE.to_owned(),
            from: 0,
            to: 99,
        },
    }];
    assert_eq!(
        apply_versioned_author_edit(&command),
        ApplyVersionedAuthorEditResult::Refused {
            reason: AuthorEditRefusal::InvalidSelection
        }
    );
}

#[test]
fn one_unit_replaces_then_splits_so_pasted_paragraphs_receive_new_identities() {
    let mut command = split_command();
    command.current_payload = upgrade_legacy_manuscript("Hello", "block-left");
    command.author_edit_units = vec![AuthorEditUnit {
        normalized_primitives: vec![
            AuthorEditPrimitive::ReplaceBlockSelection {
                manuscript_block_id: "block-left".to_owned(),
                from: 5,
                to: 5,
                text: "XY".to_owned(),
            },
            AuthorEditPrimitive::SplitBlock {
                manuscript_block_id: "block-left".to_owned(),
                offset: 6,
                new_manuscript_block_id: "block-pasted".to_owned(),
            },
        ],
        selection_snapshot: SelectionSnapshot {
            coordinate_profile: UTF16_COORDINATE_PROFILE.to_owned(),
            from: 5,
            to: 5,
        },
    }];
    assert_eq!(
        apply_versioned_author_edit(&command),
        ApplyVersionedAuthorEditResult::AuthoritativeApplied {
            payload: ManuscriptPayload {
                schema_version: MANUSCRIPT_SCHEMA_VERSION,
                coordinate_version: COORDINATE_VERSION,
                blocks: vec![
                    ManuscriptBlock {
                        manuscript_block_id: "block-left".to_owned(),
                        block_kind: ManuscriptBlockKind::Paragraph,
                        text: "HelloX".to_owned(),
                    },
                    ManuscriptBlock {
                        manuscript_block_id: "block-pasted".to_owned(),
                        block_kind: ManuscriptBlockKind::Paragraph,
                        text: "Y".to_owned(),
                    },
                ],
            }
        }
    );
}

#[test]
fn one_unit_moves_a_block_and_keeps_both_identities() {
    let mut command = split_command();
    command.current_payload = two_paragraphs();
    command.author_edit_units = vec![AuthorEditUnit {
        normalized_primitives: vec![AuthorEditPrimitive::MoveBlock {
            manuscript_block_id: "block-left".to_owned(),
            to_index: 1,
        }],
        selection_snapshot: SelectionSnapshot {
            coordinate_profile: UTF16_COORDINATE_PROFILE.to_owned(),
            from: 0,
            to: 0,
        },
    }];
    assert_eq!(
        apply_versioned_author_edit(&command),
        ApplyVersionedAuthorEditResult::AuthoritativeApplied {
            payload: ManuscriptPayload {
                schema_version: MANUSCRIPT_SCHEMA_VERSION,
                coordinate_version: COORDINATE_VERSION,
                blocks: vec![
                    ManuscriptBlock {
                        manuscript_block_id: "block-right".to_owned(),
                        block_kind: ManuscriptBlockKind::Paragraph,
                        text: "World".to_owned(),
                    },
                    ManuscriptBlock {
                        manuscript_block_id: "block-left".to_owned(),
                        block_kind: ManuscriptBlockKind::Paragraph,
                        text: "Hello".to_owned(),
                    },
                ],
            }
        }
    );
}

#[test]
fn an_invalid_move_index_refuses_without_changing_payload_order() {
    let mut command = split_command();
    command.current_payload = two_paragraphs();
    command.author_edit_units = vec![AuthorEditUnit {
        normalized_primitives: vec![AuthorEditPrimitive::MoveBlock {
            manuscript_block_id: "block-left".to_owned(),
            to_index: 2,
        }],
        selection_snapshot: SelectionSnapshot {
            coordinate_profile: UTF16_COORDINATE_PROFILE.to_owned(),
            from: 0,
            to: 0,
        },
    }];
    assert_eq!(
        apply_versioned_author_edit(&command),
        ApplyVersionedAuthorEditResult::Refused {
            reason: AuthorEditRefusal::InvalidSelection,
        }
    );
}

#[test]
fn one_to_one_retype_keeps_identity_and_text() {
    let mut command = split_command();
    command.current_payload = two_paragraphs();
    command.author_edit_units = vec![AuthorEditUnit {
        normalized_primitives: vec![AuthorEditPrimitive::RetypeBlock {
            manuscript_block_id: "block-left".to_owned(),
            block_kind: ManuscriptBlockKind::Heading,
        }],
        selection_snapshot: SelectionSnapshot {
            coordinate_profile: UTF16_COORDINATE_PROFILE.to_owned(),
            from: 0,
            to: 0,
        },
    }];
    assert_eq!(
        apply_versioned_author_edit(&command),
        ApplyVersionedAuthorEditResult::AuthoritativeApplied {
            payload: ManuscriptPayload {
                schema_version: MANUSCRIPT_SCHEMA_VERSION,
                coordinate_version: COORDINATE_VERSION,
                blocks: vec![
                    ManuscriptBlock {
                        manuscript_block_id: "block-left".to_owned(),
                        block_kind: ManuscriptBlockKind::Heading,
                        text: "Hello".to_owned(),
                    },
                    ManuscriptBlock {
                        manuscript_block_id: "block-right".to_owned(),
                        block_kind: ManuscriptBlockKind::Paragraph,
                        text: "World".to_owned(),
                    },
                ],
            }
        }
    );
}

#[test]
fn a_stale_head_still_has_zero_authority_effect_for_move() {
    let mut command = split_command();
    command.current_payload = two_paragraphs();
    command.expected_authoritative_revision_id = "stale".to_owned();
    command.author_edit_units = vec![AuthorEditUnit {
        normalized_primitives: vec![AuthorEditPrimitive::MoveBlock {
            manuscript_block_id: "block-left".to_owned(),
            to_index: 1,
        }],
        selection_snapshot: SelectionSnapshot {
            coordinate_profile: UTF16_COORDINATE_PROFILE.to_owned(),
            from: 0,
            to: 0,
        },
    }];
    assert_eq!(
        apply_versioned_author_edit(&command),
        ApplyVersionedAuthorEditResult::Conflicted {
            reason: AuthorEditConflict::StaleAuthoritativeHead,
        }
    );
}

#[test]
fn two_disconnected_block_replacements_in_one_unit_are_an_unsupported_intent() {
    let mut command = versioned_command();
    command.current_payload = two_paragraphs();
    command.author_edit_units = vec![AuthorEditUnit {
        normalized_primitives: vec![
            AuthorEditPrimitive::ReplaceBlockSelection {
                manuscript_block_id: "block-left".to_owned(),
                from: 0,
                to: 5,
                text: "Hi".to_owned(),
            },
            AuthorEditPrimitive::ReplaceBlockSelection {
                manuscript_block_id: "block-right".to_owned(),
                from: 0,
                to: 5,
                text: "Hi".to_owned(),
            },
        ],
        selection_snapshot: SelectionSnapshot {
            coordinate_profile: UTF16_COORDINATE_PROFILE.to_owned(),
            from: 0,
            to: 5,
        },
    }];
    assert_eq!(
        apply_versioned_author_edit(&command),
        ApplyVersionedAuthorEditResult::Refused {
            reason: AuthorEditRefusal::UnsupportedIntentShape
        }
    );
}

#[test]
fn chapter_display_body_joins_current_paragraphs_with_one_line_break() {
    assert_eq!(
        chapter_display_body(&two_paragraphs().blocks),
        "Hello\nWorld"
    );
    assert_eq!(
        chapter_display_body(&upgrade_legacy_manuscript("A\nB", "block-1").blocks),
        "A\nB"
    );
}
