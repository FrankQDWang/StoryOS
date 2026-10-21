//! Author Undo of a Proposal decision after the Undo of later Proposal actions (ADR 0068). Each
//! Undo takes the current Author Undo Frontier, as the Web Client does.

use storyos_application::{
    EditorSessionId, RejectProposalOperationsInput, RejectionNote, ReopenRejectedOperationsInput,
    ReopenWithdrawnProposalInput, ReplanProposalInput, UndoLatestAuthorActionSettlement,
};
use storyos_core::{TransitionOutcome, UndoLatestAuthorActionUnavailable};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;

use super::proposal_decision::{
    REJECT_PROPOSAL_OPERATIONS, REOPEN_REJECTED_OPERATIONS, REOPEN_WITHDRAWN_PROPOSAL,
    REPLAN_PROPOSAL, reject_proposal_operations, reopen_rejected_operations,
    reopen_withdrawn_proposal, replan_proposal,
};
use super::proposal_undo_chain::{
    compensated, edit, head_closure, on, undo_call, undo_latest, withdraw,
};
use super::secondary_operation::{
    TwoOperationProposal, operation_candidates, proposal_head_revision, two_operation_proposal,
};
use super::support::{issued, run_without_foreign_keys, stores, with_new_request_ids};
use super::undo::{latest_forward, undo};

/// The state axes, the candidate, and the base of the current head of one Proposal.
async fn head_state(admin: &Client, proposal_id: &str) -> [String; 5] {
    let row = admin
        .query_one(
            "SELECT revision.generation, revision.validation, revision.closure,
                    revision.candidate_text, revision.base_authoritative_revision_id::text
               FROM storyos.proposal_heads AS head
               JOIN storyos.proposal_revisions AS revision
                 ON (revision.proposal_id, revision.revision_id) =
                    (head.proposal_id, head.current_revision_id)
              WHERE head.proposal_id = $1::text::uuid",
            &[&proposal_id],
        )
        .await
        .unwrap();
    std::array::from_fn(|idx| row.get(idx))
}

/// The resolution and the reservation state of one Proposal Operation.
async fn operation_state(admin: &Client, operation_id: &str) -> (String, String) {
    let row = admin
        .query_one(
            "SELECT resolution, reservation_state FROM storyos.proposal_operations
              WHERE operation_id = $1::text::uuid",
            &[&operation_id],
        )
        .await
        .unwrap();
    (row.get(/*idx*/ 0), row.get(/*idx*/ 1))
}

/// Undo of the current Author Undo Frontier and an exact retry of the same Undo.
async fn undo_and_retry(
    store: &PostgresProjectReader,
    admin: &Client,
    proposal: &TwoOperationProposal,
    suffix: u16,
) -> (
    UndoLatestAuthorActionSettlement,
    UndoLatestAuthorActionSettlement,
) {
    let call = undo_call(
        store,
        admin,
        &proposal.scope,
        &proposal.editor_session_id,
        &proposal.chapter_revision_id,
        suffix,
    )
    .await;
    let settled = undo(store, &call).await.unwrap();
    let retried = undo(store, &with_new_request_ids(&call)).await.unwrap();
    (settled, retried)
}

/// Gives the current head of the Proposal a Proposal Conflict condition, as data from before
/// ADR 0064 has. Then replans it on the same Chapter head. Returns the Replan and the head state
/// before it.
async fn conflict_and_replan(
    store: &PostgresProjectReader,
    admin: &Client,
    proposal: &TwoOperationProposal,
    suffix: u16,
) -> (u64, [String; 5]) {
    let head = proposal_head_revision(admin, &proposal.proposal_id).await;
    let conflict_id = Uuid::now_v7().to_string();
    run_without_foreign_keys(
        admin,
        &format!(
            "UPDATE storyos.proposal_revisions SET validation = 'conflicted'
              WHERE revision_id = '{head}';
             INSERT INTO storyos.proposal_validation_conditions
               (owner_user_id, project_id, proposal_id, proposal_revision_id,
                acceptance_receipt_id, validation, conflict_id, condition_kind)
             VALUES ('{owner}', '{project}', '{proposal}', '{head}', '{receipt}',
                     'conflicted', '{conflict_id}', 'proposal_conflict')",
            owner = proposal.scope.owner_user_id.as_ref(),
            project = proposal.scope.project_id.as_ref(),
            proposal = proposal.proposal_id,
            receipt = Uuid::now_v7(),
        ),
    )
    .await;
    let conflicted = head_state(admin, &proposal.proposal_id).await;
    let input = ReplanProposalInput {
        editor_session_id: EditorSessionId::new(proposal.editor_session_id.clone()),
        proposal_id: proposal.proposal_id.clone(),
        conflicted_proposal_revision_id: head.clone(),
        expected_current_proposal_head: head,
        expected_authoritative_revision_id: proposal.chapter_revision_id.clone(),
        replacement_operation_id: proposal.operations[0].0.clone(),
        source_condition: storyos_contracts::ReplanSourceCondition::ProposalConflict {
            proposal_conflict_ref: conflict_id,
        },
    };
    let call = issued(store, &proposal.scope, suffix, &REPLAN_PROPOSAL, input).await;
    replan_proposal(store, &call).await.unwrap();
    (latest_forward(admin, &proposal.scope).await, conflicted)
}

