//! Author Undo of a chain of Forward actions on one Chapter (ADR 0067). Each Undo takes the
//! Author Undo Frontier and the current Chapter Head, as the Web Client does.

use storyos_application::{
    EditorSessionId, ProjectScope, UndoLatestAuthorActionInput, UndoLatestAuthorActionSettlement,
    UndoRecords,
};
use storyos_core::{TransitionOutcome, UndoLatestAuthorActionUnavailable};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::create_volume_authority_tests::{NamedEdit, apply_named_edit};

use super::acceptance::{ACCEPT_PROPOSAL, accept_proposal, acceptable_proposal};
use super::agent_run::seed_run;
use super::proposal_decision::{
    REJECT_PROPOSAL_OPERATIONS, pending_proposal, reject_proposal_operations,
};
use super::support::{issued, stores, two_chapter_writer};
use super::undo::{
    edit_in_chapter_of, issued_undo, latest_forward, resulting_revision, session_chapter_head, undo,
};

/// One Author Edit that inserts `text` at the start of the Chapter of the Editor Session.
async fn edit(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    session: &str,
    suffix: u16,
    text: &str,
) -> u64 {
    let chapter_id = session_chapter(admin, session).await;
    let head = session_chapter_head(admin, session).await;
    apply_named_edit(
        store,
        scope,
        NamedEdit {
            editor_session_id: session,
            chapter_id: &chapter_id,
            expected_revision_id: &head,
            suffix: &format!("{suffix:04x}"),
            local_intent_sequence: u64::from(suffix),
            text,
            proposal_target: None,
        },
    )
    .await;
    latest_forward(admin, scope).await
}

/// One Author Undo with the current Author Undo Frontier and the current Chapter Head.
async fn undo_latest(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    session: &str,
    suffix: u16,
) -> UndoLatestAuthorActionSettlement {
    let input = UndoLatestAuthorActionInput {
        editor_session_id: EditorSessionId::new(session),
        expected_author_undo_frontier_sequence: latest_forward(admin, scope).await,
        expected_authoritative_revision_id: session_chapter_head(admin, session).await,
    };
    let call = issued_undo(store, scope, suffix, input).await;
    undo(store, &call).await.unwrap()
}

/// The compensated source of an applied Author Undo and the Frontier after it. A Reversal
/// Proposal is not a Compensation.
fn compensated(settlement: UndoLatestAuthorActionSettlement) -> (u64, Option<u64>) {
    match settlement.outcome {
        TransitionOutcome::Applied(applied)
            if !matches!(applied.records, UndoRecords::ReversalRequired { .. }) =>
        {
            (
                applied.source_sequence,
                applied.author_undo_frontier_sequence,
            )
        }
        other => panic!("the Undo must compensate, got {other:?}"),
    }
}

