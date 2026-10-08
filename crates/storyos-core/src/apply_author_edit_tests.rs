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
        ordered_source_facts: None,
        target_refs: vec!["manuscript:chapter".to_owned()],
        observed_ownership_partition: "authoritative".to_owned(),
        inline_edit_disposition: InlineEditDisposition::Unspecified,
        author_edit_units: vec![AuthorEditUnit {
            normalized_primitives: vec![AuthorEditPrimitive::ReplaceSelection {
                from: 1,
                to: 3,
                text: "!".to_owned(),
            }],
            selection_snapshot: SelectionSnapshot {
                ordered_selection: None,
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
        TransitionOutcome::Applied(AuthorEditApplied::AuthoritativeApplied {
            body: "A!B".to_owned()
        })
    );
}

#[test]
fn ordered_units_apply_against_one_transient_body() {
    let mut batch = command();
    batch.author_edit_units = vec![
        AuthorEditUnit {
            normalized_primitives: vec![AuthorEditPrimitive::ReplaceSelection {
                from: 1,
                to: 3,
                text: "xy".to_owned(),
            }],
            selection_snapshot: SelectionSnapshot {
                ordered_selection: None,
                coordinate_profile: UTF16_COORDINATE_PROFILE.to_owned(),
                from: 1,
                to: 3,
            },
        },
        AuthorEditUnit {
            normalized_primitives: vec![AuthorEditPrimitive::ReplaceSelection {
                from: 3,
                to: 3,
                text: "!".to_owned(),
            }],
            selection_snapshot: SelectionSnapshot {
                ordered_selection: None,
                coordinate_profile: UTF16_COORDINATE_PROFILE.to_owned(),
                from: 3,
                to: 3,
            },
        },
    ];

    assert_eq!(
        apply_author_edit(&batch),
        TransitionOutcome::Applied(AuthorEditApplied::AuthoritativeApplied {
            body: "Axy!B".to_owned()
        })
    );
}

#[test]
fn invalid_later_unit_refuses_the_complete_batch() {
    let mut batch = command();
    let mut invalid_later = batch.author_edit_units[0].clone();
    invalid_later.selection_snapshot.to = 2;
    let AuthorEditPrimitive::ReplaceSelection { from, to, text } =
        &mut invalid_later.normalized_primitives[0]
    else {
        panic!("legacy command must use ReplaceSelection")
    };
    *from = 4;
    *to = 4;
    *text = "!".to_owned();
    batch.author_edit_units.push(invalid_later);

    assert_eq!(
        apply_author_edit(&batch),
        TransitionOutcome::Refused(AuthorEditRefused::Refused(
            AuthorEditRefusal::InvalidSelection
        ))
    );
}

#[test]
fn stale_head_and_invalid_selections_fail_without_a_partial_result() {
    let mut stale = command();
    stale.expected_authoritative_revision_id = "revision-0".to_owned();
    assert_eq!(
        apply_author_edit(&stale),
        TransitionOutcome::Conflicted(AuthorEditConflict::StaleAuthoritativeHead)
    );

    for (start, end) in [(1, 2), (2, 2), (5, 5), (3, 1), (2, 3), (1, 5)] {
        let mut invalid = command();
        let unit = &mut invalid.author_edit_units[0];
        unit.selection_snapshot.from = start;
        unit.selection_snapshot.to = end;
        let AuthorEditPrimitive::ReplaceSelection { from, to, .. } =
            &mut unit.normalized_primitives[0]
        else {
            panic!("legacy command must use ReplaceSelection")
        };
        *from = start;
        *to = end;
        assert_eq!(
            apply_author_edit(&invalid),
            TransitionOutcome::Refused(AuthorEditRefused::Refused(
                AuthorEditRefusal::InvalidSelection
            ))
        );
    }
}

#[test]
fn current_proposal_fact_conflicts_with_a_stale_authoritative_observation() {
    let mut stale = command();
    stale.current_ownership.proposal_head_revision_ids = vec!["proposal-revision".to_owned()];
    assert_eq!(
        apply_author_edit(&stale),
        TransitionOutcome::Conflicted(AuthorEditConflict::ProposalHeadPresent)
    );
}

#[test]
fn matching_proposal_heads_revise_the_candidate_without_authority() {
    let mut revise = command();
    revise.current_body = "Guard the narrator voice in this passage.".to_owned();
    revise.expected_proposal_head_revision_ids = vec!["proposal-revision".to_owned()];
    revise.current_ownership.proposal_head_revision_ids = vec!["proposal-revision".to_owned()];
    revise.observed_ownership_partition = "mixed".to_owned();
    let unit = &mut revise.author_edit_units[0];
    unit.selection_snapshot.from = 0;
    unit.selection_snapshot.to = 5;
    let AuthorEditPrimitive::ReplaceSelection { from, to, text } =
        &mut unit.normalized_primitives[0]
    else {
        panic!("legacy command must use ReplaceSelection")
    };
    *from = 0;
    *to = 5;
    *text = "Keep".to_owned();

    assert_eq!(
        apply_author_edit(&revise),
        TransitionOutcome::Applied(AuthorEditApplied::ProposalRevised {
            candidate_text: "Keep the narrator voice in this passage.".to_owned()
        })
    );
}

#[test]
fn exclusive_edge_input_applies_to_authority_without_revising_the_candidate() {
    let mut edge = command();
    edge.current_body = "Guard the narrator voice in this passage.".to_owned();
    edge.expected_proposal_head_revision_ids = vec!["proposal-revision".to_owned()];
    edge.current_ownership.proposal_head_revision_ids = vec!["proposal-revision".to_owned()];
    edge.observed_ownership_partition = "mixed".to_owned();
    edge.inline_edit_disposition = InlineEditDisposition::AuthoritativeDespiteReservation;
    let unit = &mut edge.author_edit_units[0];
    unit.selection_snapshot.from = 0;
    unit.selection_snapshot.to = 5;
    let AuthorEditPrimitive::ReplaceSelection { from, to, text } =
        &mut unit.normalized_primitives[0]
    else {
        panic!("legacy command must use ReplaceSelection")
    };
    *from = 0;
    *to = 5;
    *text = "Keep".to_owned();

    assert_eq!(
        apply_author_edit(&edge),
        TransitionOutcome::Applied(AuthorEditApplied::AuthoritativeApplied {
            body: "Keep the narrator voice in this passage.".to_owned()
        })
    );
}

#[test]
fn unchanged_content_is_a_no_effect_core_result() {
    for (start, end, replacement) in [(1, 3, "😀"), (3, 3, "")] {
        let mut unchanged = command();
        let unit = &mut unchanged.author_edit_units[0];
        unit.selection_snapshot.from = start;
        unit.selection_snapshot.to = end;
        let AuthorEditPrimitive::ReplaceSelection { from, to, text } =
            &mut unit.normalized_primitives[0]
        else {
            panic!("legacy command must use ReplaceSelection")
        };
        *from = start;
        *to = end;
        *text = replacement.to_owned();

        assert_eq!(
            apply_author_edit(&unchanged),
            TransitionOutcome::NoEffect(AuthorEditNoEffect::ContentUnchanged)
        );
    }
}

#[test]
fn every_zero_authority_author_edit_outcome_round_trips_through_its_receipt_codes() {
    let outcomes: [ApplyAuthorEditOutcome; 8] = [
        TransitionOutcome::NoEffect(AuthorEditNoEffect::ContentUnchanged),
        TransitionOutcome::Conflicted(AuthorEditConflict::StaleAuthoritativeHead),
        TransitionOutcome::Conflicted(AuthorEditConflict::ProposalHeadPresent),
        TransitionOutcome::Conflicted(AuthorEditConflict::OwnershipChanged),
        TransitionOutcome::Refused(AuthorEditRefused::Refused(
            AuthorEditRefusal::UnsupportedIntentShape,
        )),
        TransitionOutcome::Refused(AuthorEditRefused::Refused(
            AuthorEditRefusal::InvalidSelection,
        )),
        TransitionOutcome::Refused(AuthorEditRefused::Refused(
            AuthorEditRefusal::TargetMismatch,
        )),
        TransitionOutcome::Refused(AuthorEditRefused::RefusedToDraft),
    ];
    let recorded = outcomes
        .iter()
        .map(|outcome| (outcome.receipt_result_kind(), outcome.reason_code()))
        .collect::<Vec<_>>();
    assert_eq!(
        recorded,
        [
            ("no_effect", Some("content_unchanged")),
            ("conflicted", Some("stale_authoritative_head")),
            ("conflicted", Some("proposal_head_present")),
            ("conflicted", Some("ownership_changed")),
            ("refused", Some("unsupported_intent_shape")),
            ("refused", Some("invalid_selection")),
            ("refused", Some("target_mismatch")),
            ("refused_to_draft", None),
        ]
    );
    for (outcome, (result, reason)) in outcomes.into_iter().zip(recorded) {
        assert_eq!(
            ApplyAuthorEditOutcome::from_zero_authority_codes(result, reason),
            Some(outcome)
        );
    }
    // A Refused Edit Draft records no reason, and a refusal needs its reason.
    assert_eq!(
        ApplyAuthorEditOutcome::from_zero_authority_codes("refused_to_draft", Some("x")),
        None
    );
    assert_eq!(
        ApplyAuthorEditOutcome::from_zero_authority_codes("refused", None),
        None
    );
}

#[test]
fn each_applied_author_edit_variant_gives_its_receipt_result_kind() {
    assert_eq!(
        [
            AuthorEditApplied::AuthoritativeApplied {
                body: String::new()
            }
            .result_kind(),
            AuthorEditApplied::ProposalRevised {
                candidate_text: String::new()
            }
            .result_kind(),
        ],
        ["authoritative_applied", "proposal_revised"]
    );
}
