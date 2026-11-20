use storyos_application::{EditorSessionId, UndoLatestAuthorActionInput, UpdateProjectInput};
use storyos_core::{ReceiptResult, TransitionOutcome};
use tokio_postgres::Client;

use crate::shared_tests::create_volume_authority::{NamedEdit, apply_named_edit};
use crate::undo_latest_author_action::UndoLatestAuthorAction;

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_damaged_evidence::{
    ReplayError, damaged_replay, evidence_replays,
};
use crate::shared_tests::command_sequence_project_session::UPDATE_PROJECT;
use crate::shared_tests::command_sequence_retry::in_progress_retry;
use crate::shared_tests::command_sequence_rollback::failed_then_settled;
use crate::shared_tests::command_sequence_support::{
    CommandCall, issued, replayed_outcome, stores, two_chapter_writer,
};
use crate::shared_tests::command_sequence_undo::{
    Case, issued_undo, latest_forward, session_chapter_head, undo, undo_call,
};

/// The cases whose Author Undo applies, in the order of their result rows.
const APPLIED: [Case; 11] = [
    Case::CreateVolume,
    Case::SetCurrentChapter,
    Case::WithdrawProposal,
    Case::ReplanProposal,
    Case::ReopenRejectedOperations,
    Case::ReopenWithdrawnProposal,
    Case::ApplyAuthorEdit,
    Case::EditProposalCandidate,
    Case::EditSecondaryCandidate,
    Case::AcceptProposal,
    Case::AcceptanceReversal,
];

/// The cases whose Author Undo has a zero-authority outcome.
const ZERO_AUTHORITY: [Case; 4] = [
    Case::StaleFrontier,
    Case::Barrier,
    Case::AcceptanceWrongHead,
    Case::AcceptanceUnavailable,
];

/// All cases, applied first.
fn all_cases() -> impl Iterator<Item = Case> {
    APPLIED.into_iter().chain(ZERO_AUTHORITY)
}