async fn session_chapter(admin: &Client, session: &str) -> String {
    admin
        .query_one(
            "SELECT chapter_object_id::text FROM storyos.editor_session_base_snapshots
              WHERE editor_session_id = $1::text::uuid",
            &[&session],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// The payload and the ordered Block identities of the Head of the Chapter of the session.
async fn chapter_content(admin: &Client, session: &str) -> (String, Vec<String>) {
    let head = session_chapter_head(admin, session).await;
    let payload: String = admin
        .query_one(
            "SELECT convert_from(payload.canonical_bytes, 'UTF8')
               FROM storyos.authoritative_revisions AS revision
               JOIN storyos.authoritative_payloads AS payload
                 ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
                    (revision.owner_user_id, revision.project_id, revision.payload_id)
              WHERE revision.revision_id = $1::text::uuid",
            &[&head],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let blocks = admin
        .query(
            "SELECT manuscript_block_id::text FROM storyos.manuscript_revision_members
              WHERE revision_id = $1::text::uuid ORDER BY block_order",
            &[&head],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(/*idx*/ 0))
        .collect();
    (payload, blocks)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_reverses_three_prose_edits_in_reverse_order() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _chapter_a, _chapter_b, _revision_b, session) =
        two_chapter_writer(&store, /*base*/ 0xa700).await;
    let before = chapter_content(&admin, &session).await;
    let prior_frontier = latest_forward(&admin, &scope).await;
    let first = edit(
        &store, &admin, &scope, &session, /*suffix*/ 0xa709, "one ",
    )
    .await;
    let second = edit(
        &store, &admin, &scope, &session, /*suffix*/ 0xa70a, "two ",
    )
    .await;
    let third = edit(
        &store, &admin, &scope, &session, /*suffix*/ 0xa70b, "three ",
    )
    .await;

    let mut steps = Vec::new();
    for suffix in [0xa70c, 0xa70d, 0xa70e] {
        steps.push(compensated(
            undo_latest(&store, &admin, &scope, &session, suffix).await,
        ));
    }

    assert_eq!(
        (steps, chapter_content(&admin, &session).await),
        (
            vec![
                (third, Some(second)),
                (second, Some(first)),
                (first, Some(prior_frontier)),
            ],
            before
        )
    );
}

/// A new edit after an Undo, and the Undo of that edit, do not stop the earlier chain.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_continues_after_a_later_edit_is_also_undone() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _chapter_a, _chapter_b, _revision_b, session) =
        two_chapter_writer(&store, /*base*/ 0xa720).await;
    let before = chapter_content(&admin, &session).await;
    let first = edit(
        &store, &admin, &scope, &session, /*suffix*/ 0xa729, "one ",
    )
    .await;
    let second = edit(
        &store, &admin, &scope, &session, /*suffix*/ 0xa72a, "two ",
    )
    .await;
    let undo_second =
        compensated(undo_latest(&store, &admin, &scope, &session, /*suffix*/ 0xa72b).await);
    let third = edit(
        &store, &admin, &scope, &session, /*suffix*/ 0xa72c, "three ",
    )
    .await;
    let undo_third =
        compensated(undo_latest(&store, &admin, &scope, &session, /*suffix*/ 0xa72d).await);
    let undo_first =
        compensated(undo_latest(&store, &admin, &scope, &session, /*suffix*/ 0xa72e).await);

    assert_eq!(
        (
            [undo_second.0, undo_third.0, undo_first.0],
            chapter_content(&admin, &session).await
        ),
        ([second, third, first], before)
    );
}

/// A Barrier between two prose edits stops the chain after the later edit.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_barrier_between_two_prose_edits_stops_the_chain() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, input) = pending_proposal(&store, &admin, /*base*/ 0xa740).await;
    let session = input.editor_session_id.as_ref().to_owned();
    edit(
        &store, &admin, &scope, &session, /*suffix*/ 0xa74a, "one ",
    )
    .await;
    let after_first = chapter_content(&admin, &session).await;
    let rejection = issued(
        &store,
        &scope,
        /*suffix*/ 0xa74b,
        &REJECT_PROPOSAL_OPERATIONS,
        input,
    )
    .await;
    reject_proposal_operations(&store, &rejection)
        .await
        .unwrap();
    let barrier = latest_forward(&admin, &scope).await;
    let second = edit(
        &store, &admin, &scope, &session, /*suffix*/ 0xa74c, "two ",
    )
    .await;
    let undo_second =
        compensated(undo_latest(&store, &admin, &scope, &session, /*suffix*/ 0xa74d).await);
    let stopped = undo_latest(&store, &admin, &scope, &session, /*suffix*/ 0xa74e)
        .await
        .outcome;

    assert_eq!(
        (
            undo_second,
            stopped,
            chapter_content(&admin, &session).await
        ),
        (
            (second, Some(barrier)),
            TransitionOutcome::Refused(UndoLatestAuthorActionUnavailable::Barrier),
            after_first
        )
    );
}

/// An Acceptance in the chain is compensated after the Undo of a later edit of its Chapter.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_compensates_an_acceptance_after_the_undo_of_a_later_edit() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, input, _chapter_a_head) =
        acceptable_proposal(&store, &admin, /*base*/ 0xa760).await;
    let run_id: String = admin
        .query_one(
            "SELECT source_run_id::text FROM storyos.proposals WHERE proposal_id = $1::text::uuid",
            &[&input.proposal_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    seed_run(&admin, &scope, &run_id, "completed").await;
    let session = input.editor_session_id.as_ref().to_owned();
    let call = issued(
        &store,
        &scope,
        /*suffix*/ 0xa769,
        &ACCEPT_PROPOSAL,
        input,
    )
    .await;
    // The writer Editor Session edits the accepted Chapter B, as an author who accepts in the
    // Chapter does.
    edit_in_chapter_of(
        &admin,
        &session,
        &call.input.expected_authoritative_revision_id,
    )
    .await;
    let before = chapter_content(&admin, &session).await;
    accept_proposal(&store, &call).await.unwrap();
    let accepted = resulting_revision(&admin, &call.envelope.ids.receipt_id).await;
    edit_in_chapter_of(&admin, &session, &accepted).await;
    let acceptance = latest_forward(&admin, &scope).await;
    let second = edit(
        &store, &admin, &scope, &session, /*suffix*/ 0xa76a, "two ",
    )
    .await;
    let undo_second =
        compensated(undo_latest(&store, &admin, &scope, &session, /*suffix*/ 0xa76b).await);
    let undo_acceptance =
        compensated(undo_latest(&store, &admin, &scope, &session, /*suffix*/ 0xa76c).await);

    assert_eq!(
        (
            undo_second.0,
            undo_acceptance.0,
            chapter_content(&admin, &session).await
        ),
        (second, acceptance, before)
    );
}