/// Replan R, candidate edit E, Undo E, and then Undo R restores the Proposal before R.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_restores_a_replan_after_the_undo_of_a_later_candidate_edit() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4c00).await;
    let prior = latest_forward(&admin, &proposal.scope).await;
    let candidates = operation_candidates(&admin, &proposal).await;
    let (replanned, before_replan) =
        conflict_and_replan(&store, &admin, &proposal, /*suffix*/ 0x4c09).await;
    let edited = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 0, /*suffix*/ 0x4c0a, /*local_intent_sequence*/ 3,
        ),
    )
    .await;
    let undo_edit = compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4c0b).await);
    let (undo_replan, retried) = undo_and_retry(&store, &admin, &proposal, /*suffix*/ 0x4c0c).await;

    assert_eq!(
        (
            undo_edit,
            compensated(&undo_replan),
            &retried,
            head_state(&admin, &proposal.proposal_id).await,
            operation_candidates(&admin, &proposal).await
        ),
        (
            (edited, Some(replanned)),
            (replanned, Some(prior)),
            &undo_replan,
            before_replan,
            candidates
        )
    );
}

/// Rejection X, reopen D of the rejected Operation, candidate edit E of that Operation, Undo E,
/// and then Undo D rejects the Operation again. The Undo after it stops at the Barrier X.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_rejects_a_reopened_operation_after_the_undo_of_its_candidate_edit() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4c40).await;
    let candidates = operation_candidates(&admin, &proposal).await;
    let operation_id = proposal.operations[1].0.clone();
    let rejection = issued(
        &store,
        &proposal.scope,
        /*suffix*/ 0x4c49,
        &REJECT_PROPOSAL_OPERATIONS,
        RejectProposalOperationsInput {
            editor_session_id: EditorSessionId::new(proposal.editor_session_id.clone()),
            proposal_id: proposal.proposal_id.clone(),
            proposal_revision_id: proposal_head_revision(&admin, &proposal.proposal_id).await,
            selected_pending_operation_ids: vec![operation_id.clone()],
            expected_authoritative_revision_id: proposal.chapter_revision_id.clone(),
            rejection_note: RejectionNote::Omitted,
        },
    )
    .await;
    reject_proposal_operations(&store, &rejection)
        .await
        .unwrap();
    let barrier = latest_forward(&admin, &proposal.scope).await;
    let rejected = operation_state(&admin, &operation_id).await;
    let rejection_event_id: String = admin
        .query_one(
            "SELECT resolution_event_id::text FROM storyos.proposal_operation_resolutions
              WHERE operation_id = $1::text::uuid",
            &[&operation_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let reopen = issued(
        &store,
        &proposal.scope,
        /*suffix*/ 0x4c4a,
        &REOPEN_REJECTED_OPERATIONS,
        ReopenRejectedOperationsInput {
            editor_session_id: EditorSessionId::new(proposal.editor_session_id.clone()),
            proposal_id: proposal.proposal_id.clone(),
            proposal_revision_id: proposal_head_revision(&admin, &proposal.proposal_id).await,
            selected_rejected_operation_id: operation_id.clone(),
            rejection_event_id,
            expected_authoritative_revision_id: proposal.chapter_revision_id.clone(),
        },
    )
    .await;
    reopen_rejected_operations(&store, &reopen).await.unwrap();
    let reopened = latest_forward(&admin, &proposal.scope).await;
    let edited = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4c4b, /*local_intent_sequence*/ 3,
        ),
    )
    .await;
    let undo_edit = compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4c4c).await);
    let (undo_reopen, retried) = undo_and_retry(&store, &admin, &proposal, /*suffix*/ 0x4c4d).await;
    let stopped = undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4c4e)
        .await
        .outcome;

    assert_eq!(
        (
            undo_edit,
            compensated(&undo_reopen),
            &retried,
            stopped,
            operation_state(&admin, &operation_id).await,
            operation_candidates(&admin, &proposal).await
        ),
        (
            (edited, Some(reopened)),
            (reopened, Some(barrier)),
            &undo_reopen,
            TransitionOutcome::Refused(UndoLatestAuthorActionUnavailable::Barrier),
            rejected,
            candidates
        )
    );
}

