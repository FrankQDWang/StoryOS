//! Author Undo of a Refused Edit Draft expansion after the Undo of later actions (ADR 0068).
//! Each Undo takes the current Author Undo Frontier and the current Chapter Head, as the Web
//! Client does.

use storyos_application::{
    AuthorEditProposalTarget, EditorSessionId, ProjectScope, UndoLatestAuthorActionSettlement,
    UndoRecords, WithdrawProposalInput, WithdrawalNote,
};
use storyos_core::{
    AuthorEditPrimitive, TransitionOutcome, UndoLatestAuthorActionConflict,
    UndoLatestAuthorActionUnavailable,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::create_volume_authority_tests::{NamedEdit, adjusted_edit_command};

use super::draft::{expand_refused_edit_draft, expansion_call, refused_edit_expansion};
use super::proposal_decision::{WITHDRAW_PROPOSAL, withdraw_proposal};
use super::proposal_undo_chain::{compensated, head_closure, undo_call};
use super::secondary_operation::proposal_head_revision;
use super::support::{issued, run_without_foreign_keys, stores, with_new_request_ids};
use super::undo::{edit_in_chapter_of, latest_forward, session_chapter_head, undo};

/// The expanded Proposal of one Refused Edit Draft and the Forward action X of the expansion.
struct Expanded {
    scope: ProjectScope,
    editor_session_id: String,
    chapter_id: String,
    draft_id: String,
    proposal_id: String,
    operation_id: String,
    block_id: String,
    expansion: u64,
    /// The Author Undo Frontier before X.
    prior: Option<u64>,
}

/// Expands one retained Refused Edit Draft to a Proposal in a new Project.
async fn expanded(store: &PostgresProjectReader, admin: &Client, base: u16) -> Expanded {
    let (scope, input) = refused_edit_expansion(store, admin, base, "retained").await;
    let prior: Option<String> = admin
        .query_one(
            "SELECT max(author_action_sequence)::text FROM storyos.author_action_entries
              WHERE project_id = $1::text::uuid AND disposition = 'forward'",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let call = expansion_call(store, &scope, base + 9, input.clone()).await;
    let TransitionOutcome::Applied(applied) = expand_refused_edit_draft(store, &call)
        .await
        .unwrap()
        .outcome
    else {
        panic!("the expansion must apply");
    };
    // The writer Editor Session edits Chapter B, the target of the Draft.
    edit_in_chapter_of(
        admin,
        input.editor_session_id.as_ref(),
        &input.expected_target_revision_id,
    )
    .await;
    let operation_id = admin
        .query_one(
            "SELECT operation_id::text FROM storyos.proposal_operations
              WHERE proposal_id = $1::text::uuid",
            &[&applied.effect.proposal_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    Expanded {
        scope,
        editor_session_id: input.editor_session_id.as_ref().to_owned(),
        chapter_id: input.chapter_id,
        draft_id: input.draft_id,
        proposal_id: applied.effect.proposal_id,
        operation_id,
        block_id: input.target_ref,
        expansion: applied.author_action_sequence,
        prior: prior.map(|sequence| sequence.parse().unwrap()),
    }
}

/// The text that one Author Edit changes.
enum Edited {
    /// The Chapter text before the Anchor of the expanded Proposal.
    Prose,
    /// The candidate of the expanded Proposal.
    Candidate,
}

/// One Author Edit that inserts `Edited ` in the Chapter or in the candidate. It
/// names the Proposal head, as the Web Client does. Returns the Forward action.
async fn edit(
    store: &PostgresProjectReader,
    admin: &Client,
    expanded: &Expanded,
    suffix: u16,
    edited: Edited,
) -> u64 {
    let head = proposal_head_revision(admin, &expanded.proposal_id).await;
    let proposal_target = match &edited {
        Edited::Prose => None,
        Edited::Candidate => Some(AuthorEditProposalTarget {
            proposal_id: expanded.proposal_id.clone(),
            operation_id: expanded.operation_id.clone(),
            revision_id: head.clone(),
            manuscript_block_id: expanded.block_id.clone(),
        }),
    };
    let command = adjusted_edit_command(
        store,
        &expanded.scope,
        NamedEdit {
            editor_session_id: &expanded.editor_session_id,
            chapter_id: &expanded.chapter_id,
            expected_revision_id: &session_chapter_head(admin, &expanded.editor_session_id).await,
            suffix: &format!("{suffix:04x}"),
            local_intent_sequence: u64::from(suffix),
            text: "Edited ",
            proposal_target,
        },
        |command| {
            command.expected_proposal_head_revision_ids = vec![head];
            command.observed_ownership_partition = "mixed".to_owned();
            if let Edited::Candidate = edited {
                // The insertion must be inside the candidate, not on its edge.
                let unit = &mut command.author_edit_units[0];
                unit.normalized_primitives = vec![AuthorEditPrimitive::ReplaceSelection {
                    from: 1,
                    to: 1,
                    text: "Edited ".to_owned(),
                }];
                unit.selection_snapshot.from = 1;
                unit.selection_snapshot.to = 1;
            }
        },
    )
    .await;
    let before = latest_forward(admin, &expanded.scope).await;
    let settled = store.apply_author_edit(&command).await.unwrap();
    let forward = latest_forward(admin, &expanded.scope).await;
    assert_ne!(
        forward, before,
        "the edit must apply, got {:?}",
        settled.effect
    );
    forward
}

/// An expansion writes a structured candidate, and a candidate edit refuses a structured
/// candidate. The test makes the candidate plain text, so that a candidate edit applies.
async fn plain_candidate(admin: &Client, expanded: &Expanded) {
    run_without_foreign_keys(
        admin,
        &format!(
            "UPDATE storyos.proposal_operations SET candidate_blocks = NULL
              WHERE proposal_id = '{proposal}';
             UPDATE storyos.proposal_revisions SET candidate_blocks = NULL
              WHERE proposal_id = '{proposal}'",
            proposal = expanded.proposal_id
        ),
    )
    .await;
}

/// One Author Undo of the current Author Undo Frontier with the current Chapter Head.
async fn undo_latest(
    store: &PostgresProjectReader,
    admin: &Client,
    expanded: &Expanded,
    suffix: u16,
) -> UndoLatestAuthorActionSettlement {
    let head = session_chapter_head(admin, &expanded.editor_session_id).await;
    let call = undo_call(
        store,
        admin,
        &expanded.scope,
        &expanded.editor_session_id,
        &head,
        suffix,
    )
    .await;
    undo(store, &call).await.unwrap()
}

/// The closure of the Draft, the closure of the Proposal head, and the reservation state of the
/// Operation.
async fn closures(admin: &Client, expanded: &Expanded) -> (String, String, String) {
    let row = admin
        .query_one(
            "SELECT draft.closure, operation.reservation_state
               FROM storyos.draft_artifacts AS draft, storyos.proposal_operations AS operation
              WHERE draft.draft_id = $1::text::uuid AND operation.operation_id = $2::text::uuid",
            &[&expanded.draft_id, &expanded.operation_id],
        )
        .await
        .unwrap();
    (
        row.get(/*idx*/ 0),
        head_closure(admin, &expanded.proposal_id).await,
        row.get(/*idx*/ 1),
    )
}

/// The closures after an Undo that reopens the Draft and withdraws the Proposal head.
fn reopened() -> (String, String, String) {
    (
        "open".to_owned(),
        "withdrawn".to_owned(),
        "resolved".to_owned(),
    )
}

/// Undo X after X, E1, Undo E1 reopens the Draft and withdraws the head that Undo E1 wrote. An
/// exact retry replays the first settlement.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_reverses_an_expansion_after_the_undo_of_a_candidate_edit() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let expanded = expanded(&store, &admin, /*base*/ 0x4d40).await;
    plain_candidate(&admin, &expanded).await;
    let first = edit(
        &store,
        &admin,
        &expanded,
        /*suffix*/ 0x4d4a,
        Edited::Candidate,
    )
    .await;
    let undo_edit = compensated(&undo_latest(&store, &admin, &expanded, /*suffix*/ 0x4d4b).await);
    let head = proposal_head_revision(&admin, &expanded.proposal_id).await;
    let call = undo_call(
        &store,
        &admin,
        &expanded.scope,
        &expanded.editor_session_id,
        &session_chapter_head(&admin, &expanded.editor_session_id).await,
        /*suffix*/ 0x4d4c,
    )
    .await;
    let undo_expansion = undo(&store, &call).await.unwrap();
    let retried = undo(&store, &with_new_request_ids(&call)).await.unwrap();

    assert!(
        matches!(
            &undo_expansion.outcome,
            TransitionOutcome::Applied(applied) if matches!(applied.records, UndoRecords::Draft { .. })
        ),
        "the Undo must reopen the Draft, got {:?}",
        undo_expansion.outcome
    );
    assert_eq!(
        (
            undo_edit,
            compensated(&undo_expansion),
            &retried,
            proposal_head_revision(&admin, &expanded.proposal_id).await,
            closures(&admin, &expanded).await
        ),
        (
            (first, Some(expanded.expansion)),
            (expanded.expansion, expanded.prior),
            &undo_expansion,
            head,
            reopened()
        )
    );
}

/// Undo X after X, an author withdrawal W, Undo W reopens the Draft.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_reverses_an_expansion_after_the_undo_of_a_withdrawal() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let expanded = expanded(&store, &admin, /*base*/ 0x4d60).await;
    let input = WithdrawProposalInput {
        editor_session_id: EditorSessionId::new(expanded.editor_session_id.clone()),
        proposal_id: expanded.proposal_id.clone(),
        proposal_revision_id: proposal_head_revision(&admin, &expanded.proposal_id).await,
        expected_authoritative_revision_id: session_chapter_head(
            &admin,
            &expanded.editor_session_id,
        )
        .await,
        withdrawal_note: WithdrawalNote::Omitted,
    };
    let call = issued(
        &store,
        &expanded.scope,
        /*suffix*/ 0x4d6a,
        &WITHDRAW_PROPOSAL,
        input,
    )
    .await;
    withdraw_proposal(&store, &call).await.unwrap();
    let withdrawn = latest_forward(&admin, &expanded.scope).await;

    let steps = [
        compensated(&undo_latest(&store, &admin, &expanded, /*suffix*/ 0x4d6b).await),
        compensated(&undo_latest(&store, &admin, &expanded, /*suffix*/ 0x4d6c).await),
    ];

    assert_eq!(
        (steps, closures(&admin, &expanded).await),
        (
            [
                (withdrawn, Some(expanded.expansion)),
                (expanded.expansion, expanded.prior)
            ],
            reopened()
        )
    );
}

/// Undo X after X, a prose edit P1, Undo P1 reopens the Draft. The Chapter Head is the
/// Compensation of P1, and it restores the base of the expanded Proposal (ADR 0067).
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_reverses_an_expansion_after_the_undo_of_a_prose_edit() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let expanded = expanded(&store, &admin, /*base*/ 0x4d80).await;
    let prose = edit(
        &store,
        &admin,
        &expanded,
        /*suffix*/ 0x4d8a,
        Edited::Prose,
    )
    .await;

    let steps = [
        compensated(&undo_latest(&store, &admin, &expanded, /*suffix*/ 0x4d8b).await),
        compensated(&undo_latest(&store, &admin, &expanded, /*suffix*/ 0x4d8c).await),
    ];

    assert_eq!(
        (steps, closures(&admin, &expanded).await),
        (
            [
                (prose, Some(expanded.expansion)),
                (expanded.expansion, expanded.prior)
            ],
            reopened()
        )
    );
}

/// A Proposal head that no recorded Compensation explains stops the Undo of X with `barrier`.
/// A changed source Draft stays a conflict, also when the Proposal head is explained.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_unexplained_proposal_head_is_a_barrier_and_a_changed_draft_is_a_conflict() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let unexplained = expanded(&store, &admin, /*base*/ 0x4da0).await;
    let head = proposal_head_revision(&admin, &unexplained.proposal_id).await;
    let moved = uuid::Uuid::now_v7().to_string();
    admin
        .batch_execute(&format!(
            "INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id, parent_revision_id,
                candidate_blocks)
             SELECT owner_user_id, project_id, proposal_id, '{moved}', generation, validation,
                    closure, candidate_text, base_authoritative_revision_id, revision_id,
                    candidate_blocks
               FROM storyos.proposal_revisions WHERE revision_id = '{head}';
             UPDATE storyos.proposal_heads SET current_revision_id = '{moved}'
              WHERE current_revision_id = '{head}'"
        ))
        .await
        .unwrap();
    let barrier = undo_latest(&store, &admin, &unexplained, /*suffix*/ 0x4daa).await;

    let changed = expanded(&store, &admin, /*base*/ 0x4dc0).await;
    plain_candidate(&admin, &changed).await;
    edit(
        &store,
        &admin,
        &changed,
        /*suffix*/ 0x4dca,
        Edited::Candidate,
    )
    .await;
    compensated(&undo_latest(&store, &admin, &changed, /*suffix*/ 0x4dcb).await);
    run_without_foreign_keys(
        &admin,
        &format!(
            "UPDATE storyos.draft_artifact_revisions SET payload_digest = '{}'
              WHERE draft_id = '{}'",
            "0".repeat(64),
            changed.draft_id
        ),
    )
    .await;
    let conflict = undo_latest(&store, &admin, &changed, /*suffix*/ 0x4dcc).await;

    assert_eq!(
        (
            barrier.outcome,
            closures(&admin, &unexplained).await.0,
            conflict.outcome,
            closures(&admin, &changed).await.0
        ),
        (
            TransitionOutcome::Refused(UndoLatestAuthorActionUnavailable::Barrier),
            "closed".to_owned(),
            TransitionOutcome::Conflicted(UndoLatestAuthorActionConflict::SourceBindingChanged),
            "closed".to_owned()
        )
    );
}

