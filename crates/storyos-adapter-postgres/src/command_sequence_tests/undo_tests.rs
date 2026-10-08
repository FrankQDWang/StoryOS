use storyos_application::{
    CreateVolumeInput, EditorSessionId, ProjectCommandError, ProjectScope,
    UndoLatestAuthorActionInput, UndoLatestAuthorActionSettlement,
};
use storyos_core::ReceiptResult;
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::undo_latest_author_action::UndoLatestAuthorAction;

use super::acceptance::{ACCEPT_PROPOSAL, accept_proposal, acceptable_proposal};
use super::damaged_evidence::{ReplayError, damaged_replay, evidence_replays};
use super::draft::{close_editor_flow_draft, close_editor_flow_draft_call};
use super::proposal_decision::{withdraw_proposal, withdraw_proposal_call};
use super::retry::in_progress_retry;
use super::rollback::failed_then_settled;
use super::structure::{CREATE_VOLUME, create_volume, set_current_chapter_call};
use super::support::{
    CommandCall, Route, issued, issued_with_bytes, replayed_outcome, stores, two_chapter_writer,
};

pub(super) const UNDO_LATEST_AUTHOR_ACTION: Route = Route {
    kind: "undoLatestAuthorAction",
    method: storyos_contracts::UNDO_LATEST_AUTHOR_ACTION_METHOD,
    path: storyos_contracts::UNDO_LATEST_AUTHOR_ACTION_PATH,
    schema: storyos_contracts::UNDO_LATEST_AUTHOR_ACTION_REQUEST_SCHEMA_ID,
};

/// The Forward command kinds whose Author Undo the contract rows settle.
#[derive(Clone, Copy, Debug)]
enum Forward {
    CreateVolume,
    SetCurrentChapter,
    WithdrawProposal,
    CloseEditorFlowDraft,
    AcceptProposal,
}

const FORWARDS: [Forward; 5] = [
    Forward::CreateVolume,
    Forward::SetCurrentChapter,
    Forward::WithdrawProposal,
    Forward::CloseEditorFlowDraft,
    Forward::AcceptProposal,
];

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

