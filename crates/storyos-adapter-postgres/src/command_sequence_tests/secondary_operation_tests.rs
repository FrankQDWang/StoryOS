//! Acceptance and Author Undo of an Author Edit of a secondary Proposal Operation.

use storyos_application::{AcceptProposalInput, EditorSessionId, UndoLatestAuthorActionInput};
use storyos_core::TransitionOutcome;

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_acceptance::{ACCEPT_PROPOSAL, accept_proposal};
use crate::shared_tests::command_sequence_secondary_operation::{
    PRIMARY_TEXT, SECONDARY_TEXT, chapter_blocks, edit_secondary_candidate, operation_candidates,
    proposal_head, two_operation_proposal,
};
use crate::shared_tests::command_sequence_support::{issued, stores};
use crate::shared_tests::command_sequence_undo::{issued_undo, latest_forward, undo};

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn acceptance_of_an_edited_secondary_operation_applies_its_edited_candidate() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0xe8a0).await;
    let before = chapter_blocks(&store, &proposal.scope, &proposal.chapter_id).await;
    edit_secondary_candidate(&store, &admin, &proposal, /*suffix*/ 0xe8a9).await;
    let (proposal_revision_id, validation_receipt_id) =
        proposal_head(&admin, &proposal.proposal_id).await;
    let call = issued(
        &store,
        &proposal.scope,
        /*suffix*/ 0xe8aa,
        &ACCEPT_PROPOSAL,
        AcceptProposalInput {
            editor_session_id: EditorSessionId::new(proposal.editor_session_id.clone()),
            proposal_id: proposal.proposal_id.clone(),
            proposal_revision_id,
            validation_receipt_id,
            selected_operation_ids: vec![proposal.operations[1].0.clone()],
            expected_authoritative_revision_id: proposal.chapter_revision_id.clone(),
        },
    )
    .await;
    let accepted = matches!(
        accept_proposal(&store, &call).await.unwrap().outcome,
        TransitionOutcome::Applied(_)
    );
    let mut expected = before;
    expected[1].1 = format!("Edited {SECONDARY_TEXT}");
    assert_eq!(
        (
            accepted,
            chapter_blocks(&store, &proposal.scope, &proposal.chapter_id).await
        ),
        (true, expected)
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_of_a_secondary_candidate_edit_restores_that_operation() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0xe8c0).await;
    let chapter_before = chapter_blocks(&store, &proposal.scope, &proposal.chapter_id).await;
    let split_sequence = latest_forward(&admin, &proposal.scope).await;
    edit_secondary_candidate(&store, &admin, &proposal, /*suffix*/ 0xe8c9).await;
    let (edited_revision_id, _) = proposal_head(&admin, &proposal.proposal_id).await;
    let edit_sequence = latest_forward(&admin, &proposal.scope).await;
    let call = issued_undo(
        &store,
        &proposal.scope,
        /*suffix*/ 0xe8ca,
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
    let (restored_revision_id, _) = proposal_head(&admin, &proposal.proposal_id).await;
    let restored = admin
        .query_one(
            "SELECT revision.parent_revision_id::text, revision.candidate_text,
                    receipt.result, receipt.manuscript_block_id::text, receipt.candidate_text
               FROM storyos.proposal_revisions AS revision
               JOIN storyos.validation_receipts AS receipt
                 ON (receipt.proposal_id, receipt.proposal_revision_id) =
                    (revision.proposal_id, revision.revision_id)
              WHERE revision.revision_id = $1::text::uuid",
            &[&restored_revision_id],
        )
        .await
        .map(|row| {
            (
                row.get::<_, String>(/*idx*/ 0),
                row.get::<_, String>(/*idx*/ 1),
                row.get::<_, String>(/*idx*/ 2),
                row.get::<_, String>(/*idx*/ 3),
                row.get::<_, String>(/*idx*/ 4),
            )
        })
        .unwrap();
    assert_eq!(
        (
            source_sequence,
            latest_forward(&admin, &proposal.scope).await,
            operation_candidates(&admin, &proposal).await,
            restored,
            chapter_blocks(&store, &proposal.scope, &proposal.chapter_id).await,
        ),
        (
            edit_sequence,
            split_sequence,
            vec![PRIMARY_TEXT.to_owned(), SECONDARY_TEXT.to_owned()],
            (
                edited_revision_id,
                PRIMARY_TEXT.to_owned(),
                "valid".to_owned(),
                proposal.operations[1].1.clone(),
                SECONDARY_TEXT.to_owned(),
            ),
            chapter_before,
        )
    );
}

/// A secondary candidate edit from before migration 0085 identifies no edited Operation. Its
/// candidate before the edit is not known, so Author Undo is unavailable.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_of_an_earlier_secondary_candidate_edit_is_unavailable() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0xe8e0).await;
    edit_secondary_candidate(&store, &admin, &proposal, /*suffix*/ 0xe8e9).await;
    let (edited_revision_id, _) = proposal_head(&admin, &proposal.proposal_id).await;
    admin
        .execute(
            "UPDATE storyos.proposal_revisions
                SET edited_operation_id = NULL, prior_operation_candidate_text = NULL
              WHERE revision_id = $1::text::uuid",
            &[&edited_revision_id],
        )
        .await
        .unwrap();
    let call = issued_undo(
        &store,
        &proposal.scope,
        /*suffix*/ 0xe8ea,
        UndoLatestAuthorActionInput {
            editor_session_id: EditorSessionId::new(proposal.editor_session_id.clone()),
            expected_author_undo_frontier_sequence: latest_forward(&admin, &proposal.scope).await,
            expected_authoritative_revision_id: proposal.chapter_revision_id.clone(),
        },
    )
    .await;
    let refused = matches!(
        undo(&store, &call).await.unwrap().outcome,
        TransitionOutcome::Refused(_)
    );
    assert_eq!(
        (
            refused,
            proposal_head(&admin, &proposal.proposal_id).await.0,
            operation_candidates(&admin, &proposal).await,
        ),
        (
            true,
            edited_revision_id,
            vec![PRIMARY_TEXT.to_owned(), format!("Edited {SECONDARY_TEXT}")],
        )
    );
}
