use storyos_application::{
    CreateVolumeInput, EditorSessionId, ProjectCommandError, ProjectScope,
    UndoLatestAuthorActionInput, UndoLatestAuthorActionSettlement,
};
use storyos_core::ReceiptResult;
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::create_volume_authority_tests::{NamedEdit, apply_named_edit};
use crate::undo_latest_author_action::UndoLatestAuthorAction;

use super::acceptance::{ACCEPT_PROPOSAL, accept_proposal, acceptable_proposal};
use super::agent_run::seed_run;
use super::damaged_evidence::{ReplayError, damaged_replay, evidence_replays};
use super::draft::{
    close_editor_flow_draft, close_editor_flow_draft_call, expand_refused_edit_draft,
    expand_refused_edit_draft_call,
};
use super::proposal_decision::{
    reject_proposal_operations, reject_proposal_operations_call, reopen_rejected_operations,
    reopen_rejected_operations_call, reopen_withdrawn_proposal, reopen_withdrawn_proposal_call,
    replan_proposal, replan_proposal_call, withdraw_proposal, withdraw_proposal_call,
};
use super::retry::in_progress_retry;
use super::rollback::failed_then_settled;
use super::structure::{CREATE_VOLUME, create_volume, set_current_chapter_call};
use super::support::{
    CommandCall, Route, issued, issued_with_bytes, replayed_outcome, run_without_foreign_keys,
    stores, two_chapter_writer,
};

pub(super) const UNDO_LATEST_AUTHOR_ACTION: Route = Route {
    kind: "undoLatestAuthorAction",
    method: storyos_contracts::UNDO_LATEST_AUTHOR_ACTION_METHOD,
    path: storyos_contracts::UNDO_LATEST_AUTHOR_ACTION_PATH,
    schema: storyos_contracts::UNDO_LATEST_AUTHOR_ACTION_REQUEST_SCHEMA_ID,
};

/// The Undo cases of the contract rows. Each case settles one Forward Author Action, or a
/// state that one Author Undo outcome needs.
#[derive(Clone, Copy, Debug)]
enum Case {
    CreateVolume,
    SetCurrentChapter,
    WithdrawProposal,
    ReplanProposal,
    ReopenRejectedOperations,
    ReopenWithdrawnProposal,
    CloseEditorFlowDraft,
    ExpandRefusedEditDraft,
    ApplyAuthorEdit,
    AcceptProposal,
    /// An Acceptance whose Chapter head moved back to the prior Revision.
    AcceptanceReversal,
    /// A stale Author Undo Frontier.
    StaleFrontier,
    /// A Draft Close whose Draft is no longer retained.
    ArchivedDraft,
    /// A Forward action with a Barrier disposition.
    Barrier,
    /// An Acceptance Undo that expects a head that is not current.
    AcceptanceWrongHead,
    /// An Acceptance whose current Revision Envelope does not agree with its payload.
    AcceptanceUnavailable,
}

/// The cases whose Author Undo applies, in the order of their result rows.
const APPLIED: [Case; 11] = [
    Case::CreateVolume,
    Case::SetCurrentChapter,
    Case::WithdrawProposal,
    Case::ReplanProposal,
    Case::ReopenRejectedOperations,
    Case::ReopenWithdrawnProposal,
    Case::CloseEditorFlowDraft,
    Case::ExpandRefusedEditDraft,
    Case::ApplyAuthorEdit,
    Case::AcceptProposal,
    Case::AcceptanceReversal,
];

/// The cases whose Author Undo has a zero-authority outcome.
const ZERO_AUTHORITY: [Case; 5] = [
    Case::StaleFrontier,
    Case::ArchivedDraft,
    Case::Barrier,
    Case::AcceptanceWrongHead,
    Case::AcceptanceUnavailable,
];

/// All cases, applied first.
fn all_cases() -> impl Iterator<Item = Case> {
    APPLIED.into_iter().chain(ZERO_AUTHORITY)
}

