use storyos_application::{
    ApplyAuthorEditCommand, AuthorCommandAdmissionIds, AuthorEditError, AuthorEditProposalTarget,
    AuthorEditSettlement,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::author_edit::AuthorEditFault;
use crate::create_volume_authority_tests::{NamedEdit, adjusted_edit_command, named_edit_command};

use super::acceptance::acceptable_proposal;
use super::agent_run::seed_run;
use super::support::{run_without_foreign_keys, settlement_rows, stores, two_chapter_writer};
use super::undo::{edit_in_chapter_of, session_chapter_head};

/// The Author Edit cases of the contract rows.
#[derive(Clone, Copy, Debug)]
enum Edit {
    /// A prose edit of the Authoritative Revision.
    Prose,
    /// An edit of the candidate of a Proposal Operation.
    ProposalCandidate,
    /// An edit that leaves the body unchanged.
    Unchanged,
    /// An edit that expects an earlier Revision of the Chapter.
    StaleHead,
    /// An edit whose selection is outside the body.
    InvalidSelection,
    /// A structured edit across an Inline Proposal and the Manuscript, which is kept as a Refused
    /// Edit Draft.
    RefusedToDraft,
}

const EDITS: [Edit; 6] = [
    Edit::Prose,
    Edit::ProposalCandidate,
    Edit::Unchanged,
    Edit::StaleHead,
    Edit::InvalidSelection,
    Edit::RefusedToDraft,
];

/// The Receipt result kind and the Receipt, Author Action, Activity, Commit, and Snapshot rows of
/// each case, in the order of `EDITS`.
const RECORDS: [(&str, [i64; 5]); 6] = [
    ("authoritative_applied", [1, 1, 1, 1, 1]),
    ("proposal_revised", [1, 1, 0, 0, 0]),
    ("no_effect", [1, 0, 0, 0, 0]),
    ("conflicted", [1, 0, 0, 0, 0]),
    ("refused", [1, 0, 0, 0, 0]),
    ("refused_to_draft", [1, 0, 0, 0, 0]),
];

/// The Author Edit of `edit` in a new Project, with its issued Command Challenge.
async fn edit_command(
    store: &PostgresProjectReader,
    admin: &Client,
    edit: Edit,
    base: u16,
) -> ApplyAuthorEditCommand {
    let suffix = format!("{:04x}", base + 9);
    if let Edit::RefusedToDraft = edit {
        return refused_to_draft_command(store, admin, base, &suffix).await;
    }
    if let Edit::ProposalCandidate = edit {
        let (scope, input, _chapter_a_head) = acceptable_proposal(store, admin, base).await;
        let run_id: String = admin
            .query_one(
                "SELECT source_run_id::text FROM storyos.proposals
                  WHERE proposal_id = $1::text::uuid",
                &[&input.proposal_id],
            )
            .await
            .unwrap()
            .get(/*idx*/ 0);
        seed_run(admin, &scope, &run_id, "completed").await;
        let (chapter_b, block_id): (String, String) = admin
            .query_one(
                "SELECT chapter_id::text, manuscript_block_id::text FROM storyos.proposals
                  WHERE proposal_id = $1::text::uuid",
                &[&input.proposal_id],
            )
            .await
            .map(|row| (row.get(/*idx*/ 0), row.get(/*idx*/ 1)))
            .unwrap();
        let editor_session_id = input.editor_session_id.as_ref().to_owned();
        edit_in_chapter_of(
            admin,
            &editor_session_id,
            &input.expected_authoritative_revision_id,
        )
        .await;
        return named_edit_command(
            store,
            &scope,
            NamedEdit {
                editor_session_id: &editor_session_id,
                chapter_id: &chapter_b,
                expected_revision_id: &input.expected_authoritative_revision_id,
                suffix: &suffix,
                local_intent_sequence: 1,
                text: "x",
                proposal_target: Some(AuthorEditProposalTarget {
                    proposal_id: input.proposal_id.clone(),
                    operation_id: input.selected_operation_ids[0].clone(),
                    revision_id: input.proposal_revision_id.clone(),
                    manuscript_block_id: block_id,
                }),
            },
        )
        .await;
    }
    let (scope, chapter_a, _chapter_b, _revision_b, editor_session_id) =
        two_chapter_writer(store, base).await;
    let head = session_chapter_head(admin, &editor_session_id).await;
    let named = |suffix, expected_revision_id, local_intent_sequence, text| NamedEdit {
        editor_session_id: &editor_session_id,
        chapter_id: &chapter_a,
        expected_revision_id,
        suffix,
        local_intent_sequence,
        text,
        proposal_target: None,
    };
    match edit {
        Edit::Prose | Edit::ProposalCandidate | Edit::RefusedToDraft => {
            named_edit_command(
                store,
                &scope,
                named(&suffix, &head, /*local_intent_sequence*/ 1, "x"),
            )
            .await
        }
        Edit::Unchanged => {
            named_edit_command(
                store,
                &scope,
                named(&suffix, &head, /*local_intent_sequence*/ 1, ""),
            )
            .await
        }
        Edit::StaleHead => {
            let first = named_edit_command(
                store,
                &scope,
                named(&suffix, &head, /*local_intent_sequence*/ 1, "x"),
            )
            .await;
            store.apply_author_edit(&first).await.unwrap();
            let later = format!("{:04x}", base + 10);
            named_edit_command(
                store,
                &scope,
                named(&later, &head, /*local_intent_sequence*/ 2, "y"),
            )
            .await
        }
        Edit::InvalidSelection => {
            adjusted_edit_command(
                store,
                &scope,
                named(&suffix, &head, /*local_intent_sequence*/ 1, "x"),
                |command| {
                    let unit = &mut command.author_edit_units[0];
                    unit.selection_snapshot.from = 999;
                    unit.selection_snapshot.to = 999;
                    if let storyos_core::AuthorEditPrimitive::ReplaceSelection {
                        from, to, ..
                    } = &mut unit.normalized_primitives[0]
                    {
                        (*from, *to) = (999, 999);
                    }
                },
            )
            .await
        }
    }
}

/// A structured edit that replaces an Inline Proposal candidate and the Manuscript text after
/// its Anchor. Core keeps it as a Refused Edit Draft.
async fn refused_to_draft_command(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
    suffix: &str,
) -> ApplyAuthorEditCommand {
    let (scope, _chapter_a, chapter_b, revision_b, editor_session_id) =
        two_chapter_writer(store, base).await;
    // The writer edits Chapter B and writes its text first.
    edit_in_chapter_of(admin, &editor_session_id, &revision_b).await;
    let written = format!("{:04x}", base + 8);
    let written = named_edit_command(
        store,
        &scope,
        NamedEdit {
            editor_session_id: &editor_session_id,
            chapter_id: &chapter_b,
            expected_revision_id: &revision_b,
            suffix: &written,
            local_intent_sequence: 1,
            text: "Hello world",
            proposal_target: None,
        },
    )
    .await;
    let written = store.apply_author_edit(&written).await.unwrap();
    let storyos_application::AuthorEditSettlementEffect::AuthoritativeApplied {
        ids, blocks, ..
    } = written.effect
    else {
        panic!("the text edit must apply");
    };
    let revision_b = ids.revision_id;
    let block = blocks.into_iter().next().unwrap();
    let (block_id, block_text) = (block.manuscript_block_id, block.text);
    let input = storyos_application::AcceptProposalInput {
        editor_session_id: storyos_application::EditorSessionId::new(editor_session_id.clone()),
        proposal_id: Uuid::now_v7().to_string(),
        proposal_revision_id: Uuid::now_v7().to_string(),
        validation_receipt_id: Uuid::now_v7().to_string(),
        selected_operation_ids: vec![Uuid::now_v7().to_string()],
        expected_authoritative_revision_id: revision_b.clone(),
    };
    run_without_foreign_keys(
        admin,
        &format!(
            "INSERT INTO storyos.proposals
               (owner_user_id, project_id, proposal_id, kind, chapter_id, manuscript_block_id,
                source_run_id, source_decision_id)
             VALUES ('{owner}', '{project}', '{proposal}', 'inline_edit', '{chapter}',
                     '{block}', '{run}', '{decision}');
             INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{revision}', 'ready', 'valid',
                     'open', 'Accepted text', '{base_revision}');
             INSERT INTO storyos.proposal_heads
               (owner_user_id, project_id, proposal_id, current_revision_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{revision}');
             INSERT INTO storyos.proposal_operations
               (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                resolution, reservation_state, candidate_text)
             VALUES ('{owner}', '{project}', '{proposal}', '{operation}', '{block}',
                     'pending', 'unresolved', 'Accepted text')",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            proposal = input.proposal_id,
            chapter = chapter_b,
            block = block_id,
            run = Uuid::now_v7(),
            decision = Uuid::now_v7(),
            revision = input.proposal_revision_id,
            base_revision = revision_b,
            operation = input.selected_operation_ids[0],
        ),
    )
    .await;
    let block_end = block_text.encode_utf16().count() as u32;
    assert!(block_end >= 2, "the fixture Block needs two characters");
    let anchor_to = 1;
    let digest = storyos_core::proposal_anchor_base_slice_digest(
        &block_id,
        "paragraph",
        /*manuscript_schema_version*/ 1,
        "prosemirror-token-utf16.v1",
        /*from*/ 0,
        anchor_to,
        &block_text[..1],
    );
    run_without_foreign_keys(
        admin,
        &format!(
            "INSERT INTO storyos.proposal_anchors
               (owner_user_id, project_id, proposal_id, operation_id, anchor_order,
                manuscript_block_id, base_authoritative_revision_id, manuscript_schema_version,
                coordinate_profile, range_from, range_to, boundary_profile, base_slice_digest)
             VALUES ('{owner}', '{project}', '{proposal}', '{operation}', 1, '{block}',
                     '{revision}', 1, 'prosemirror-token-utf16.v1', 0, {anchor_to},
                     'exclusive-authoritative-edges.v1', '{digest}')",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            proposal = input.proposal_id,
            operation = input.selected_operation_ids[0],
            block = block_id,
            revision = revision_b,
        ),
    )
    .await;
    let candidate = "Accepted text".to_owned();
    let candidate_end = candidate.encode_utf16().count() as u32;
    let sources = vec![
        storyos_core::SelectedEditSource {
            owner: storyos_core::EditSourceOwner::Proposal {
                proposal_id: input.proposal_id.clone(),
                operation_id: input.selected_operation_ids[0].clone(),
                revision_id: input.proposal_revision_id.clone(),
                manuscript_block_id: block_id.clone(),
            },
            coordinate_profile: storyos_core::UTF16_COORDINATE_PROFILE.to_owned(),
            from: 0,
            to: candidate_end,
            block_kind: storyos_core::ManuscriptBlockKind::Paragraph,
            source_text: candidate,
        },
        storyos_core::SelectedEditSource {
            owner: storyos_core::EditSourceOwner::Manuscript {
                manuscript_block_id: block_id,
            },
            coordinate_profile: "prosemirror-token-utf16.v1".to_owned(),
            from: anchor_to,
            to: block_end,
            block_kind: storyos_core::ManuscriptBlockKind::Paragraph,
            source_text: block_text,
        },
    ];
    adjusted_edit_command(
        store,
        &scope,
        NamedEdit {
            editor_session_id: &editor_session_id,
            chapter_id: &chapter_b,
            expected_revision_id: &revision_b,
            suffix,
            local_intent_sequence: 2,
            text: "x",
            proposal_target: None,
        },
        |command| {
            command.expected_proposal_head_revision_ids = vec![input.proposal_revision_id.clone()];
            command.observed_ownership_partition = "mixed".to_owned();
            command.author_edit_units = vec![storyos_core::AuthorEditUnit {
                normalized_primitives: vec![
                    storyos_core::AuthorEditPrimitive::ReplaceStructuredSelection {
                        replacement: vec![storyos_core::ReplacementBlock {
                            block_kind: storyos_core::ManuscriptBlockKind::Paragraph,
                            text: "New passage".to_owned(),
                        }],
                    },
                ],
                selection_snapshot: storyos_core::SelectionSnapshot {
                    ordered_selection: Some(storyos_core::OrderedSourceSelection {
                        sources,
                        anchor: storyos_core::SourceSelectionEndpoint {
                            source_index: 0,
                            source_offset: 0,
                        },
                        head: storyos_core::SourceSelectionEndpoint {
                            source_index: 1,
                            source_offset: block_end,
                        },
                    }),
                    coordinate_profile: "storyos.editor.ordered-source.v1".to_owned(),
                    from: 0,
                    to: block_end,
                },
            }];
        },
    )
    .await
}

/// The command of `command` with new request identities, as an exact retry sends it.
fn exact_retry(command: &ApplyAuthorEditCommand) -> ApplyAuthorEditCommand {
    let mut retry = command.clone();
    retry.ids = AuthorCommandAdmissionIds {
        command_id: Uuid::now_v7().to_string(),
        author_command_admission_id: Uuid::now_v7().to_string(),
        receipt_id: Uuid::now_v7().to_string(),
    };
    retry
}

async fn result_kind(admin: &Client, receipt_id: &str) -> String {
    admin
        .query_one(
            "SELECT result_kind FROM storyos.domain_receipts WHERE receipt_id = $1::text::uuid",
            &[&receipt_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// Counts the Admission and settlement rows and reads the fence state of one command.
async fn admission_state(admin: &Client, command: &ApplyAuthorEditCommand) -> (i64, i64, String) {
    let row = admin
        .query_one(
            "SELECT (SELECT count(*) FROM storyos.author_command_admissions
                      WHERE author_command_admission_id = $1::text::uuid),
                    (SELECT count(*) FROM storyos.author_command_admission_settlements
                      WHERE author_command_admission_id = $1::text::uuid),
                    (SELECT outcome_kind FROM storyos.command_idempotency
                      WHERE command_kind = 'applyAuthorEdit'
                        AND idempotency_key = $2::text::uuid)",
            &[
                &command.ids.author_command_admission_id,
                &command.challenge_binding.idempotency_key,
            ],
        )
        .await
        .unwrap();
    (row.get(/*idx*/ 0), row.get(/*idx*/ 1), row.get(/*idx*/ 2))
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_author_edit_outcome_replays_its_first_settlement_and_writes_only_its_records() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    for (edit, base) in EDITS.into_iter().zip((0x3800..).step_by(/*step*/ 0x10)) {
        let command = edit_command(&store, &admin, edit, base).await;
        let first = store.apply_author_edit(&command).await.unwrap();
        let replayed = store
            .apply_author_edit(&exact_retry(&command))
            .await
            .unwrap();
        assert_eq!(replayed, first, "{edit:?}");
        observed.push((
            result_kind(&admin, &first.ids.receipt_id).await,
            settlement_rows(&admin, &first.ids.receipt_id).await,
        ));
    }
    assert_eq!(
        observed
            .iter()
            .map(|(kind, rows)| (kind.as_str(), *rows))
            .collect::<Vec<_>>(),
        RECORDS
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_failing_settle_step_rolls_back_every_row_and_a_later_settle_step_completes_it() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    for (edit, base) in EDITS.into_iter().zip((0x3860..).step_by(/*step*/ 0x10)) {
        let command = edit_command(&store, &admin, edit, base).await;
        let failed = store
            .apply_author_edit_with_fault(&command, AuthorEditFault::CoreBeforeCommit)
            .await;
        assert!(
            matches!(&failed, Err(AuthorEditError::Unavailable(source))
                if source.to_string() == "CFP-CORE-BEFORE-COMMIT"),
            "{edit:?}"
        );
        let after_failure = (
            settlement_rows(&admin, &command.ids.receipt_id).await,
            admission_state(&admin, &command).await,
        );
        let settled = store
            .complete_admitted_author_edit(&command, AuthorEditFault::None)
            .await
            .unwrap();
        observed.push((
            after_failure,
            result_kind(&admin, &settled.ids.receipt_id).await,
        ));
    }
    let expected =
        RECORDS.map(|(kind, _rows)| (([0; 5], (1, 0, "in_progress".to_owned())), kind.to_owned()));
    assert_eq!(observed, expected);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_cut_after_the_admit_step_leaves_one_admission_and_an_exact_retry_conflicts() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    for (edit, base) in EDITS.into_iter().zip((0x38c0..).step_by(/*step*/ 0x10)) {
        let command = edit_command(&store, &admin, edit, base).await;
        let cut = store
            .apply_author_edit_with_fault(&command, AuthorEditFault::AfterAdmissionBeforeCore)
            .await;
        assert!(
            matches!(&cut, Err(AuthorEditError::Unavailable(source))
                if source.to_string() == "CFP-ADMISSION-BEFORE-CORE"),
            "{edit:?}"
        );
        let after_cut = admission_state(&admin, &command).await;
        let retry = store.apply_author_edit(&exact_retry(&command)).await;
        observed.push((
            after_cut,
            matches!(retry, Err(AuthorEditError::BindingConflict)),
            settlement_rows(&admin, &command.ids.receipt_id).await,
        ));
    }
    assert_eq!(
        observed,
        vec![((1, 0, "in_progress".to_owned()), true, [0; 5]); EDITS.len()]
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_author_edit_replay_needs_no_response_record_and_damaged_evidence_is_a_store_fault() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    for (edit, base, damage) in [
        (
            Edit::Prose,
            0x3920,
            (|receipt_id: &str| {
                format!(
                    "DELETE FROM storyos.project_activity_events WHERE receipt_id = '{receipt_id}'"
                )
            }) as fn(&str) -> String,
        ),
        (Edit::ProposalCandidate, 0x3930, |receipt_id: &str| {
            format!("DELETE FROM storyos.author_action_entries WHERE receipt_id = '{receipt_id}'")
        }),
        (Edit::Unchanged, 0x3940, |receipt_id: &str| {
            format!(
                "INSERT INTO storyos.authoritative_commits
                   (owner_user_id, project_id, authoritative_commit_id,
                    authoritative_commit_sequence, manuscript_object_id, prior_revision_id,
                    resulting_revision_id, author_command_admission_id, receipt_id,
                    receipt_result_kind)
                 SELECT receipt.owner_user_id, receipt.project_id, gen_random_uuid(), 999999,
                        admission.chapter_object_id, receipt.prior_heads[1],
                        receipt.resulting_heads[1], receipt.author_command_admission_id,
                        receipt.receipt_id, 'authoritative_applied'
                   FROM storyos.domain_receipts AS receipt
                   JOIN storyos.author_command_admissions AS admission
                     ON admission.author_command_admission_id =
                        receipt.author_command_admission_id
                  WHERE receipt.receipt_id = '{receipt_id}'"
            )
        }),
        (Edit::StaleHead, 0x3950, |receipt_id: &str| {
            format!(
                "INSERT INTO storyos.project_activity_events
                   (owner_user_id, project_id, project_activity_position,
                    project_activity_event_id, event_kind, receipt_id, receipt_result_kind,
                    authoritative_commit_id, resulting_revision_id, author_action_sequence)
                 SELECT owner_user_id, project_id, 999999, gen_random_uuid(),
                        'authoritative_author_edit_applied', receipt_id,
                        'authoritative_applied', gen_random_uuid(), resulting_heads[1], 999999
                   FROM storyos.domain_receipts WHERE receipt_id = '{receipt_id}'"
            )
        }),
        (Edit::RefusedToDraft, 0x3960, |receipt_id: &str| {
            format!("DELETE FROM storyos.draft_lifecycle_events WHERE receipt_id = '{receipt_id}'")
        }),
    ] {
        let command = edit_command(&store, &admin, edit, base).await;
        let first = store.apply_author_edit(&command).await.unwrap();
        // The Command Idempotency Fence keeps no response record for an Author Edit.
        admin
            .execute(
                "UPDATE storyos.command_idempotency
                    SET acknowledgement_format = NULL, response_project = NULL
                  WHERE command_kind = 'applyAuthorEdit' AND idempotency_key = $1::text::uuid",
                &[&command.challenge_binding.idempotency_key],
            )
            .await
            .unwrap();
        let replayed: Result<AuthorEditSettlement, _> =
            store.apply_author_edit(&exact_retry(&command)).await;
        assert_eq!(replayed.unwrap(), first, "{edit:?}");
        run_without_foreign_keys(&admin, &damage(&first.ids.receipt_id)).await;
        observed.push(matches!(
            store.apply_author_edit(&exact_retry(&command)).await,
            Err(AuthorEditError::Unavailable(_))
        ));
    }
    assert_eq!(observed, vec![true; 5]);
}

/// The outcome query of one Author Edit, as the Server sends it after a cut.
async fn outcome(
    store: &PostgresProjectReader,
    command: &ApplyAuthorEditCommand,
) -> storyos_application::ApplyAuthorEditOutcome {
    storyos_application::get_apply_author_edit_outcome(
        store,
        &storyos_application::ResolveApplyAuthorEditOutcome {
            project_scope: command.project_scope.clone(),
            client_binding: command.client_binding.clone(),
            limit_profile_revision: command.challenge_binding.limit_profile_revision.clone(),
            idempotency_key: command.challenge_binding.idempotency_key.clone(),
            nonce_digest: command.nonce_digest.clone(),
        },
    )
    .await
    .unwrap()
}

/// The Receipt facts of one settlement that do not depend on its identities.
async fn receipt_facts(
    admin: &Client,
    settlement: &AuthorEditSettlement,
) -> (String, [i64; 5], Option<String>) {
    let body = match &settlement.effect {
        storyos_application::AuthorEditSettlementEffect::AuthoritativeApplied { body, .. } => {
            Some(body.clone())
        }
        _ => None,
    };
    (
        result_kind(admin, &settlement.ids.receipt_id).await,
        settlement_rows(admin, &settlement.ids.receipt_id).await,
        body,
    )
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_outcome_query_settles_a_cut_admission_with_the_receipt_of_an_uninterrupted_edit() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut recovered = Vec::new();
    let mut uninterrupted = Vec::new();
    for ((edit, cut_base), direct_base) in EDITS
        .into_iter()
        .zip((0x3a00..).step_by(/*step*/ 0x10))
        .zip((0x3b00..).step_by(/*step*/ 0x10))
    {
        let command = edit_command(&store, &admin, edit, cut_base).await;
        store
            .apply_author_edit_with_fault(&command, AuthorEditFault::AfterAdmissionBeforeCore)
            .await
            .expect_err("the cut must stop after the admit step");
        let storyos_application::ApplyAuthorEditOutcome::Committed(committed) =
            outcome(&store, &command).await
        else {
            panic!("the outcome query must settle the open Admission of {edit:?}");
        };
        // A second outcome query replays the same settlement.
        assert_eq!(
            outcome(&store, &command).await,
            storyos_application::ApplyAuthorEditOutcome::Committed(committed.clone()),
            "{edit:?}"
        );
        recovered.push(receipt_facts(&admin, &committed.settlement).await);

        let direct = edit_command(&store, &admin, edit, direct_base).await;
        let settled = store.apply_author_edit(&direct).await.unwrap();
        uninterrupted.push(receipt_facts(&admin, &settled).await);
    }
    assert_eq!(recovered, uninterrupted);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_outcome_query_settles_an_expired_admission_as_requires_reconfirmation() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let command = edit_command(&store, &admin, Edit::Prose, /*base*/ 0x3c00).await;
    store
        .apply_author_edit_with_fault(&command, AuthorEditFault::AfterAdmissionBeforeCore)
        .await
        .expect_err("the cut must stop after the admit step");
    run_without_foreign_keys(
        &admin,
        &format!(
            "UPDATE storyos.author_command_admissions
                SET challenge_expires_at = challenge_consumed_at
              WHERE author_command_admission_id = '{admission}';
             UPDATE storyos.project_command_challenges SET expires_at = consumed_at
              WHERE command_kind = 'applyAuthorEdit' AND idempotency_key = '{key}'",
            admission = command.ids.author_command_admission_id,
            key = command.challenge_binding.idempotency_key,
        ),
    )
    .await;
    let expected = storyos_application::ApplyAuthorEditOutcome::RequiresReconfirmation(
        storyos_application::RequiresReconfirmationApplyAuthorEdit {
            command_id: command.ids.command_id.clone(),
            author_command_admission_id: command.ids.author_command_admission_id.clone(),
            reconfirmation_reason:
                storyos_application::ApplyAuthorEditReconfirmationReason::AdmissionExpired,
            recovery_draft_ref: None,
        },
    );
    assert_eq!(outcome(&store, &command).await, expected);
    assert_eq!(outcome(&store, &command).await, expected);
    // The Admission has one reconfirmation, no Receipt, and no Receipt settlement.
    let row = admin
        .query_one(
            "SELECT (SELECT count(*) FROM storyos.domain_receipts
                      WHERE author_command_admission_id = $1::text::uuid),
                    (SELECT count(*) FROM storyos.author_command_admission_reconfirmations
                      WHERE author_command_admission_id = $1::text::uuid)",
            &[&command.ids.author_command_admission_id],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            row.get::<_, i64>(/*idx*/ 0),
            row.get::<_, i64>(/*idx*/ 1),
            admission_state(&admin, &command).await
        ),
        (0, 1, (1, 0, "settled".to_owned()))
    );
}
