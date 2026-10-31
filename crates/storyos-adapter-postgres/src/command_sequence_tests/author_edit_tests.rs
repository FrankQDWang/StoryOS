use storyos_application::{
    ApplyAuthorEditCommand, AuthorCommandAdmissionIds, AuthorEditError, AuthorEditProposalTarget,
    AuthorEditSettlement, AuthorEditSettlementEffect, EditorSessionId, UndoLatestAuthorActionInput,
};
use storyos_core::{AuthorEditConflict, TransitionOutcome};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::author_edit::AuthorEditFault;
use crate::create_volume_authority_tests::{NamedEdit, adjusted_edit_command, named_edit_command};

use super::acceptance::{acceptable_proposal, advance_chapter_head, seed_condition};
use super::agent_run::seed_run;
use super::secondary_operation::{
    BLOCK_EDIT, CandidateEdit, PRIMARY_TEXT, chapter_blocks, earlier_base_proposal, edit_block,
    edit_operation_candidate, one_operation_proposal, proposal_head_revision,
};
use super::support::{run_without_foreign_keys, settlement_rows, stores, two_chapter_writer};
use super::undo::{edit_in_chapter_of, issued_undo, latest_forward, session_chapter_head, undo};

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
}

const EDITS: [Edit; 5] = [
    Edit::Prose,
    Edit::ProposalCandidate,
    Edit::Unchanged,
    Edit::StaleHead,
    Edit::InvalidSelection,
];

/// The Receipt result kind and the Receipt, Author Action, Activity, Commit, and Snapshot rows of
/// each case, in the order of `EDITS`.
const RECORDS: [(&str, [i64; 5]); 5] = [
    ("authoritative_applied", [1, 1, 1, 1, 1]),
    ("proposal_revised", [1, 1, 0, 0, 0]),
    ("no_effect", [1, 0, 0, 0, 0]),
    ("conflicted", [1, 0, 0, 0, 0]),
    ("refused", [1, 0, 0, 0, 0]),
];