/// Counts the `undo_acceptance_receipts` rows of one Author Undo Receipt.
async fn undo_acceptance_rows(admin: &Client, receipt_id: &str) -> i64 {
    admin
        .query_one(
            "SELECT count(*) FROM storyos.undo_acceptance_receipts
              WHERE author_undo_receipt_id = $1::text::uuid",
            &[&receipt_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

fn plain(call: &CommandCall<UndoLatestAuthorAction>) -> CommandCall<UndoLatestAuthorActionInput> {
    CommandCall {
        envelope: call.envelope.clone(),
        input: call.input.input.clone(),
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_undo_outcome_replays_its_first_settlement_and_writes_only_its_records() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    let mut acceptance_children = Vec::new();
    for (case, base) in all_cases().zip((0x3000..).step_by(/*step*/ 0x10)) {
        let call = plain(&undo_call(&store, &admin, case, base).await);
        observed.push(replayed_outcome(&store, &admin, &call, undo).await);
        acceptance_children.push(undo_acceptance_rows(&admin, &call.envelope.ids.receipt_id).await);
    }

    // Receipt, Author Action, Activity, Commit, and Snapshot rows of each outcome. A Structure
    // or Current Chapter Compensation writes a canonical Snapshot and no Activity event. The
    // Snapshot count joins the Activity rows, so it is zero.
    let structure = ("authoritative_applied", [1, 1, 0, 1, 0]);
    let current_chapter = ("authoritative_applied", [1, 1, 0, 0, 0]);
    let proposal = ("authoritative_applied", [1, 1, 0, 0, 0]);
    let revision = ("authoritative_applied", [1, 1, 1, 1, 1]);
    // The Receipt of a Reversal records `authoritative_applied` and a Forward Author Action, as
    // on `main`.
    let reversal = ("authoritative_applied", [1, 1, 0, 0, 0]);
    let conflicted = ("conflicted", [1, 0, 0, 0, 0]);
    let refused = ("refused", [1, 0, 0, 0, 0]);
    assert_eq!(
        observed
            .iter()
            .map(|(result_kind, rows)| (result_kind.as_str(), *rows))
            .collect::<Vec<_>>(),
        vec![
            structure,
            current_chapter,
            proposal,
            proposal,
            proposal,
            proposal,
            revision,
            proposal,
            proposal,
            revision,
            reversal,
            conflicted,
            refused,
            conflicted,
            refused,
        ]
    );
    // The `undo_acceptance_receipts` row of each outcome. An Acceptance conflict writes none, as
    // on `main`.
    assert_eq!(
        acceptance_children,
        vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 1]
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_failing_undo_step_rolls_back_every_row_and_keeps_the_challenge_unused() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    for (case, base) in all_cases().zip((0x3200..).step_by(/*step*/ 0x10)) {
        let call = undo_call(&store, &admin, case, base).await;
        observed.push(failed_then_settled(&store, &admin, &call).await);
    }
    let rolled_back = |result, after_classify: [&'static str; 2]| {
        let failures = std::iter::once("classify")
            .chain(after_classify)
            .map(|step| (Some(step), [0; 5]))
            .collect::<Vec<_>>();
        (failures, result)
    };
    let applied = rolled_back(
        ReceiptResult::AuthoritativeApplied,
        ["apply", "after authority"],
    );
    // A refused Author Undo reloads its frontier in the zero-authority write step. A conflicted
    // one writes only its Receipt, so only its classify step can fail. The first settlement
    // after that failure is real, and the next one replays it.
    let refused = rolled_back(ReceiptResult::Refused, ["zero-authority write"; 2]);
    let conflicted = |result| {
        (
            vec![
                (Some("classify"), [0; 5]),
                (None, [1, 0, 0, 0, 0]),
                (None, [1, 0, 0, 0, 0]),
            ],
            result,
        )
    };
    let mut expected = vec![applied; APPLIED.len()];
    expected.extend([
        conflicted(ReceiptResult::Conflicted),
        refused.clone(),
        conflicted(ReceiptResult::Conflicted),
        refused,
    ]);
    assert_eq!(observed, expected);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_in_progress_undo_retry_conflicts_and_writes_no_row() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    for (case, base) in all_cases().zip((0x3400..).step_by(/*step*/ 0x10)) {
        let call = undo_call(&store, &admin, case, base).await;
        observed.push(in_progress_retry(&store, &admin, &call).await);
    }
    assert_eq!(
        observed,
        vec![(true, [0; 5]); APPLIED.len() + ZERO_AUTHORITY.len()]
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_undo_replay_separates_pre_capture_from_damaged_evidence() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    for (case, base) in all_cases().zip((0x3600..).step_by(/*step*/ 0x10)) {
        let call = undo_call(&store, &admin, case, base).await;
        observed.push(evidence_replays(&store, &admin, &call).await);
    }
    let pre_capture_then_damaged = [
        ReplayError::HistoricalAcknowledgementUnavailable,
        ReplayError::Unavailable,
    ];
    let mut expected = vec![
        (
            ReceiptResult::AuthoritativeApplied,
            pre_capture_then_damaged.clone()
        );
        APPLIED.len()
    ];
    expected.extend(
        [
            ReceiptResult::Conflicted,
            ReceiptResult::Refused,
            ReceiptResult::Conflicted,
            ReceiptResult::Refused,
        ]
        .map(|result| (result, pre_capture_then_damaged.clone())),
    );
    assert_eq!(observed, expected);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_damaged_undo_replay_is_a_store_fault() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    for (case, base, damage) in [
        (
            Case::CreateVolume,
            0x3e00,
            (|receipt_id: &str| {
                format!(
                    "DELETE FROM storyos.project_snapshots AS snapshot
                      USING storyos.domain_receipts AS receipt
                      WHERE receipt.receipt_id = '{receipt_id}'
                        AND snapshot.project_id = receipt.project_id
                        AND snapshot.snapshot_kind = 'canonical'
                        AND snapshot.created_at >= receipt.created_at"
                )
            }) as fn(&str) -> String,
        ),
        (Case::AcceptProposal, 0x3e10, |receipt_id: &str| {
            format!("DELETE FROM storyos.authoritative_commits WHERE receipt_id = '{receipt_id}'")
        }),
        (Case::SetCurrentChapter, 0x3e20, |receipt_id: &str| {
            format!(
                "UPDATE storyos.author_action_entries SET disposition = 'forward',
                        compensated_source_sequence = NULL
                  WHERE receipt_id = '{receipt_id}'"
            )
        }),
    ] {
        let call = undo_call(&store, &admin, case, base).await;
        observed.push(damaged_replay(&store, &admin, &call, damage).await);
    }
    assert_eq!(observed, vec![ReplayError::Unavailable; 3]);
}

/// A Project setting command writes no Author Action (ADR 0076). Thus Author Undo after a
/// rename compensates the earlier prose edit and keeps the new title.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_after_a_rename_compensates_the_earlier_prose_edit_and_keeps_the_title() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_a, _chapter_b, _revision_b, editor_session_id) =
        two_chapter_writer(&store, /*base*/ 0xe110).await;
    let prior_head = session_chapter_head(&admin, &editor_session_id).await;
    apply_named_edit(
        &store,
        &scope,
        NamedEdit {
            editor_session_id: &editor_session_id,
            chapter_id: &chapter_a,
            expected_revision_id: &prior_head,
            suffix: "e119",
            local_intent_sequence: 1,
            text: "Keep this sentence.",
            proposal_target: None,
        },
    )
    .await;
    let edited_head = session_chapter_head(&admin, &editor_session_id).await;
    let edit_sequence = latest_forward(&admin, &scope).await;
    let rename = issued(
        &store,
        &scope,
        /*suffix*/ 0xe11a,
        &UPDATE_PROJECT,
        UpdateProjectInput {
            title: "Renamed".to_owned(),
            expected_revision: 1,
        },
    )
    .await;
    store
        .update_project(&rename.envelope, &rename.input)
        .await
        .unwrap();
    let call = issued_undo(
        &store,
        &scope,
        /*suffix*/ 0xe11b,
        UndoLatestAuthorActionInput {
            editor_session_id: EditorSessionId::new(editor_session_id.clone()),
            expected_author_undo_frontier_sequence: latest_forward(&admin, &scope).await,
            expected_authoritative_revision_id: edited_head,
        },
    )
    .await;
    let source_sequence = match undo(&store, &call).await.unwrap().outcome {
        TransitionOutcome::Applied(applied) => applied.source_sequence,
        other => panic!("the Undo must compensate the prose edit, got {other:?}"),
    };
    let title: String = admin
        .query_one(
            "SELECT title FROM storyos.projects WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    assert_eq!(
        (source_sequence, title.as_str()),
        (edit_sequence, "Renamed")
    );
}
