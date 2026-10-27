//! Author Undo of a Proposal candidate edit after the Undo of a later Acceptance (ADR 0068).
//! Each Undo takes the current Author Undo Frontier, as the Web Client does.

use storyos_application::{AcceptProposalInput, EditorSessionId, UndoLatestAuthorActionSettlement};
use storyos_core::{TransitionOutcome, UndoLatestAuthorActionUnavailable};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;

use super::acceptance::{ACCEPT_PROPOSAL, accept_proposal};
use super::agent_run::seed_run;
use super::proposal_decision_undo_chain::{conflict_and_replan, head_state};
use super::proposal_undo_chain::{compensated, edit, on, undo_call};
use super::secondary_operation::{
    TwoOperationProposal, proposal_head, proposal_head_revision, two_operation_proposal,
};
use super::support::{issued, run_without_foreign_keys, stores};
use super::undo::{latest_forward, undo};

/// The Proposal fixture with its AgentRun, which a derived Proposal copies.
async fn derivable_proposal(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> TwoOperationProposal {
    let proposal = two_operation_proposal(store, admin, base).await;
    let run_id: String = admin
        .query_one(
            "SELECT source_run_id::text FROM storyos.proposals WHERE proposal_id = $1::text::uuid",
            &[&proposal.proposal_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    seed_run(admin, &proposal.scope, &run_id, "completed").await;
    proposal
}

/// Accepts the selected Operations of the current Proposal head and returns the Acceptance.
/// The Chapter head of `proposal` moves to the accepted Revision.
async fn accept(
    store: &PostgresProjectReader,
    admin: &Client,
    proposal: &mut TwoOperationProposal,
    operations: &[usize],
    suffix: u16,
) -> u64 {
    let (proposal_revision_id, validation_receipt_id) =
        proposal_head(admin, &proposal.proposal_id).await;
    let call = issued(
        store,
        &proposal.scope,
        suffix,
        &ACCEPT_PROPOSAL,
        AcceptProposalInput {
            editor_session_id: EditorSessionId::new(proposal.editor_session_id.clone()),
            proposal_id: proposal.proposal_id.clone(),
            proposal_revision_id,
            validation_receipt_id,
            selected_operation_ids: operations
                .iter()
                .map(|index| proposal.operations[*index].0.clone())
                .collect(),
            expected_authoritative_revision_id: proposal.chapter_revision_id.clone(),
        },
    )
    .await;
    let accepted = accept_proposal(store, &call).await.unwrap();
    assert!(
        matches!(accepted.outcome, TransitionOutcome::Applied(_)),
        "the Acceptance must apply, got {:?}",
        accepted.outcome
    );
    proposal.chapter_revision_id = chapter_head(admin, &proposal.chapter_id).await;
    latest_forward(admin, &proposal.scope).await
}

/// One Author Undo of the current Frontier. The Chapter head of `proposal` follows it.
async fn undo_latest(
    store: &PostgresProjectReader,
    admin: &Client,
    proposal: &mut TwoOperationProposal,
    suffix: u16,
) -> Result<UndoLatestAuthorActionSettlement, String> {
    let call = undo_call(
        store,
        admin,
        &proposal.scope,
        &proposal.editor_session_id,
        &proposal.chapter_revision_id,
        suffix,
    )
    .await;
    let settled = undo(store, &call)
        .await
        .map_err(|error| format!("{error:?}"))?;
    proposal.chapter_revision_id = chapter_head(admin, &proposal.chapter_id).await;
    Ok(settled)
}

/// The compensated source of an Author Undo that must compensate.
async fn undo_source(
    store: &PostgresProjectReader,
    admin: &Client,
    proposal: &mut TwoOperationProposal,
    suffix: u16,
) -> u64 {
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
    proposal.chapter_revision_id = chapter_head(admin, &proposal.chapter_id).await;
    compensated(&settled).0
}

async fn chapter_head(admin: &Client, chapter_id: &str) -> String {
    admin
        .query_one(
            "SELECT current_revision_id::text FROM storyos.authoritative_heads
              WHERE manuscript_object_id = $1::text::uuid",
            &[&chapter_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// The parent and the base of one Proposal Revision.
async fn revision_lineage(admin: &Client, proposal_id: &str, revision_id: &str) -> [String; 2] {
    let row = admin
        .query_one(
            "SELECT revision.parent_revision_id::text,
                    revision.base_authoritative_revision_id::text
               FROM storyos.proposal_revisions AS revision
              WHERE revision.proposal_id = $1::text::uuid
                AND revision.revision_id = $2::text::uuid",
            &[&proposal_id, &revision_id],
        )
        .await
        .unwrap();
    [row.get(/*idx*/ 0), row.get(/*idx*/ 1)]
}

/// One column of each Operation of the Proposal fixture, primary first. A derived Proposal
/// copies the Operation ids, so the query also names the Proposal.
async fn operation_values(
    admin: &Client,
    proposal: &TwoOperationProposal,
    column: &str,
) -> Vec<String> {
    let mut values = Vec::new();
    for (operation_id, _) in &proposal.operations {
        values.push(
            admin
                .query_one(
                    &format!(
                        "SELECT {column} FROM storyos.proposal_operations
                          WHERE proposal_id = $1::text::uuid AND operation_id = $2::text::uuid"
                    ),
                    &[&proposal.proposal_id, operation_id],
                )
                .await
                .unwrap()
                .get(/*idx*/ 0),
        );
    }
    values
}

async fn operation_candidates(admin: &Client, proposal: &TwoOperationProposal) -> Vec<String> {
    operation_values(admin, proposal, "candidate_text").await
}

async fn operation_resolutions(admin: &Client, proposal: &TwoOperationProposal) -> Vec<String> {
    operation_values(admin, proposal, "resolution").await
}

/// E1, Acceptance, Undo of the Acceptance, Undo of E1. The Undo of the Acceptance reopens the
/// Proposal on the new Chapter Revision. The Undo of E1 restores the candidate before E1 on the
/// base of that reopen Revision.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_reverses_a_candidate_edit_after_the_undo_of_its_acceptance() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4e00).await;
    let before = operation_candidates(&admin, &proposal).await;
    let first = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 0, /*suffix*/ 0x4e09, /*local_intent_sequence*/ 3,
        ),
    )
    .await;
    let acceptance = accept(
        &store,
        &admin,
        &mut proposal,
        &[0, 1],
        /*suffix*/ 0x4e0a,
    )
    .await;
    let undo_acceptance = undo_source(&store, &admin, &mut proposal, /*suffix*/ 0x4e0b).await;
    let reopened = proposal_head_revision(&admin, &proposal.proposal_id).await;
    let undo_first = undo_source(&store, &admin, &mut proposal, /*suffix*/ 0x4e0c).await;
    let restored = proposal_head_revision(&admin, &proposal.proposal_id).await;

    assert_eq!(
        (
            (undo_acceptance, undo_first),
            operation_candidates(&admin, &proposal).await,
            operation_resolutions(&admin, &proposal).await,
            revision_lineage(&admin, &proposal.proposal_id, &restored).await,
        ),
        (
            (acceptance, first),
            before,
            vec!["pending".to_owned(), "pending".to_owned()],
            [reopened, proposal.chapter_revision_id.clone()],
        )
    );
}

/// E1, Acceptance, Undo of the Acceptance, E2, Undo of E2, Undo of E1. The Acceptance step is in
/// the middle of the walk from the Proposal head to the Revision of E1.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_walk_goes_through_an_undone_acceptance_in_the_middle_of_the_lineage() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4e20).await;
    let before = operation_candidates(&admin, &proposal).await;
    let first = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4e29, /*local_intent_sequence*/ 3,
        ),
    )
    .await;
    accept(
        &store,
        &admin,
        &mut proposal,
        &[0, 1],
        /*suffix*/ 0x4e2a,
    )
    .await;
    undo_source(&store, &admin, &mut proposal, /*suffix*/ 0x4e2b).await;
    let second = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4e2c, /*local_intent_sequence*/ 4,
        ),
    )
    .await;
    let undo_second = undo_source(&store, &admin, &mut proposal, /*suffix*/ 0x4e2d).await;
    let undo_first = undo_source(&store, &admin, &mut proposal, /*suffix*/ 0x4e2e).await;

    assert_eq!(
        (
            (undo_second, undo_first),
            operation_candidates(&admin, &proposal).await
        ),
        ((second, first), before)
    );
}