/// The Author Edit of `edit` in a new Project, with its issued Command Challenge.
async fn edit_command(
    store: &PostgresProjectReader,
    admin: &Client,
    edit: Edit,
    base: u16,
) -> ApplyAuthorEditCommand {
    let suffix = format!("{:04x}", base + 9);
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
        Edit::Prose | Edit::ProposalCandidate => {
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
    assert_eq!(observed, vec![true; 4]);
}

/// The CHECK constraints of the Receipt table that refuse a malformed Receipt shape.
const RECEIPT_SHAPE_CHECKS: [&str; 2] = [
    "domain_receipts_common_shape",
    "domain_receipts_result_shape",
];

/// Requires that the Receipt shape CHECK constraints refuse `damage`, replays `command` once past
/// them, and then restores the Receipt of `command` and the constraints.
async fn replay_past_receipt_checks(
    store: &PostgresProjectReader,
    admin: &Client,
    command: &ApplyAuthorEditCommand,
    damage: &str,
) -> Result<AuthorEditSettlement, AuthorEditError> {
    assert!(
        admin.batch_execute(damage).await.is_err(),
        "the Receipt shape checks must refuse: {damage}"
    );
    let mut restored = String::new();
    for name in RECEIPT_SHAPE_CHECKS {
        let definition: String = admin
            .query_one(
                "SELECT pg_get_constraintdef(oid) FROM pg_constraint
                  WHERE conrelid = 'storyos.domain_receipts'::regclass AND conname = $1",
                &[&name],
            )
            .await
            .unwrap()
            .get(/*idx*/ 0);
        restored.push_str(&format!(
            "ALTER TABLE storyos.domain_receipts ADD CONSTRAINT {name} {definition};"
        ));
    }
    let dropped = RECEIPT_SHAPE_CHECKS
        .map(|name| format!("ALTER TABLE storyos.domain_receipts DROP CONSTRAINT {name};"))
        .concat();
    run_without_foreign_keys(
        admin,
        &format!(
            "CREATE TEMP TABLE kept_receipt AS
               SELECT * FROM storyos.domain_receipts WHERE receipt_id = '{receipt_id}';
             {dropped} {damage}",
            receipt_id = command.ids.receipt_id,
        ),
    )
    .await;
    let replayed = store.apply_author_edit(&exact_retry(command)).await;
    run_without_foreign_keys(
        admin,
        &format!(
            "UPDATE storyos.domain_receipts AS receipt
                SET expected_heads = kept.expected_heads, prior_heads = kept.prior_heads,
                    resulting_heads = kept.resulting_heads,
                    authoritative_commit_ids = kept.authoritative_commit_ids,
                    result_payload = kept.result_payload
               FROM kept_receipt AS kept
              WHERE receipt.receipt_id = kept.receipt_id;
             DROP TABLE kept_receipt; {restored}"
        ),
    )
    .await;
    replayed
}

/// Gives the Commit and the Envelope of the Receipt `{receipt}` another prior Revision.
const CORRUPT_PRIOR_REVISION: &str = "
    UPDATE storyos.authoritative_revision_envelopes
       SET parent_revision_id = revision_id, payload_digest = 'sha256:corrupt'
     WHERE receipt_id = '{receipt}';
    UPDATE storyos.authoritative_commits SET prior_revision_id = resulting_revision_id
     WHERE receipt_id = '{receipt}'";

/// Where a damage row changes the stored records of a settled Author Edit.
#[derive(Clone, Copy, Debug)]
enum Damage {
    /// A change that the CHECK constraints permit.
    Records,
    /// A change of the Receipt shape that only a replay past the CHECK constraints can read.
    ReceiptShape,
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_replay_refuses_a_changed_binding_and_fails_on_a_damaged_receipt_shape() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let receipt = |set: &str| {
        format!("UPDATE storyos.domain_receipts SET {set} WHERE receipt_id = '{{receipt}}'")
    };
    let other_head = "ARRAY['018f0000-0000-7001-8000-0000000099ff'::uuid]";
    let rows: [(Edit, Damage, String); 13] = [
        (
            Edit::Prose,
            Damage::ReceiptShape,
            receipt(
                "authoritative_commit_ids =
                   array_fill(authoritative_commit_ids[1], ARRAY[1], ARRAY[0])",
            ),
        ),
        (
            Edit::Unchanged,
            Damage::ReceiptShape,
            receipt(
                "expected_heads = array_fill(expected_heads[1], ARRAY[1], ARRAY[0]),
                 prior_heads = array_fill(prior_heads[1], ARRAY[1], ARRAY[0]),
                 resulting_heads = array_fill(resulting_heads[1], ARRAY[1], ARRAY[0])",
            ),
        ),
        (
            Edit::Unchanged,
            Damage::ReceiptShape,
            receipt(
                "expected_heads = ARRAY[NULL]::uuid[], prior_heads = ARRAY[NULL]::uuid[],
                 resulting_heads = ARRAY[NULL]::uuid[]",
            ),
        ),
        (
            Edit::StaleHead,
            Damage::ReceiptShape,
            receipt(
                "prior_heads = expected_heads, resulting_heads = expected_heads,
                 result_payload = jsonb_build_object(
                   'reason', 'stale_authoritative_head',
                   'current_authoritative_revision_id', expected_heads[1]::text)",
            ),
        ),
        (
            Edit::StaleHead,
            Damage::ReceiptShape,
            receipt(
                "result_payload =
                   '{\"reason\":null,\"current_authoritative_revision_id\":null}'::jsonb",
            ),
        ),
        (
            Edit::InvalidSelection,
            Damage::ReceiptShape,
            receipt("result_payload = '{\"reason\":null}'::jsonb"),
        ),
        (
            Edit::Prose,
            Damage::Records,
            receipt("expected_heads = resulting_heads, prior_heads = resulting_heads"),
        ),
        (
            Edit::StaleHead,
            Damage::Records,
            receipt(&format!("expected_heads = {other_head}")),
        ),
        (
            Edit::Unchanged,
            Damage::Records,
            receipt(&format!(
                "expected_heads = {other_head}, prior_heads = {other_head},
                 resulting_heads = {other_head}"
            )),
        ),
        (
            Edit::InvalidSelection,
            Damage::Records,
            receipt(&format!(
                "expected_heads = {other_head}, prior_heads = {other_head},
                 resulting_heads = {other_head}"
            )),
        ),
        (
            Edit::Prose,
            Damage::Records,
            "UPDATE storyos.author_command_admissions
                SET target_refs = ARRAY['manuscript:corrupt']::text[]
              WHERE author_command_admission_id = (
                SELECT author_command_admission_id FROM storyos.domain_receipts
                 WHERE receipt_id = '{receipt}')"
                .to_owned(),
        ),
        (
            Edit::Prose,
            Damage::Records,
            CORRUPT_PRIOR_REVISION.to_owned(),
        ),
        (
            Edit::StaleHead,
            Damage::Records,
            "UPDATE storyos.authoritative_commits AS edit_commit
                SET author_command_admission_id = receipt.author_command_admission_id
               FROM storyos.domain_receipts AS receipt, storyos.domain_receipts AS applied
              WHERE receipt.receipt_id = '{receipt}'
                AND (applied.owner_user_id, applied.project_id, applied.command_kind) =
                    (receipt.owner_user_id, receipt.project_id, 'applyAuthorEdit')
                AND edit_commit.receipt_id = applied.receipt_id"
                .to_owned(),
        ),
    ];
    let mut observed = Vec::new();
    for ((edit, damage, statements), base) in rows.into_iter().zip((0x9900..).step_by(0x10)) {
        let command = edit_command(&store, &admin, edit, base).await;
        store.apply_author_edit(&command).await.unwrap();
        let statements = statements.replace("{receipt}", &command.ids.receipt_id);
        let replayed = match damage {
            Damage::ReceiptShape => {
                replay_past_receipt_checks(&store, &admin, &command, &statements).await
            }
            Damage::Records => {
                run_without_foreign_keys(&admin, &statements).await;
                store.apply_author_edit(&exact_retry(&command)).await
            }
        };
        observed.push(match replayed {
            Err(AuthorEditError::BindingConflict) => "binding_conflict",
            Err(AuthorEditError::Unavailable(_)) => "unavailable",
            other => panic!("the replay of {edit:?} must fail, got {other:?}"),
        });
    }
    assert_eq!(
        observed,
        [
            ["unavailable"; 4].as_slice(),
            &["binding_conflict"; 7],
            &["unavailable"; 2],
        ]
        .concat()
    );
}

/// The outcome query of one Author Edit, as the Server sends it after a cut.
async fn outcome_result(
    store: &PostgresProjectReader,
    command: &ApplyAuthorEditCommand,
) -> Result<
    storyos_application::ApplyAuthorEditOutcome,
    storyos_application::ApplyAuthorEditOutcomeResolveError,
> {
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
}

async fn outcome(
    store: &PostgresProjectReader,
    command: &ApplyAuthorEditCommand,
) -> storyos_application::ApplyAuthorEditOutcome {
    outcome_result(store, command).await.unwrap()
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
async fn the_outcome_query_settles_an_expired_or_unrecoverable_admission_as_requires_reconfirmation()
 {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    let mut expected = Vec::new();
    for (base, damage, reconfirmation_reason) in [
        (
            0x3c00,
            "UPDATE storyos.author_command_admissions
                SET challenge_expires_at = challenge_consumed_at
              WHERE author_command_admission_id = '{admission}';
             UPDATE storyos.project_command_challenges SET expires_at = consumed_at
              WHERE command_kind = 'applyAuthorEdit' AND idempotency_key = '{key}'",
            storyos_application::ApplyAuthorEditReconfirmationReason::AdmissionExpired,
        ),
        (
            0x99e0,
            "UPDATE storyos.author_command_admissions
                SET command_payload = command_payload - 'author_edit_units'
              WHERE author_command_admission_id = '{admission}'",
            storyos_application::ApplyAuthorEditReconfirmationReason::DirectEditIntentUnrecoverable,
        ),
    ] {
        let command = edit_command(&store, &admin, Edit::Prose, base).await;
        store
            .apply_author_edit_with_fault(&command, AuthorEditFault::AfterAdmissionBeforeCore)
            .await
            .expect_err("the cut must stop after the admit step");
        run_without_foreign_keys(
            &admin,
            &damage
                .replace("{admission}", &command.ids.author_command_admission_id)
                .replace("{key}", &command.challenge_binding.idempotency_key),
        )
        .await;
        let queried = [
            outcome(&store, &command).await,
            outcome(&store, &command).await,
        ];
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
        observed.push((
            queried,
            row.get::<_, i64>(/*idx*/ 0),
            row.get::<_, i64>(/*idx*/ 1),
            admission_state(&admin, &command).await,
        ));
        let requires_reconfirmation =
            storyos_application::ApplyAuthorEditOutcome::RequiresReconfirmation(
                storyos_application::RequiresReconfirmationApplyAuthorEdit {
                    command_id: command.ids.command_id.clone(),
                    author_command_admission_id: command.ids.author_command_admission_id.clone(),
                    reconfirmation_reason,
                },
            );
        // The Admission has one reconfirmation, no Receipt, and no Receipt settlement.
        expected.push((
            [requires_reconfirmation.clone(), requires_reconfirmation],
            0,
            1,
            (1, 0, "settled".to_owned()),
        ));
    }
    assert_eq!(observed, expected);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_outcome_query_fails_on_a_changed_command_payload_or_a_corrupt_commit() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    for (edit, base, damage) in [
        (
            Edit::Unchanged,
            0x9860,
            "UPDATE storyos.author_command_admissions
                SET command_payload = jsonb_set(
                  command_payload, '{author_edit_units,0,selection_snapshot,to}', '998'::jsonb,
                  false)
              WHERE author_command_admission_id = (
                SELECT author_command_admission_id FROM storyos.domain_receipts
                 WHERE receipt_id = '{receipt}')",
        ),
        (Edit::Prose, 0x99d0, CORRUPT_PRIOR_REVISION),
    ] {
        let command = edit_command(&store, &admin, edit, base).await;
        store.apply_author_edit(&command).await.unwrap();
        run_without_foreign_keys(
            &admin,
            &damage.replace("{receipt}", &command.ids.receipt_id),
        )
        .await;
        observed.push(
            outcome_result(&store, &command)
                .await
                .map_err(|error| error.to_string()),
        );
    }
    assert_eq!(
        observed,
        vec![Err("Apply Author Edit outcome resolution is unavailable".to_owned()); 2]
    );
}

/// A versioned edit of the Block that an open Block Proposal reserves conflicts and changes
/// nothing, also when it acknowledges the Proposal head. A versioned edit of another Block of the
/// Chapter applies.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_versioned_edit_conflicts_on_a_reserved_block_and_applies_on_another_block() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let proposal = one_operation_proposal(&store, &admin, /*base*/ 0x8a00).await;
    let proposal_revision = proposal_head_revision(&admin, &proposal.proposal_id).await;
    let before = chapter_blocks(&store, &proposal.scope, &proposal.chapter_id).await;
    let mut conflicts = Vec::new();
    for (suffix, local_intent_sequence, acknowledged) in [
        (0x8a09, 3, Vec::new()),
        (0x8a0a, 4, vec![proposal_revision.clone()]),
    ] {
        let edited = edit_block(
            &store,
            &proposal,
            /*block*/ 0,
            suffix,
            local_intent_sequence,
            acknowledged,
        )
        .await;
        conflicts.push(edited.effect);
    }
    let unchanged = (
        chapter_blocks(&store, &proposal.scope, &proposal.chapter_id).await,
        session_chapter_head(&admin, &proposal.editor_session_id).await,
        proposal_head_revision(&admin, &proposal.proposal_id).await,
    );
    let other = edit_block(
        &store,
        &proposal,
        /*block*/ 1,
        /*suffix*/ 0x8a0b,
        /*local_intent_sequence*/ 5,
        Vec::new(),
    )
    .await;
    let AuthorEditSettlementEffect::AuthoritativeApplied { blocks, .. } = other.effect else {
        panic!("the edit of the unreserved Block must apply, got {other:?}");
    };

    let conflicted = |reason| AuthorEditSettlementEffect::Conflicted {
        reason,
        current_authoritative_revision_id: proposal.chapter_revision_id.clone(),
    };
    let mut applied = before.clone();
    applied[1].1 = format!("{BLOCK_EDIT}{}", before[1].1);
    assert_eq!(
        (
            conflicts,
            unchanged,
            blocks
                .into_iter()
                .map(|block| (block.manuscript_block_id, block.text))
                .collect::<Vec<_>>(),
        ),
        (
            vec![
                conflicted(AuthorEditConflict::ProposalHeadPresent),
                conflicted(AuthorEditConflict::OwnershipChanged),
            ],
            (
                before,
                proposal.chapter_revision_id.clone(),
                proposal_revision
            ),
            applied,
        )
    );
}

/// The base, the candidate, the validation, and the Validation Receipt result of the current
/// Revision of one Proposal.
async fn head_validation(admin: &Client, proposal_id: &str) -> [String; 4] {
    let row = admin
        .query_one(
            "SELECT revision.base_authoritative_revision_id::text, revision.candidate_text,
                    revision.validation, receipt.result
               FROM storyos.proposal_heads AS head
               JOIN storyos.proposal_revisions AS revision
                 ON (revision.proposal_id, revision.revision_id) =
                    (head.proposal_id, head.current_revision_id)
               JOIN storyos.validation_receipts AS receipt
                 ON (receipt.proposal_id, receipt.proposal_revision_id) =
                    (revision.proposal_id, revision.revision_id)
              WHERE head.proposal_id = $1::text::uuid",
            &[&proposal_id],
        )
        .await
        .unwrap();
    std::array::from_fn(|idx| row.get(idx))
}

/// The state of an earlier-base Proposal before its candidate edit.
#[derive(Clone, Copy)]
enum EarlierCandidate {
    Plain,
    /// The current Revision has a Proposal Conflict condition.
    ProposalConflict,
    /// The Chapter head no longer has the target Block, which the base has.
    TargetRemoved,
}

/// A candidate edit on an earlier base keeps that base and is valid, also after a Proposal
/// Conflict condition and when the head no longer has the target Block. Author Undo of the edit
/// restores the prior candidate on that base.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_candidate_edit_on_an_earlier_base_is_valid_and_its_undo_keeps_the_base() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut edited = Vec::new();
    let mut bases = Vec::new();
    let mut undone = None;
    for (base, case) in [
        (0x8a20, EarlierCandidate::Plain),
        (0x8a40, EarlierCandidate::ProposalConflict),
        (0x8a60, EarlierCandidate::TargetRemoved),
    ] {
        let (mut proposal, earlier) = earlier_base_proposal(&store, &admin, base).await;
        match case {
            EarlierCandidate::Plain => {}
            EarlierCandidate::ProposalConflict => {
                let revision = proposal_head_revision(&admin, &proposal.proposal_id).await;
                seed_condition(
                    &admin,
                    &proposal.scope,
                    &proposal.proposal_id,
                    &revision,
                    "proposal_conflict",
                )
                .await;
            }
            EarlierCandidate::TargetRemoved => {
                let other =
                    chapter_blocks(&store, &proposal.scope, &proposal.chapter_id).await[1].clone();
                proposal.chapter_revision_id =
                    advance_chapter_head(&admin, &proposal.chapter_revision_id, &[other]).await;
            }
        }
        edit_operation_candidate(
            &store,
            &admin,
            &proposal,
            CandidateEdit {
                operation: 0,
                suffix: base + 9,
                local_intent_sequence: 4,
                text: "Edited ",
            },
        )
        .await;
        edited.push(head_validation(&admin, &proposal.proposal_id).await);
        bases.push(earlier);
        if let EarlierCandidate::Plain = case {
            let edit_sequence = latest_forward(&admin, &proposal.scope).await;
            let call = issued_undo(
                &store,
                &proposal.scope,
                base + 10,
                UndoLatestAuthorActionInput {
                    editor_session_id: EditorSessionId::new(proposal.editor_session_id.clone()),
                    expected_author_undo_frontier_sequence: edit_sequence,
                    expected_authoritative_revision_id: proposal.chapter_revision_id.clone(),
                },
            )
            .await;
            let source_sequence = match undo(&store, &call).await.unwrap().outcome {
                TransitionOutcome::Applied(applied) => applied.source_sequence,
                other => panic!("the Undo must compensate the candidate edit, got {other:?}"),
            };
            undone = Some((
                source_sequence == edit_sequence,
                head_validation(&admin, &proposal.proposal_id).await,
            ));
        }
    }

    let valid =
        |base: &str, candidate: &str| [base, candidate, "valid", "valid"].map(str::to_owned);
    let candidate = format!("Edited {PRIMARY_TEXT}");
    assert_eq!(
        (edited, undone),
        (
            bases
                .iter()
                .map(|base| valid(base, &candidate))
                .collect::<Vec<_>>(),
            Some((true, valid(&bases[0], PRIMARY_TEXT))),
        )
    );
}
