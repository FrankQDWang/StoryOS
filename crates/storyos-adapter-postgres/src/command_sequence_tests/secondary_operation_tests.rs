//! Acceptance and Author Undo of an Author Edit of a secondary Proposal Operation.

use storyos_application::{
    AcceptProposalInput, AuthorEditProposalTarget, ChapterId, EditorSessionId, OpenChapter,
    ProjectScope, UndoLatestAuthorActionInput, open_chapter,
};
use storyos_core::{AuthorEditPrimitive, AuthorEditUnit, SelectionSnapshot, TransitionOutcome};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::create_volume_authority_tests::{NamedEdit, adjusted_edit_command, apply_named_edit};

use super::acceptance::{ACCEPT_PROPOSAL, accept_proposal};
use super::support::{issued, run_without_foreign_keys, stores, two_chapter_writer};
use super::undo::{edit_in_chapter_of, issued_undo, latest_forward, resulting_revision, undo};

const PRIMARY_TEXT: &str = "Primary candidate";
const SECONDARY_TEXT: &str = "Secondary candidate";

/// A new Project whose Chapter B has two Blocks, and one valid Proposal with one pending
/// Operation for each Block. The Block of the Proposal is the first Block.
pub(super) struct TwoOperationProposal {
    pub(super) scope: ProjectScope,
    pub(super) editor_session_id: String,
    pub(super) chapter_id: String,
    /// The Chapter head on which the Proposal is based.
    pub(super) chapter_revision_id: String,
    pub(super) proposal_id: String,
    /// The primary and then the secondary Operation, each with its Block.
    pub(super) operations: [(String, String); 2],
}

pub(super) async fn two_operation_proposal(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> TwoOperationProposal {
    let (scope, _chapter_a, chapter_id, revision_b, editor_session_id) =
        two_chapter_writer(store, base).await;
    edit_in_chapter_of(admin, &editor_session_id, &revision_b).await;
    let typed = format!("{:04x}", base + 5);
    apply_named_edit(
        store,
        &scope,
        NamedEdit {
            editor_session_id: &editor_session_id,
            chapter_id: &chapter_id,
            expected_revision_id: &revision_b,
            suffix: &typed,
            local_intent_sequence: 1,
            text: "Hello World",
            proposal_target: None,
        },
    )
    .await;
    let typed_revision =
        resulting_revision(admin, &format!("018f0000-0000-7001-8000-00000003{typed}")).await;
    let first_block = chapter_blocks(store, &scope, &chapter_id).await[0]
        .0
        .clone();
    let split = adjusted_edit_command(
        store,
        &scope,
        NamedEdit {
            editor_session_id: &editor_session_id,
            chapter_id: &chapter_id,
            expected_revision_id: &typed_revision,
            suffix: &format!("{:04x}", base + 6),
            local_intent_sequence: 2,
            text: "",
            proposal_target: None,
        },
        |command| {
            command.author_edit_units = vec![AuthorEditUnit {
                normalized_primitives: vec![AuthorEditPrimitive::SplitBlock {
                    manuscript_block_id: first_block,
                    offset: 6,
                    new_manuscript_block_id: Uuid::now_v7().to_string(),
                }],
                selection_snapshot: SelectionSnapshot {
                    ordered_selection: None,
                    coordinate_profile: storyos_core::UTF16_COORDINATE_PROFILE.to_owned(),
                    from: 6,
                    to: 6,
                },
            }];
        },
    )
    .await;
    store.apply_author_edit(&split).await.unwrap();
    let chapter_revision_id = resulting_revision(admin, &split.ids.receipt_id).await;
    let blocks = chapter_blocks(store, &scope, &chapter_id).await;
    let [(primary_block, _), (secondary_block, _)] = blocks.as_slice() else {
        panic!("Chapter B must have two Blocks, got {blocks:?}");
    };
    let proposal_id = Uuid::now_v7().to_string();
    let operations = [
        (Uuid::now_v7().to_string(), primary_block.clone()),
        (Uuid::now_v7().to_string(), secondary_block.clone()),
    ];
    run_without_foreign_keys(
        admin,
        &format!(
            "INSERT INTO storyos.proposals
               (owner_user_id, project_id, proposal_id, kind, chapter_id, manuscript_block_id,
                source_run_id, source_decision_id)
             VALUES ('{owner}', '{project}', '{proposal_id}', 'block_edit', '{chapter_id}',
                     '{primary_block}', '{run}', '{decision}');
             INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id)
             VALUES ('{owner}', '{project}', '{proposal_id}', '{revision}', 'ready', 'valid',
                     'open', '{PRIMARY_TEXT}', '{chapter_revision_id}');
             INSERT INTO storyos.proposal_heads
               (owner_user_id, project_id, proposal_id, current_revision_id)
             VALUES ('{owner}', '{project}', '{proposal_id}', '{revision}');
             INSERT INTO storyos.proposal_operations
               (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                resolution, reservation_state, candidate_text)
             VALUES ('{owner}', '{project}', '{proposal_id}', '{primary}', '{primary_block}',
                     'pending', 'unresolved', '{PRIMARY_TEXT}'),
                    ('{owner}', '{project}', '{proposal_id}', '{secondary}', '{secondary_block}',
                     'pending', 'unresolved', '{SECONDARY_TEXT}');
             INSERT INTO storyos.validation_receipts
               (owner_user_id, project_id, validation_receipt_id, proposal_id,
                proposal_revision_id, result, base_authoritative_revision_id,
                manuscript_block_id, candidate_text, reservation_state)
             VALUES ('{owner}', '{project}', '{validation}', '{proposal_id}', '{revision}',
                     'valid', '{chapter_revision_id}', '{primary_block}', '{PRIMARY_TEXT}',
                     'unresolved')",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            run = Uuid::now_v7(),
            decision = Uuid::now_v7(),
            revision = Uuid::now_v7(),
            primary = operations[0].0,
            secondary = operations[1].0,
            validation = Uuid::now_v7(),
        ),
    )
    .await;
    TwoOperationProposal {
        scope,
        editor_session_id,
        chapter_id,
        chapter_revision_id,
        proposal_id,
        operations,
    }
}