/// The latest Forward Author Action of the Project that no Compensation compensates.
async fn latest_forward(admin: &Client, scope: &ProjectScope) -> u64 {
    let sequence: String = admin
        .query_one(
            "SELECT max(forward.author_action_sequence)::text
               FROM storyos.author_action_entries AS forward
              WHERE forward.project_id = $1::text::uuid
                AND forward.disposition = 'forward'
                AND NOT EXISTS (
                      SELECT 1 FROM storyos.author_action_entries AS compensation
                       WHERE compensation.project_id = forward.project_id
                         AND compensation.compensated_source_sequence =
                             forward.author_action_sequence)",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    sequence.parse().unwrap()
}

/// The resulting Manuscript Revision of the Authoritative Commit of one Receipt.
async fn resulting_revision(admin: &Client, receipt_id: &str) -> String {
    admin
        .query_one(
            "SELECT resulting_revision_id::text FROM storyos.authoritative_commits
              WHERE receipt_id = $1::text::uuid",
            &[&receipt_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// The head of the Chapter of the base snapshot of one Editor Session.
async fn session_chapter_head(admin: &Client, editor_session_id: &str) -> String {
    admin
        .query_one(
            "SELECT head.current_revision_id::text
               FROM storyos.editor_session_base_snapshots AS snapshot
               JOIN storyos.authoritative_heads AS head
                 ON (head.owner_user_id, head.project_id, head.manuscript_object_id) =
                    (snapshot.owner_user_id, snapshot.project_id, snapshot.chapter_object_id)
              WHERE snapshot.editor_session_id = $1::text::uuid",
            &[&editor_session_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// Moves the base snapshot of one Editor Session to the Chapter and Revision of `revision_id`.
async fn edit_in_chapter_of(admin: &Client, editor_session_id: &str, revision_id: &str) {
    admin
        .execute(
            "UPDATE storyos.editor_session_base_snapshots AS snapshot
                SET chapter_object_id = revision.manuscript_object_id,
                    authoritative_revision_id = revision.revision_id
               FROM storyos.authoritative_revisions AS revision
              WHERE snapshot.editor_session_id = $1::text::uuid
                AND revision.revision_id = $2::text::uuid",
            &[&editor_session_id, &revision_id],
        )
        .await
        .unwrap();
}

/// Settles the Forward Author Action of `case` in a new Project and returns the Author Undo of
/// it.
async fn undo_call(
    store: &PostgresProjectReader,
    admin: &Client,
    case: Case,
    base: u16,
) -> CommandCall<UndoLatestAuthorAction> {
    let (scope, editor_session_id, expected_authoritative_revision_id) = match case {
        Case::CreateVolume => {
            let (scope, _chapter_a, _chapter_b, revision_b, editor_session_id) =
                two_chapter_writer(store, base).await;
            let expected_tree_revision: i64 = admin
                .query_one(
                    "SELECT tree_revision FROM storyos.projects WHERE project_id = $1::text::uuid",
                    &[&scope.project_id.as_ref()],
                )
                .await
                .unwrap()
                .get(/*idx*/ 0);
            let input = CreateVolumeInput {
                title: "Volume".to_owned(),
                expected_tree_revision: expected_tree_revision.try_into().unwrap(),
            };
            let call = issued(store, &scope, base + 9, &CREATE_VOLUME, input).await;
            create_volume(store, &call).await.unwrap();
            (scope, editor_session_id, revision_b)
        }
        Case::SetCurrentChapter | Case::StaleFrontier => {
            let call = set_current_chapter_call(store, base).await;
            store
                .set_current_chapter(&call.envelope, &call.input)
                .await
                .unwrap();
            (
                call.envelope.project_scope,
                call.input.editor_session_id.as_ref().to_owned(),
                call.input.expected_target_revision_id,
            )
        }
        Case::WithdrawProposal => {
            let call = withdraw_proposal_call(store, admin, base).await;
            withdraw_proposal(store, &call).await.unwrap();
            (
                call.envelope.project_scope,
                call.input.editor_session_id.as_ref().to_owned(),
                call.input.expected_authoritative_revision_id,
            )
        }
        Case::ReplanProposal => {
            let call = replan_proposal_call(store, admin, base).await;
            replan_proposal(store, &call).await.unwrap();
            (
                call.envelope.project_scope,
                call.input.editor_session_id.as_ref().to_owned(),
                call.input.expected_authoritative_revision_id,
            )
        }
        Case::ReopenRejectedOperations => {
            let call = reopen_rejected_operations_call(store, admin, base).await;
            reopen_rejected_operations(store, &call).await.unwrap();
            (
                call.envelope.project_scope,
                call.input.editor_session_id.as_ref().to_owned(),
                call.input.expected_authoritative_revision_id,
            )
        }
        Case::ReopenWithdrawnProposal => {
            let call = reopen_withdrawn_proposal_call(store, admin, base).await;
            reopen_withdrawn_proposal(store, &call).await.unwrap();
            (
                call.envelope.project_scope,
                call.input.editor_session_id.as_ref().to_owned(),
                call.input.expected_authoritative_revision_id,
            )
        }
        Case::Barrier => {
            let call = reject_proposal_operations_call(store, admin, base).await;
            reject_proposal_operations(store, &call).await.unwrap();
            let head = session_chapter_head(admin, call.input.editor_session_id.as_ref()).await;
            (
                call.envelope.project_scope,
                call.input.editor_session_id.as_ref().to_owned(),
                head,
            )
        }
        Case::CloseEditorFlowDraft | Case::ArchivedDraft => {
            let call = close_editor_flow_draft_call(store, admin, base).await;
            close_editor_flow_draft(store, &call).await.unwrap();
            if matches!(case, Case::ArchivedDraft) {
                admin
                    .execute(
                        "UPDATE storyos.draft_artifacts SET retention_state = 'archived'
                          WHERE draft_id = $1::text::uuid",
                        &[&call.input.draft_id],
                    )
                    .await
                    .unwrap();
            }
            let head = session_chapter_head(admin, call.input.editor_session_id.as_ref()).await;
            (
                call.envelope.project_scope,
                call.input.editor_session_id.as_ref().to_owned(),
                head,
            )
        }
        Case::ExpandRefusedEditDraft => {
            let call = expand_refused_edit_draft_call(store, admin, base).await;
            expand_refused_edit_draft(store, &call).await.unwrap();
            // The writer Editor Session edits Chapter B, the target of the Draft.
            edit_in_chapter_of(
                admin,
                call.input.editor_session_id.as_ref(),
                &call.input.expected_target_revision_id,
            )
            .await;
            (
                call.envelope.project_scope,
                call.input.editor_session_id.as_ref().to_owned(),
                call.input.expected_target_revision_id,
            )
        }
        Case::ApplyAuthorEdit => {
            let (scope, chapter_a, _chapter_b, _revision_b, editor_session_id) =
                two_chapter_writer(store, base).await;
            let head = session_chapter_head(admin, &editor_session_id).await;
            let suffix = format!("{:04x}", base + 9);
            apply_named_edit(
                store,
                &scope,
                NamedEdit {
                    editor_session_id: &editor_session_id,
                    chapter_id: &chapter_a,
                    expected_revision_id: &head,
                    suffix: &suffix,
                    local_intent_sequence: 1,
                    text: "x",
                },
            )
            .await;
            let resulting =
                resulting_revision(admin, &format!("018f0000-0000-7001-8000-00000003{suffix}"))
                    .await;
            (scope, editor_session_id, resulting)
        }
        Case::AcceptProposal
        | Case::AcceptanceReversal
        | Case::AcceptanceWrongHead
        | Case::AcceptanceUnavailable => {
            let (scope, input, _chapter_a_head) = acceptable_proposal(store, admin, base).await;
            // A Reversal Proposal names the AgentRun of the accepted Proposal, so the run must
            // exist.
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
            let prior = input.expected_authoritative_revision_id.clone();
            let call = issued(store, &scope, base + 9, &ACCEPT_PROPOSAL, input).await;
            accept_proposal(store, &call).await.unwrap();
            let resulting = resulting_revision(admin, &call.envelope.ids.receipt_id).await;
            // The writer Editor Session edits the accepted Chapter B, as an author who accepts
            // in the Chapter does.
            edit_in_chapter_of(admin, call.input.editor_session_id.as_ref(), &resulting).await;
            let expected = match case {
                Case::AcceptanceReversal => {
                    run_without_foreign_keys(
                        admin,
                        &format!(
                            "UPDATE storyos.authoritative_heads
                                SET current_revision_id = '{prior}'
                              WHERE current_revision_id = '{resulting}'"
                        ),
                    )
                    .await;
                    prior
                }
                Case::AcceptanceWrongHead => prior,
                Case::AcceptanceUnavailable => {
                    run_without_foreign_keys(
                        admin,
                        &format!(
                            "UPDATE storyos.authoritative_revision_envelopes
                                SET payload_digest = 'sha256:damaged'
                              WHERE revision_id = '{resulting}'"
                        ),
                    )
                    .await;
                    resulting
                }
                _ => resulting,
            };
            (
                scope,
                call.input.editor_session_id.as_ref().to_owned(),
                expected,
            )
        }
    };
    let frontier = latest_forward(admin, &scope).await;
    let input = UndoLatestAuthorActionInput {
        editor_session_id: EditorSessionId::new(editor_session_id),
        expected_author_undo_frontier_sequence: match case {
            Case::StaleFrontier => frontier + 1,
            _ => frontier,
        },
        expected_authoritative_revision_id,
    };
    let call = issued_undo(store, &scope, base + 10, input).await;
    CommandCall {
        envelope: call.envelope,
        input: UndoLatestAuthorAction { input: call.input },
    }
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

/// Issues one Author Undo whose canonical bytes and digest are those of its command body. The
/// Draft reopen records check both.
async fn issued_undo(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: u16,
    input: UndoLatestAuthorActionInput,
) -> CommandCall<UndoLatestAuthorActionInput> {
    let bytes = undo_body(&input);
    let digest = format!(
        "sha256:{}:{}",
        storyos_contracts::UNDO_LATEST_AUTHOR_ACTION_DIGEST_PROFILE,
        storyos_core::hex_sha256(&bytes)
    );
    issued_with_bytes(
        store,
        scope,
        suffix,
        &UNDO_LATEST_AUTHOR_ACTION,
        input,
        &bytes,
        digest,
    )
    .await
}

/// The canonical command body of one Author Undo.
fn undo_body(input: &UndoLatestAuthorActionInput) -> Vec<u8> {
    serde_json::json!({
        "command_schema": storyos_contracts::UNDO_LATEST_AUTHOR_ACTION_REQUEST_SCHEMA_ID,
        "undo_latest_author_action_input": {
            "expected_author_undo_frontier_sequence":
                input.expected_author_undo_frontier_sequence.to_string(),
            "expected_authoritative_revision_id": input.expected_authoritative_revision_id,
            "editor_session_id": input.editor_session_id.as_ref(),
        },
    })
    .to_string()
    .into_bytes()
}

async fn undo(
    store: &PostgresProjectReader,
    call: &CommandCall<UndoLatestAuthorActionInput>,
) -> Result<UndoLatestAuthorActionSettlement, ProjectCommandError> {
    store
        .undo_latest_author_action(&call.envelope, &call.input)
        .await
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
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    let mut acceptance_children = Vec::new();
    for (case, base) in all_cases().zip((0x3a00..).step_by(/*step*/ 0x10)) {
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
    let draft = ("draft_closure_changed", [1, 1, 0, 0, 0]);
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
            draft,
            draft,
            revision,
            revision,
            reversal,
            conflicted,
            refused,
            refused,
            conflicted,
            refused,
        ]
    );
    // The `undo_acceptance_receipts` row of each outcome. An Acceptance conflict writes none, as
    // on `main`.
    assert_eq!(
        acceptance_children,
        vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1]
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_failing_undo_step_rolls_back_every_row_and_keeps_the_challenge_unused() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    for (case, base) in all_cases().zip((0x3b00..).step_by(/*step*/ 0x10)) {
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
        refused.clone(),
        conflicted(ReceiptResult::Conflicted),
        refused,
    ]);
    assert_eq!(observed, expected);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_in_progress_undo_retry_conflicts_and_writes_no_row() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    for (case, base) in all_cases().zip((0x3c00..).step_by(/*step*/ 0x10)) {
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
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    for (case, base) in all_cases().zip((0x3d00..).step_by(/*step*/ 0x10)) {
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
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
        (Case::CloseEditorFlowDraft, 0x3e30, |receipt_id: &str| {
            format!(
                "DELETE FROM storyos.draft_reopen_events AS event
                  USING storyos.domain_receipts AS receipt
                  WHERE receipt.receipt_id = '{receipt_id}'
                    AND event.event_id::text = receipt.result_payload->>'event_id'"
            )
        }),
    ] {
        let call = undo_call(&store, &admin, case, base).await;
        observed.push(damaged_replay(&store, &admin, &call, damage).await);
    }
    assert_eq!(observed, vec![ReplayError::Unavailable; 4]);
}