/// Settles one Forward Author Action in a new Project and returns the Author Undo of it.
async fn undo_call(
    store: &PostgresProjectReader,
    admin: &Client,
    forward: Forward,
    base: u16,
) -> CommandCall<UndoLatestAuthorAction> {
    let (scope, editor_session_id, expected_authoritative_revision_id) = match forward {
        Forward::CreateVolume => {
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
        Forward::SetCurrentChapter => {
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
        Forward::WithdrawProposal => {
            let call = withdraw_proposal_call(store, admin, base).await;
            withdraw_proposal(store, &call).await.unwrap();
            (
                call.envelope.project_scope,
                call.input.editor_session_id.as_ref().to_owned(),
                call.input.expected_authoritative_revision_id,
            )
        }
        Forward::CloseEditorFlowDraft => {
            let call = close_editor_flow_draft_call(store, admin, base).await;
            close_editor_flow_draft(store, &call).await.unwrap();
            let head = session_chapter_head(admin, call.input.editor_session_id.as_ref()).await;
            (
                call.envelope.project_scope,
                call.input.editor_session_id.as_ref().to_owned(),
                head,
            )
        }
        Forward::AcceptProposal => {
            let (scope, input, _chapter_a_head) = acceptable_proposal(store, admin, base).await;
            let call = issued(store, &scope, base + 9, &ACCEPT_PROPOSAL, input).await;
            accept_proposal(store, &call).await.unwrap();
            let resulting = resulting_revision(admin, &call.envelope.ids.receipt_id).await;
            // The writer Editor Session edits the accepted Chapter B, as an author who accepts
            // in the Chapter does.
            admin
                .execute(
                    "UPDATE storyos.editor_session_base_snapshots AS snapshot
                        SET chapter_object_id = revision.manuscript_object_id,
                            authoritative_revision_id = revision.revision_id
                       FROM storyos.authoritative_revisions AS revision
                      WHERE snapshot.editor_session_id = $1::text::uuid
                        AND revision.revision_id = $2::text::uuid",
                    &[&call.input.editor_session_id.as_ref(), &resulting],
                )
                .await
                .unwrap();
            (
                scope,
                call.input.editor_session_id.as_ref().to_owned(),
                resulting,
            )
        }
    };
    let input = UndoLatestAuthorActionInput {
        editor_session_id: EditorSessionId::new(editor_session_id),
        expected_author_undo_frontier_sequence: latest_forward(admin, &scope).await,
        expected_authoritative_revision_id,
    };
    let call = issued_undo(store, &scope, base + 10, input).await;
    CommandCall {
        envelope: call.envelope,
        input: UndoLatestAuthorAction { input: call.input },
    }
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
    for (forward, base) in FORWARDS.into_iter().zip((0x3a00..).step_by(/*step*/ 0x10)) {
        let call = plain(&undo_call(&store, &admin, forward, base).await);
        observed.push(replayed_outcome(&store, &admin, &call, undo).await);
    }
    // A stale Author Undo Frontier conflicts, and a Draft Close whose Draft is no longer
    // retained is refused.
    let mut stale = plain(
        &undo_call(
            &store,
            &admin,
            Forward::SetCurrentChapter,
            /*base*/ 0x3a60,
        )
        .await,
    );
    stale.input.expected_author_undo_frontier_sequence += 1;
    let stale = issued_undo(
        &store,
        &stale.envelope.project_scope,
        /*suffix*/ 0x3a6b,
        stale.input,
    )
    .await;
    observed.push(replayed_outcome(&store, &admin, &stale, undo).await);
    let archived = plain(
        &undo_call(
            &store,
            &admin,
            Forward::CloseEditorFlowDraft,
            /*base*/ 0x3a70,
        )
        .await,
    );
    admin
        .execute(
            "UPDATE storyos.draft_artifacts SET retention_state = 'archived'
              WHERE project_id = $1::text::uuid",
            &[&archived.envelope.project_scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    observed.push(replayed_outcome(&store, &admin, &archived, undo).await);

    // Receipt, Author Action, Activity, Commit, and Snapshot rows of each outcome. A Structure
    // or Current Chapter Compensation writes a canonical Snapshot and no Activity event. The
    // Snapshot count joins the Activity rows, so it is zero.
    let structure = ("authoritative_applied", [1, 1, 0, 1, 0]);
    let current_chapter = ("authoritative_applied", [1, 1, 0, 0, 0]);
    let proposal = ("authoritative_applied", [1, 1, 0, 0, 0]);
    let draft = ("draft_closure_changed", [1, 1, 0, 0, 0]);
    let revision = ("authoritative_applied", [1, 1, 1, 1, 1]);
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
            draft,
            revision,
            conflicted,
            refused,
        ]
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
    for (forward, base) in FORWARDS.into_iter().zip((0x3b00..).step_by(/*step*/ 0x10)) {
        let call = undo_call(&store, &admin, forward, base).await;
        observed.push(failed_then_settled(&store, &admin, &call).await);
    }
    let mut stale = plain(
        &undo_call(
            &store,
            &admin,
            Forward::SetCurrentChapter,
            /*base*/ 0x3b50,
        )
        .await,
    );
    stale.input.expected_author_undo_frontier_sequence += 1;
    let stale = issued_undo(
        &store,
        &stale.envelope.project_scope,
        /*suffix*/ 0x3b5b,
        stale.input,
    )
    .await;
    let stale = CommandCall {
        envelope: stale.envelope,
        input: UndoLatestAuthorAction { input: stale.input },
    };
    observed.push(failed_then_settled(&store, &admin, &stale).await);
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
    let mut expected = vec![applied; 5];
    // A zero-authority Author Undo writes only its Receipt, so only its classify step can fail.
    // The first settlement after that failure is real, and the next one replays it.
    expected.push((
        vec![
            (Some("classify"), [0; 5]),
            (None, [1, 0, 0, 0, 0]),
            (None, [1, 0, 0, 0, 0]),
        ],
        ReceiptResult::Conflicted,
    ));
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
    for (forward, base) in FORWARDS.into_iter().zip((0x3c00..).step_by(/*step*/ 0x10)) {
        let call = undo_call(&store, &admin, forward, base).await;
        observed.push(in_progress_retry(&store, &admin, &call).await);
    }
    assert_eq!(observed, vec![(true, [0; 5]); 5]);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_undo_replay_separates_pre_capture_from_damaged_evidence() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    for (forward, base) in FORWARDS.into_iter().zip((0x3d00..).step_by(/*step*/ 0x10)) {
        let call = undo_call(&store, &admin, forward, base).await;
        observed.push(evidence_replays(&store, &admin, &call).await);
    }
    let pre_capture_then_damaged = [
        ReplayError::HistoricalAcknowledgementUnavailable,
        ReplayError::Unavailable,
    ];
    assert_eq!(
        observed,
        vec![
            (
                ReceiptResult::AuthoritativeApplied,
                pre_capture_then_damaged
            );
            5
        ]
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_damaged_undo_replay_is_a_store_fault() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    for (forward, base, damage) in [
        (
            Forward::CreateVolume,
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
        (Forward::AcceptProposal, 0x3e10, |receipt_id: &str| {
            format!("DELETE FROM storyos.authoritative_commits WHERE receipt_id = '{receipt_id}'")
        }),
        (Forward::SetCurrentChapter, 0x3e20, |receipt_id: &str| {
            format!(
                "UPDATE storyos.author_action_entries SET disposition = 'forward',
                        compensated_source_sequence = NULL
                  WHERE receipt_id = '{receipt_id}'"
            )
        }),
        (Forward::CloseEditorFlowDraft, 0x3e30, |receipt_id: &str| {
            format!(
                "DELETE FROM storyos.draft_reopen_events AS event
                  USING storyos.domain_receipts AS receipt
                  WHERE receipt.receipt_id = '{receipt_id}'
                    AND event.event_id::text = receipt.result_payload->>'event_id'"
            )
        }),
    ] {
        let call = undo_call(&store, &admin, forward, base).await;
        observed.push(damaged_replay(&store, &admin, &call, damage).await);
    }
    assert_eq!(observed, vec![ReplayError::Unavailable; 4]);
}
