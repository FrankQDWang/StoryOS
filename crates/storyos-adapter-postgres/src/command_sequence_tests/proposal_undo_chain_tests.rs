//! Author Undo of a chain of Proposal candidate edits and withdrawals (ADR 0068). Each Undo
//! takes the current Author Undo Frontier, as the Web Client does.

use storyos_application::{
    EditorSessionId, ProjectScope, RejectProposalOperationsInput, RejectionNote,
    ReopenWithdrawnProposalInput, UndoLatestAuthorActionInput, UndoLatestAuthorActionSettlement,
    WithdrawProposalInput, WithdrawalNote,
};
use storyos_core::{TransitionOutcome, UndoLatestAuthorActionUnavailable};
use tokio_postgres::Client;

use crate::PostgresProjectReader;

use super::proposal_decision::{
    REJECT_PROPOSAL_OPERATIONS, REOPEN_WITHDRAWN_PROPOSAL, WITHDRAW_PROPOSAL,
    reject_proposal_operations, reopen_withdrawn_proposal, withdraw_proposal,
    withdraw_proposal_call,
};
use super::secondary_operation::{
    CandidateEdit, TwoOperationProposal, edit_operation_candidate, operation_candidates,
    proposal_head, two_operation_proposal,
};
use super::support::{CommandCall, issued, run_without_foreign_keys, stores, with_new_request_ids};
use super::undo::{issued_undo, latest_forward, undo};

/// Issues one Author Undo of the current Author Undo Frontier.
async fn undo_call(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    session: &str,
    expected_head: &str,
    suffix: u16,
) -> CommandCall<UndoLatestAuthorActionInput> {
    let input = UndoLatestAuthorActionInput {
        editor_session_id: EditorSessionId::new(session),
        expected_author_undo_frontier_sequence: latest_forward(admin, scope).await,
        expected_authoritative_revision_id: expected_head.to_owned(),
    };
    issued_undo(store, scope, suffix, input).await
}

/// One Author Undo of the current Author Undo Frontier of the Proposal fixture.
async fn undo_latest(
    store: &PostgresProjectReader,
    admin: &Client,
    proposal: &TwoOperationProposal,
    suffix: u16,
) -> UndoLatestAuthorActionSettlement {
    let call = undo_call(
        store,
        admin,
        &proposal.scope,
        &proposal.editor_session_id,
        &proposal.chapter_revision_id,
        suffix,
    )
    .await;
    undo(store, &call).await.unwrap()
}

/// The compensated source of an applied Author Undo and the Frontier after it.
fn compensated(settlement: &UndoLatestAuthorActionSettlement) -> (u64, Option<u64>) {
    match &settlement.outcome {
        TransitionOutcome::Applied(applied) => (
            applied.source_sequence,
            applied.author_undo_frontier_sequence,
        ),
        other => panic!("the Undo must compensate, got {other:?}"),
    }
}

/// Prepends `text` to the candidate of one Operation and returns the Forward action.
async fn edit(
    store: &PostgresProjectReader,
    admin: &Client,
    proposal: &TwoOperationProposal,
    edit: CandidateEdit<'_>,
) -> u64 {
    edit_operation_candidate(store, admin, proposal, edit).await;
    latest_forward(admin, &proposal.scope).await
}