/// The Undo of an Acceptance that did not reopen the same Proposal leaves its Operations
/// `applied`. The Undo of an earlier candidate edit of that Proposal is then a Barrier and
/// changes nothing.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_derived_or_unlinked_acceptance_undo_makes_the_earlier_edit_a_barrier() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut stopped = Vec::new();
    for (base, drift) in [
        // The accepted lineage drifted, so the Undo derives a new Proposal.
        (
            0x4e40_u16,
            "UPDATE storyos.proposal_revisions SET closure = 'withdrawn'
              WHERE revision_id = '{accepted}'::uuid",
        ),
        // Another Proposal reserves the Block, so the Undo links no Proposal.
        (
            0x4e60_u16,
            "INSERT INTO storyos.proposals
               (owner_user_id, project_id, proposal_id, kind, chapter_id, manuscript_block_id,
                source_run_id, source_decision_id)
             SELECT owner_user_id, project_id, '{other}', kind, chapter_id, manuscript_block_id,
                    source_run_id, source_decision_id
               FROM storyos.proposals WHERE proposal_id = '{proposal_id}'::uuid;
             INSERT INTO storyos.proposal_operations
               (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                resolution, reservation_state, candidate_text)
             SELECT owner_user_id, project_id, '{other}', gen_random_uuid(), manuscript_block_id,
                    'pending', 'unresolved', candidate_text
               FROM storyos.proposal_operations WHERE proposal_id = '{proposal_id}'::uuid",
        ),
    ] {
        let mut proposal = derivable_proposal(&store, &admin, base).await;
        edit(
            &store,
            &admin,
            &proposal,
            on(
                /*operation*/ 0,
                base + 9,
                /*local_intent_sequence*/ 3,
            ),
        )
        .await;
        let accepted = proposal_head_revision(&admin, &proposal.proposal_id).await;
        accept(&store, &admin, &mut proposal, &[0, 1], base + 10).await;
        run_without_foreign_keys(
            &admin,
            &drift
                .replace("{accepted}", &accepted)
                .replace("{other}", &Uuid::now_v7().to_string())
                .replace("{proposal_id}", &proposal.proposal_id),
        )
        .await;
        undo_source(&store, &admin, &mut proposal, base + 11).await;
        let candidates = operation_candidates(&admin, &proposal).await;
        let outcome = undo_latest(&store, &admin, &mut proposal, base + 12)
            .await
            .map(|settled| settled.outcome);
        stopped.push((
            outcome,
            proposal_head_revision(&admin, &proposal.proposal_id).await,
            operation_candidates(&admin, &proposal).await,
            operation_resolutions(&admin, &proposal).await,
        ));
        assert_eq!(stopped.last().unwrap().2, candidates);
    }

    let barrier = Ok(TransitionOutcome::Refused(
        UndoLatestAuthorActionUnavailable::Barrier,
    ));
    assert_eq!(
        stopped
            .iter()
            .map(|(outcome, _, _, resolutions)| (outcome.clone(), resolutions.clone()))
            .collect::<Vec<_>>(),
        vec![
            (
                barrier.clone(),
                vec!["applied".to_owned(), "applied".to_owned()]
            ),
            (barrier, vec!["applied".to_owned(), "applied".to_owned()]),
        ]
    );
}

