use super::*;

fn command() -> ApplyAuthorEdit {
    ApplyAuthorEdit {
        chapter_id: "chapter".to_owned(),
        current_authoritative_revision_id: "revision-1".to_owned(),
        current_body: "A😀B".to_owned(),
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
            normalized_primitives: vec![AuthorEditPrimitive::ReplaceSelection {
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
fn one_replace_selection_is_classified_as_one_authoritative_result() {
    assert_eq!(
        apply_author_edit(&command()),
        ApplyAuthorEditResult::AuthoritativeApplied {
            body: "A!B".to_owned()
        }
    );
}

#[test]
fn ordered_units_use_each_prior_transient_body() {
    let mut batch = command();
    let first = &mut batch.author_edit_units[0];
    let AuthorEditPrimitive::ReplaceSelection { text, .. } = &mut first.normalized_primitives[0];
    *text = "xy".to_owned();
    batch.author_edit_units.push(AuthorEditUnit {
        normalized_primitives: vec![AuthorEditPrimitive::ReplaceSelection {
            from: 2,
            to: 3,
            text: "!".to_owned(),
        }],
        selection_snapshot: SelectionSnapshot {
            coordinate_profile: UTF16_COORDINATE_PROFILE.to_owned(),
            from: 2,
            to: 3,
        },
    });

    assert_eq!(
        apply_author_edit(&batch),
        ApplyAuthorEditResult::AuthoritativeApplied {
            body: "Ax!B".to_owned()
        }
    );
}

#[test]
fn invalid_later_unit_refuses_the_whole_batch() {
    let mut batch = command();
    batch.author_edit_units.push(AuthorEditUnit {
        normalized_primitives: vec![AuthorEditPrimitive::ReplaceSelection {
            from: 99,
            to: 99,
            text: "never authoritative".to_owned(),
        }],
        selection_snapshot: SelectionSnapshot {
            coordinate_profile: UTF16_COORDINATE_PROFILE.to_owned(),
            from: 99,
            to: 99,
        },
    });

    assert_eq!(
        apply_author_edit(&batch),
        ApplyAuthorEditResult::Refused {
            reason: AuthorEditRefusal::InvalidSelection
        }
    );
}

#[test]
fn a_batch_above_the_prerelease_ceiling_is_refused() {
    let mut batch = command();
    batch.author_edit_units = vec![batch.author_edit_units[0].clone(); 241];

    assert_eq!(
        apply_author_edit(&batch),
        ApplyAuthorEditResult::Refused {
            reason: AuthorEditRefusal::UnsupportedIntentShape
        }
    );
}

#[test]
fn stale_head_and_surrogate_split_fail_without_a_partial_result() {
    let mut stale = command();
    stale.expected_authoritative_revision_id = "revision-0".to_owned();
    assert_eq!(
        apply_author_edit(&stale),
        ApplyAuthorEditResult::Conflicted {
            reason: AuthorEditConflict::StaleAuthoritativeHead
        }
    );

    let mut split = command();
    split.author_edit_units[0].selection_snapshot.to = 2;
    let AuthorEditPrimitive::ReplaceSelection { to, .. } =
        &mut split.author_edit_units[0].normalized_primitives[0];
    *to = 2;
    assert_eq!(
        apply_author_edit(&split),
        ApplyAuthorEditResult::Refused {
            reason: AuthorEditRefusal::InvalidSelection
        }
    );
}

#[test]
fn current_proposal_fact_conflicts_with_a_stale_authoritative_observation() {
    let mut stale = command();
    stale.current_ownership.proposal_head_revision_ids = vec!["proposal-revision".to_owned()];
    assert_eq!(
        apply_author_edit(&stale),
        ApplyAuthorEditResult::Conflicted {
            reason: AuthorEditConflict::ProposalHeadPresent
        }
    );
}

#[test]
fn unchanged_content_is_a_no_effect_core_result() {
    let mut unchanged = command();
    let AuthorEditPrimitive::ReplaceSelection { text, .. } =
        &mut unchanged.author_edit_units[0].normalized_primitives[0];
    *text = "😀".to_owned();

    assert_eq!(
        apply_author_edit(&unchanged),
        ApplyAuthorEditResult::NoEffect {
            reason: AuthorEditNoEffect::ContentUnchanged
        }
    );
}