/// Runs the reopen trigger function of migration 0097 on one recorded Draft reopen event.
async fn check_reopen_event(admin: &Client, draft_id: &str) -> Result<(), String> {
    let checked = admin
        .batch_execute(&format!(
            "BEGIN;
             CREATE TEMPORARY TABLE reopen_probe
               (owner_user_id uuid, project_id uuid, event_id uuid) ON COMMIT DROP;
             CREATE TRIGGER reopen_probe_check AFTER INSERT ON reopen_probe
               FOR EACH ROW EXECUTE FUNCTION storyos.require_draft_reopen_settlement();
             INSERT INTO reopen_probe
               SELECT owner_user_id, project_id, event_id FROM storyos.draft_reopen_events
                WHERE draft_id = '{draft_id}';
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

/// The reopen trigger accepts a withdrawn Proposal head that a recorded Proposal Compensation
/// or a recorded Acceptance Compensation wrote. It refuses a head that no record explains.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_reopen_trigger_accepts_a_head_that_a_recorded_compensation_wrote() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let expanded = expanded(&store, &admin, /*base*/ 0x4de0).await;
    plain_candidate(&admin, &expanded).await;
    edit(
        &store,
        &admin,
        &expanded,
        /*suffix*/ 0x4dea,
        Edited::Candidate,
    )
    .await;
    let undo_edit = undo_call(
        &store,
        &admin,
        &expanded.scope,
        &expanded.editor_session_id,
        &session_chapter_head(&admin, &expanded.editor_session_id).await,
        /*suffix*/ 0x4deb,
    )
    .await;
    compensated(&undo(&store, &undo_edit).await.unwrap());
    compensated(&undo_latest(&store, &admin, &expanded, /*suffix*/ 0x4dec).await);
    let recorded = check_reopen_event(&admin, &expanded.draft_id).await;
    let head = proposal_head_revision(&admin, &expanded.proposal_id).await;
    run_without_foreign_keys(
        &admin,
        &format!(
            "UPDATE storyos.domain_receipts
                SET result_payload = result_payload - 'proposal_revision_id'
              WHERE receipt_id = '{}'",
            undo_edit.envelope.ids.receipt_id
        ),
    )
    .await;
    let unexplained = check_reopen_event(&admin, &expanded.draft_id).await;
    // The shape of the reopen Revision of an Acceptance Compensation (migration 0074).
    run_without_foreign_keys(
        &admin,
        &format!(
            "INSERT INTO storyos.undo_acceptance_receipts
               (owner_user_id, project_id, undo_acceptance_receipt_id, author_undo_receipt_id,
                acceptance_receipt_id, source_author_action_sequence, outcome,
                authoritative_commit_id, proposal_id, proposal_revision_id,
                reported_activity_position)
             VALUES ('{owner}', '{project}', '{id}', '{id}', '{id}', 1, 'compensated', '{id}',
                     '{proposal}', '{head}', 1)",
            owner = expanded.scope.owner_user_id.as_ref(),
            project = expanded.scope.project_id.as_ref(),
            id = uuid::Uuid::now_v7(),
            proposal = expanded.proposal_id,
        ),
    )
    .await;

    assert_eq!(
        (
            recorded,
            unexplained,
            check_reopen_event(&admin, &expanded.draft_id).await
        ),
        (
            Ok(()),
            Err(r#"Some(("23514", "Incomplete Draft reopen settlement"))"#.to_owned()),
            Ok(())
        )
    );
}