/// A partial Acceptance that did not select the Operation of E1, and a derived Undo of it. The
/// Operation of E1 stays `pending` in the original Proposal, so the Undo of E1 compensates there.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_undo_of_an_unselected_edit_compensates_after_a_derived_acceptance_undo() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut proposal = derivable_proposal(&store, &admin, /*base*/ 0x4ea0).await;
    let before = operation_candidates(&admin, &proposal).await;
    let first = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4ea9, /*local_intent_sequence*/ 3,
        ),
    )
    .await;
    accept(&store, &admin, &mut proposal, &[0], /*suffix*/ 0x4eaa).await;
    let second = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4eab, /*local_intent_sequence*/ 4,
        ),
    )
    .await;
    let undo_second = undo_source(&store, &admin, &mut proposal, /*suffix*/ 0x4eac).await;
    // The head moved after the Acceptance, so its Undo derives a new Proposal.
    undo_source(&store, &admin, &mut proposal, /*suffix*/ 0x4ead).await;
    let undo_first = undo_source(&store, &admin, &mut proposal, /*suffix*/ 0x4eae).await;

    assert_eq!(
        (
            (undo_second, undo_first),
            operation_candidates(&admin, &proposal).await,
            operation_resolutions(&admin, &proposal).await,
        ),
        (
            (second, first),
            before,
            vec!["applied".to_owned(), "pending".to_owned()]
        )
    );
}

/// Replan D, candidate edit E, Acceptance, Undo of the Acceptance, Undo of E, Undo of D. The edit
/// gives the replanned Proposal a Validation Receipt, so the Acceptance is possible. The Undo of D
/// restores the Revision before D with its base.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_restores_a_replan_after_the_undo_of_an_acceptance_of_its_edit() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4ec0).await;
    let before = operation_candidates(&admin, &proposal).await;
    let (replanned, before_replan) =
        conflict_and_replan(&store, &admin, &proposal, /*suffix*/ 0x4ec9).await;
    let edited = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 0, /*suffix*/ 0x4eca, /*local_intent_sequence*/ 3,
        ),
    )
    .await;
    accept(
        &store,
        &admin,
        &mut proposal,
        &[0, 1],
        /*suffix*/ 0x4ecb,
    )
    .await;
    undo_source(&store, &admin, &mut proposal, /*suffix*/ 0x4ecc).await;
    let undo_edit = undo_source(&store, &admin, &mut proposal, /*suffix*/ 0x4ecd).await;
    let undo_replan = undo_source(&store, &admin, &mut proposal, /*suffix*/ 0x4ece).await;

    assert_eq!(
        (
            (undo_edit, undo_replan),
            head_state(&admin, &proposal.proposal_id).await,
            operation_candidates(&admin, &proposal).await,
        ),
        ((edited, replanned), before_replan, before)
    );
}