/// The current head of one Proposal.
async fn head_revision(admin: &Client, proposal_id: &str) -> String {
    admin
        .query_one(
            "SELECT current_revision_id::text FROM storyos.proposal_heads
              WHERE proposal_id = $1::text::uuid",
            &[&proposal_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// The closure of the current head of one Proposal.
async fn head_closure(admin: &Client, proposal_id: &str) -> String {
    admin
        .query_one(
            "SELECT revision.closure
               FROM storyos.proposal_heads AS head
               JOIN storyos.proposal_revisions AS revision
                 ON (revision.proposal_id, revision.revision_id) =
                    (head.proposal_id, head.current_revision_id)
              WHERE head.proposal_id = $1::text::uuid",
            &[&proposal_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// The author Withdrawal of the current head of the Proposal fixture.
async fn withdraw(
    store: &PostgresProjectReader,
    admin: &Client,
    proposal: &TwoOperationProposal,
    suffix: u16,
) -> u64 {
    let input = WithdrawProposalInput {
        editor_session_id: EditorSessionId::new(proposal.editor_session_id.clone()),
        proposal_id: proposal.proposal_id.clone(),
        proposal_revision_id: proposal_head(admin, &proposal.proposal_id).await.0,
        expected_authoritative_revision_id: proposal.chapter_revision_id.clone(),
        withdrawal_note: WithdrawalNote::Omitted,
    };
    let call = issued(store, &proposal.scope, suffix, &WITHDRAW_PROPOSAL, input).await;
    withdraw_proposal(store, &call).await.unwrap();
    latest_forward(admin, &proposal.scope).await
}

const fn on(operation: usize, suffix: u16, local_intent_sequence: u64) -> CandidateEdit<'static> {
    CandidateEdit {
        operation,
        suffix,
        local_intent_sequence,
        text: "Edited ",
    }
}

/// Undo E2 and then Undo E1 restore the candidate before E1. An exact retry of the second Undo
/// replays its first settlement.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_reverses_two_candidate_edits_and_replays_the_second_undo() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4a00).await;
    let before = operation_candidates(&admin, &proposal).await;
    let prior = latest_forward(&admin, &proposal.scope).await;
    let first = edit(&store, &admin, &proposal, on(1, 0x4a09, 3)).await;
    let second = edit(&store, &admin, &proposal, on(1, 0x4a0a, 4)).await;
    let undo_second = compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4a0b).await);
    let call = undo_call(
        &store,
        &admin,
        &proposal.scope,
        &proposal.editor_session_id,
        &proposal.chapter_revision_id,
        /*suffix*/ 0x4a0c,
    )
    .await;
    let undo_first = undo(&store, &call).await.unwrap();
    let retried = undo(&store, &with_new_request_ids(&call)).await.unwrap();

    assert_eq!(
        (
            undo_second,
            compensated(&undo_first),
            &retried,
            operation_candidates(&admin, &proposal).await
        ),
        (
            (second, Some(first)),
            (first, Some(prior)),
            &undo_first,
            before
        )
    );
}

/// Three candidate edits of both Operations, undone in reverse order, restore both candidates.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_reverses_three_candidate_edits_in_reverse_order() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4a20).await;
    let before = operation_candidates(&admin, &proposal).await;
    let prior = latest_forward(&admin, &proposal.scope).await;
    let first = edit(&store, &admin, &proposal, on(1, 0x4a29, 3)).await;
    let second = edit(&store, &admin, &proposal, on(0, 0x4a2a, 4)).await;
    let third = edit(&store, &admin, &proposal, on(1, 0x4a2b, 5)).await;

    let mut steps = Vec::new();
    for suffix in [0x4a2c, 0x4a2d, 0x4a2e] {
        steps.push(compensated(
            &undo_latest(&store, &admin, &proposal, suffix).await,
        ));
    }

    assert_eq!(
        (steps, operation_candidates(&admin, &proposal).await),
        (
            vec![
                (third, Some(second)),
                (second, Some(first)),
                (first, Some(prior)),
            ],
            before
        )
    );
}

/// The Undo of a Reopen withdraws the Proposal again in a new head. The Undo of the Withdrawal
/// then reopens the Proposal.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_reopens_a_withdrawal_after_the_undo_of_a_later_reopen() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let withdrawal = withdraw_proposal_call(&store, &admin, /*base*/ 0x4a40).await;
    let scope = withdrawal.envelope.project_scope.clone();
    let session = withdrawal.input.editor_session_id.as_ref().to_owned();
    let chapter_head = withdrawal.input.expected_authoritative_revision_id.clone();
    let proposal_id = withdrawal.input.proposal_id.clone();
    let prior = latest_forward(&admin, &scope).await;
    withdraw_proposal(&store, &withdrawal).await.unwrap();
    let withdrawn = latest_forward(&admin, &scope).await;
    let withdrawal_event_id: String = admin
        .query_one(
            "SELECT withdrawal_event_id::text FROM storyos.proposal_withdrawals
              WHERE proposal_id = $1::text::uuid",
            &[&proposal_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let reopen = issued(
        &store,
        &scope,
        /*suffix*/ 0x4a4a,
        &REOPEN_WITHDRAWN_PROPOSAL,
        ReopenWithdrawnProposalInput {
            editor_session_id: EditorSessionId::new(session.clone()),
            proposal_id: proposal_id.clone(),
            proposal_revision_id: head_revision(&admin, &proposal_id).await,
            withdrawal_event_id,
            expected_authoritative_revision_id: chapter_head.clone(),
        },
    )
    .await;
    reopen_withdrawn_proposal(&store, &reopen).await.unwrap();
    let reopened = latest_forward(&admin, &scope).await;

    let mut steps = Vec::new();
    let mut closures = Vec::new();
    for suffix in [0x4a4b, 0x4a4c] {
        let call = undo_call(&store, &admin, &scope, &session, &chapter_head, suffix).await;
        steps.push(compensated(&undo(&store, &call).await.unwrap()));
        closures.push(head_closure(&admin, &proposal_id).await);
    }

    assert_eq!(
        (steps, closures),
        (
            vec![(reopened, Some(withdrawn)), (withdrawn, Some(prior))],
            vec!["withdrawn".to_owned(), "open".to_owned()]
        )
    );
}

/// The Undo of a Withdrawal moves the Proposal head. The Undo of the earlier candidate edit
/// still restores its candidate.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_reverses_a_candidate_edit_after_the_undo_of_a_later_withdrawal() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4a60).await;
    let before = operation_candidates(&admin, &proposal).await;
    let prior = latest_forward(&admin, &proposal.scope).await;
    let edited = edit(&store, &admin, &proposal, on(1, 0x4a69, 3)).await;
    let withdrawn = withdraw(&store, &admin, &proposal, /*suffix*/ 0x4a6a).await;

    let mut steps = Vec::new();
    for suffix in [0x4a6b, 0x4a6c] {
        steps.push(compensated(
            &undo_latest(&store, &admin, &proposal, suffix).await,
        ));
    }

    assert_eq!(
        (
            steps,
            operation_candidates(&admin, &proposal).await,
            head_closure(&admin, &proposal.proposal_id).await
        ),
        (
            vec![(withdrawn, Some(edited)), (edited, Some(prior))],
            before,
            "open".to_owned()
        )
    );
}

/// A Barrier between candidate edits stops the chain after the later edits.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_barrier_between_candidate_edits_stops_the_chain() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4a80).await;
    edit(&store, &admin, &proposal, on(1, 0x4a89, 3)).await;
    let after_first = operation_candidates(&admin, &proposal).await;
    let rejection = issued(
        &store,
        &proposal.scope,
        /*suffix*/ 0x4a8a,
        &REJECT_PROPOSAL_OPERATIONS,
        RejectProposalOperationsInput {
            editor_session_id: EditorSessionId::new(proposal.editor_session_id.clone()),
            proposal_id: proposal.proposal_id.clone(),
            proposal_revision_id: proposal_head(&admin, &proposal.proposal_id).await.0,
            selected_pending_operation_ids: vec![proposal.operations[0].0.clone()],
            expected_authoritative_revision_id: proposal.chapter_revision_id.clone(),
            rejection_note: RejectionNote::Omitted,
        },
    )
    .await;
    reject_proposal_operations(&store, &rejection)
        .await
        .unwrap();
    let barrier = latest_forward(&admin, &proposal.scope).await;
    let second = edit(&store, &admin, &proposal, on(1, 0x4a8b, 4)).await;
    let third = edit(&store, &admin, &proposal, on(1, 0x4a8c, 5)).await;
    let mut steps = Vec::new();
    for suffix in [0x4a8d, 0x4a8e] {
        steps.push(compensated(
            &undo_latest(&store, &admin, &proposal, suffix).await,
        ));
    }
    let stopped = undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4a8f)
        .await
        .outcome;

    assert_eq!(
        (
            steps,
            stopped,
            operation_candidates(&admin, &proposal).await
        ),
        (
            vec![(third, Some(second)), (second, Some(barrier))],
            TransitionOutcome::Refused(UndoLatestAuthorActionUnavailable::Barrier),
            after_first
        )
    );
}