/// Prepends `Edited ` to the candidate of the secondary Operation through one Author Edit.
pub(super) async fn edit_secondary_candidate(
    store: &PostgresProjectReader,
    admin: &Client,
    proposal: &TwoOperationProposal,
    suffix: u16,
) {
    let (revision_id, _) = proposal_head(admin, &proposal.proposal_id).await;
    let (operation_id, block_id) = proposal.operations[1].clone();
    apply_named_edit(
        store,
        &proposal.scope,
        NamedEdit {
            editor_session_id: &proposal.editor_session_id,
            chapter_id: &proposal.chapter_id,
            expected_revision_id: &proposal.chapter_revision_id,
            suffix: &format!("{suffix:04x}"),
            local_intent_sequence: 3,
            text: "Edited ",
            proposal_target: Some(AuthorEditProposalTarget {
                proposal_id: proposal.proposal_id.clone(),
                operation_id,
                revision_id,
                manuscript_block_id: block_id,
            }),
        },
    )
    .await;
}

/// The id and the text of each Block of the current Chapter head, in Block order.
async fn chapter_blocks(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    chapter_id: &str,
) -> Vec<(String, String)> {
    let OpenChapter::Found(opened) =
        open_chapter(store, scope, &ChapterId::new(chapter_id.to_owned()))
            .await
            .unwrap()
    else {
        panic!("the Chapter must open");
    };
    opened
        .chapter
        .blocks
        .into_iter()
        .map(|block| (block.manuscript_block_id, block.text))
        .collect()
}

/// The current Revision of one Proposal and the id of its Validation Receipt.
async fn proposal_head(admin: &Client, proposal_id: &str) -> (String, String) {
    admin
        .query_one(
            "SELECT head.current_revision_id::text, receipt.validation_receipt_id::text
               FROM storyos.proposal_heads AS head
               JOIN storyos.validation_receipts AS receipt
                 ON (receipt.proposal_id, receipt.proposal_revision_id) =
                    (head.proposal_id, head.current_revision_id)
              WHERE head.proposal_id = $1::text::uuid",
            &[&proposal_id],
        )
        .await
        .map(|row| (row.get(/*idx*/ 0), row.get(/*idx*/ 1)))
        .unwrap()
}

/// The candidate of each Operation of one Proposal, primary first.
async fn operation_candidates(admin: &Client, proposal: &TwoOperationProposal) -> Vec<String> {
    let mut candidates = Vec::new();
    for (operation_id, _) in &proposal.operations {
        candidates.push(
            admin
                .query_one(
                    "SELECT candidate_text FROM storyos.proposal_operations
                      WHERE operation_id = $1::text::uuid",
                    &[operation_id],
                )
                .await
                .unwrap()
                .get(/*idx*/ 0),
        );
    }
    candidates
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn acceptance_of_an_edited_secondary_operation_applies_its_edited_candidate() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