/// Withdrawal W, reopen D, candidate edit E, Undo E, and then Undo D withdraws the Proposal
/// again. The Undo of W after it goes through the Compensation of D and reopens the Proposal.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_withdraws_a_reopened_proposal_after_the_undo_of_a_later_candidate_edit() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4c80).await;
    let candidates = operation_candidates(&admin, &proposal).await;
    let prior = latest_forward(&admin, &proposal.scope).await;
    let withdrawn = withdraw(&store, &admin, &proposal, /*suffix*/ 0x4c89).await;
    let withdrawal_event_id: String = admin
        .query_one(
            "SELECT withdrawal_event_id::text FROM storyos.proposal_withdrawals
              WHERE proposal_id = $1::text::uuid",
            &[&proposal.proposal_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let reopen = issued(
        &store,
        &proposal.scope,
        /*suffix*/ 0x4c8a,
        &REOPEN_WITHDRAWN_PROPOSAL,
        ReopenWithdrawnProposalInput {
            editor_session_id: EditorSessionId::new(proposal.editor_session_id.clone()),
            proposal_id: proposal.proposal_id.clone(),
            proposal_revision_id: proposal_head_revision(&admin, &proposal.proposal_id).await,
            withdrawal_event_id,
            expected_authoritative_revision_id: proposal.chapter_revision_id.clone(),
        },
    )
    .await;
    reopen_withdrawn_proposal(&store, &reopen).await.unwrap();
    let reopened = latest_forward(&admin, &proposal.scope).await;
    let edited = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4c8b, /*local_intent_sequence*/ 3,
        ),
    )
    .await;
    let undo_edit = compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4c8c).await);
    let (undo_reopen, retried) = undo_and_retry(&store, &admin, &proposal, /*suffix*/ 0x4c8d).await;
    let closure_after_undo = head_closure(&admin, &proposal.proposal_id).await;
    let undo_withdrawal =
        compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4c8e).await);

    assert_eq!(
        (
            [undo_edit, compensated(&undo_reopen), undo_withdrawal],
            &retried,
            [
                closure_after_undo,
                head_closure(&admin, &proposal.proposal_id).await
            ],
            operation_candidates(&admin, &proposal).await
        ),
        (
            [
                (edited, Some(reopened)),
                (reopened, Some(withdrawn)),
                (withdrawn, Some(prior))
            ],
            &undo_reopen,
            ["withdrawn".to_owned(), "open".to_owned()],
            candidates
        )
    );
}

/// The target of a Replan Undo is two Compensations below the head. Undo E appended C1, and the
/// author withdrew C1 in place. Undo W appended C2. Undo R still restores the Proposal.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_restores_a_replan_two_compensations_below_the_head() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4cc0).await;
    let prior = latest_forward(&admin, &proposal.scope).await;
    let candidates = operation_candidates(&admin, &proposal).await;
    let (replanned, before_replan) =
        conflict_and_replan(&store, &admin, &proposal, /*suffix*/ 0x4cc9).await;
    let edited = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 0, /*suffix*/ 0x4cca, /*local_intent_sequence*/ 3,
        ),
    )
    .await;
    let mut steps = vec![compensated(
        &undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4ccb).await,
    )];
    let withdrawn = withdraw(&store, &admin, &proposal, /*suffix*/ 0x4ccc).await;
    for suffix in [0x4ccd, 0x4cce] {
        steps.push(compensated(
            &undo_latest(&store, &admin, &proposal, suffix).await,
        ));
    }

    assert_eq!(
        (
            steps,
            head_state(&admin, &proposal.proposal_id).await,
            operation_candidates(&admin, &proposal).await
        ),
        (
            vec![
                (edited, Some(replanned)),
                (withdrawn, Some(replanned)),
                (replanned, Some(prior))
            ],
            before_replan,
            candidates
        )
    );
}