/// A Proposal head that a Compensation with no Receipt payload wrote is not explained by the
/// records. Thus the earlier candidate edit is a Barrier.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_proposal_head_that_the_records_cannot_explain_stops_the_chain() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4aa0).await;
    let first = edit(&store, &admin, &proposal, on(1, 0x4aa9, 3)).await;
    let second = edit(&store, &admin, &proposal, on(1, 0x4aaa, 4)).await;
    let third = edit(&store, &admin, &proposal, on(1, 0x4aab, 5)).await;
    let undo_third = compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4aac).await);
    let call = undo_call(
        &store,
        &admin,
        &proposal.scope,
        &proposal.editor_session_id,
        &proposal.chapter_revision_id,
        /*suffix*/ 0x4aad,
    )
    .await;
    let undo_second = compensated(&undo(&store, &call).await.unwrap());
    let after_first = operation_candidates(&admin, &proposal).await;
    // An earlier Proposal Compensation recorded no Receipt payload.
    run_without_foreign_keys(
        &admin,
        &format!(
            "UPDATE storyos.domain_receipts SET result_payload = '{{}}'::jsonb
              WHERE receipt_id = '{}'",
            call.envelope.ids.receipt_id
        ),
    )
    .await;
    let stopped = undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4aae)
        .await
        .outcome;

    assert_eq!(
        (
            [undo_third, undo_second],
            stopped,
            operation_candidates(&admin, &proposal).await
        ),
        (
            [(third, Some(second)), (second, Some(first))],
            TransitionOutcome::Refused(UndoLatestAuthorActionUnavailable::Barrier),
            after_first
        )
    );
}

