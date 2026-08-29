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
