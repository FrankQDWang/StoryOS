//! The shared test support from `command_sequence_tests/secondary_operation_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::command_sequence_support::{run_without_foreign_keys, two_chapter_writer};
use crate::shared_tests::command_sequence_undo::{edit_in_chapter_of, resulting_revision};
use crate::shared_tests::create_volume_authority::{
    NamedEdit, adjusted_edit_command, apply_named_edit,
};
use crate::statement_cache::Client;
use storyos_application::{
    AuthorEditProposalTarget, AuthorEditSettlement, AuthorEditSettlementEffect, ChapterId,
    OpenChapter, ProjectScope, open_chapter,
};
use storyos_core::{AuthorEditPrimitive, AuthorEditUnit, SelectionSnapshot};
use uuid::Uuid;

pub(crate) const PRIMARY_TEXT: &str = "Primary candidate";

pub(crate) const SECONDARY_TEXT: &str = "Secondary candidate";

/// A new Project whose Chapter B has two Blocks, and one valid Proposal with one pending
/// Operation for each Block. The Block of the Proposal is the first Block.
pub(crate) struct TwoOperationProposal {
    pub(crate) scope: ProjectScope,
    pub(crate) editor_session_id: String,
    pub(crate) chapter_id: String,
    /// The Chapter head on which the Proposal is based.
    pub(crate) chapter_revision_id: String,
    pub(crate) proposal_id: String,
    /// The primary and then the secondary Operation, each with its Block.
    pub(crate) operations: [(String, String); 2],
}

pub(crate) async fn two_operation_proposal(
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

/// The two-operation fixture without its secondary Operation, so that only the first Block of
/// Chapter B is reserved. `operations[1]` keeps the id of the second Block.
pub(crate) async fn one_operation_proposal(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> TwoOperationProposal {
    let proposal = two_operation_proposal(store, admin, base).await;
    admin
        .execute(
            "DELETE FROM storyos.proposal_operations WHERE operation_id = $1::text::uuid",
            &[&proposal.operations[1].0],
        )
        .await
        .unwrap();
    proposal
}

/// The text that `edit_block` inserts at the start of a Block.
pub(crate) const BLOCK_EDIT: &str = "Again ";

/// One versioned Author Edit that inserts `BLOCK_EDIT` at the start of the Block of
/// `operations[block]` on the Chapter head of `proposal`. The edit acknowledges
/// `acknowledged_heads`, and observes the `mixed` partition when it acknowledges a head.
pub(crate) async fn edit_block(
    store: &PostgresProjectReader,
    proposal: &TwoOperationProposal,
    block: usize,
    suffix: u16,
    local_intent_sequence: u64,
    acknowledged_heads: Vec<String>,
) -> AuthorEditSettlement {
    let block_id = proposal.operations[block].1.clone();
    let command = adjusted_edit_command(
        store,
        &proposal.scope,
        NamedEdit {
            editor_session_id: &proposal.editor_session_id,
            chapter_id: &proposal.chapter_id,
            expected_revision_id: &proposal.chapter_revision_id,
            suffix: &format!("{suffix:04x}"),
            local_intent_sequence,
            text: "",
            proposal_target: None,
        },
        |command| {
            command.observed_ownership_partition = if acknowledged_heads.is_empty() {
                "authoritative"
            } else {
                "mixed"
            }
            .to_owned();
            command.expected_proposal_head_revision_ids = acknowledged_heads;
            command.author_edit_units = vec![AuthorEditUnit {
                normalized_primitives: vec![AuthorEditPrimitive::ReplaceBlockSelection {
                    manuscript_block_id: block_id,
                    from: 0,
                    to: 0,
                    text: BLOCK_EDIT.to_owned(),
                }],
                selection_snapshot: SelectionSnapshot {
                    coordinate_profile: storyos_core::UTF16_COORDINATE_PROFILE.to_owned(),
                    from: 0,
                    to: 0,
                },
            }];
        },
    )
    .await;
    store.apply_author_edit(&command).await.unwrap()
}

/// The one-operation fixture after a versioned edit of its unreserved second Block. The Chapter
/// head of the result is the edited Revision, and the Proposal keeps its earlier base, which this
/// returns too.
pub(crate) async fn earlier_base_proposal(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> (TwoOperationProposal, String) {
    let mut proposal = one_operation_proposal(store, admin, base).await;
    let edited = edit_block(
        store,
        &proposal,
        /*block*/ 1,
        base + 7,
        /*local_intent_sequence*/ 3,
        Vec::new(),
    )
    .await;
    let AuthorEditSettlementEffect::AuthoritativeApplied { ids, .. } = edited.effect else {
        panic!("the edit of the unreserved Block must apply, got {edited:?}");
    };
    let earlier = std::mem::replace(&mut proposal.chapter_revision_id, ids.revision_id);
    (proposal, earlier)
}

/// Prepends `Edited ` to the candidate of the secondary Operation through one Author Edit.
pub(crate) async fn edit_secondary_candidate(
    store: &PostgresProjectReader,
    admin: &Client,
    proposal: &TwoOperationProposal,
    suffix: u16,
) {
    edit_operation_candidate(
        store,
        admin,
        proposal,
        CandidateEdit {
            operation: 1,
            suffix,
            local_intent_sequence: 3,
            text: "Edited ",
        },
    )
    .await;
}

/// One Author Edit that prepends `text` to the candidate of one Operation of a Proposal.
pub(crate) struct CandidateEdit<'a> {
    /// The index of the Operation: 0 for the primary and 1 for the secondary Operation.
    pub(crate) operation: usize,
    pub(crate) suffix: u16,
    pub(crate) local_intent_sequence: u64,
    pub(crate) text: &'a str,
}

/// Applies `edit` on the current Proposal head.
pub(crate) async fn edit_operation_candidate(
    store: &PostgresProjectReader,
    admin: &Client,
    proposal: &TwoOperationProposal,
    edit: CandidateEdit<'_>,
) {
    let revision_id = proposal_head_revision(admin, &proposal.proposal_id).await;
    let (operation_id, block_id) = proposal.operations[edit.operation].clone();
    apply_named_edit(
        store,
        &proposal.scope,
        NamedEdit {
            editor_session_id: &proposal.editor_session_id,
            chapter_id: &proposal.chapter_id,
            expected_revision_id: &proposal.chapter_revision_id,
            suffix: &format!("{:04x}", edit.suffix),
            local_intent_sequence: edit.local_intent_sequence,
            text: edit.text,
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
pub(crate) async fn chapter_blocks(
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

/// The current Revision of one Proposal.
pub(crate) async fn proposal_head_revision(admin: &Client, proposal_id: &str) -> String {
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

/// The current Revision of one Proposal and the id of its Validation Receipt.
pub(crate) async fn proposal_head(admin: &Client, proposal_id: &str) -> (String, String) {
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
pub(crate) async fn operation_candidates(
    admin: &Client,
    proposal: &TwoOperationProposal,
) -> Vec<String> {
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