/// Runs the Receipt trigger function of migration 0096 on one recorded Undo Receipt.
async fn check_undo_receipt(admin: &Client, receipt_id: &str) -> Result<(), String> {
    let checked = admin
        .batch_execute(&format!(
            "BEGIN;
             CREATE TEMPORARY TABLE receipt_probe
               (owner_user_id uuid, project_id uuid, receipt_id uuid) ON COMMIT DROP;
             CREATE TRIGGER receipt_probe_check AFTER INSERT ON receipt_probe
               FOR EACH ROW EXECUTE FUNCTION storyos.require_draft_retry_settlement();
             INSERT INTO receipt_probe
               SELECT owner_user_id, project_id, receipt_id FROM storyos.domain_receipts
                WHERE receipt_id = '{receipt_id}';
             COMMIT;"
        ))
        .await;
    if checked.is_err() {
        // A failed statement leaves the transaction open.
        admin.batch_execute("ROLLBACK").await.unwrap();
    }
    checked.map_err(|error| {
        let refusal = error
            .as_db_error()
            .map(|db| (db.code().code(), db.message()));
        format!("{refusal:?}")
    })
}

/// The Receipt trigger refuses a Proposal edit Compensation whose recorded source is not the
/// parent of its appended Revision.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_receipt_trigger_refuses_a_source_that_is_not_the_parent() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4ac0).await;
    let unedited = head_revision(&admin, &proposal.proposal_id).await;
    edit(&store, &admin, &proposal, on(1, 0x4ac9, 3)).await;
    let call = undo_call(
        &store,
        &admin,
        &proposal.scope,
        &proposal.editor_session_id,
        &proposal.chapter_revision_id,
        /*suffix*/ 0x4aca,
    )
    .await;
    compensated(&undo(&store, &call).await.unwrap());
    let receipt_id = call.envelope.ids.receipt_id.clone();
    let recorded = check_undo_receipt(&admin, &receipt_id).await;
    // The Revision before the edit is in the same Proposal, but it is not the parent.
    run_without_foreign_keys(
        &admin,
        &format!(
            "UPDATE storyos.domain_receipts
                SET result_payload = jsonb_set(result_payload, '{{source_proposal_revision_id}}',
                                               to_jsonb('{unedited}'::text))
              WHERE receipt_id = '{receipt_id}'"
        ),
    )
    .await;

    assert_eq!(
        (recorded, check_undo_receipt(&admin, &receipt_id).await),
        (
            Ok(()),
            Err(r#"Some(("23514", "Incomplete Proposal compensation identity"))"#.to_owned())
        )
    );
}
