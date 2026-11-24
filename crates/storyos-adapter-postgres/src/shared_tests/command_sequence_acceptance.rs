//! The shared test support from `command_sequence_tests/acceptance_tests.rs`.

use crate::PostgresProjectReader;
use crate::accept_proposal::AcceptProposal;
use crate::shared_tests::command_sequence_support::{
    CommandCall, Route, SequenceError, issued, run_without_foreign_keys, two_chapter_writer,
};
use crate::statement_cache::Client;
use storyos_application::{
    AcceptProposalInput, AcceptProposalSettlement, EditorSessionId, ManuscriptBlock,
    ManuscriptBlockKind, ProjectCommandError, ProjectScope,
};
use storyos_contracts::CommandKind;
use uuid::Uuid;

pub(crate) const ACCEPT_PROPOSAL: Route = Route {
    kind: CommandKind::AcceptProposal,
    method: storyos_contracts::ACCEPT_PROPOSAL_METHOD,
    path: storyos_contracts::ACCEPT_PROPOSAL_PATH,
    schema: storyos_contracts::ACCEPT_PROPOSAL_REQUEST_SCHEMA_ID,
};

pub(crate) async fn accept_proposal(
    store: &PostgresProjectReader,
    call: &CommandCall<AcceptProposalInput>,
) -> Result<AcceptProposalSettlement, ProjectCommandError> {
    store
        .accept_proposal(&call.envelope, &call.input)
        .await
        .map_err(SequenceError::sequence)
}

/// A new Project with a writer Editor Session and one valid Proposal for the first Block of
/// Chapter B. The Proposal has one pending Operation and its valid Validation Receipt.
///
/// Returns the Scope, the input that accepts the Operation, and the head of Chapter A. The
/// fixture skips the foreign keys of the AgentRun.
pub(crate) async fn acceptable_proposal(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> (ProjectScope, AcceptProposalInput, String) {
    let (scope, _chapter_a, chapter_b, revision_b, editor_session_id) =
        two_chapter_writer(store, base).await;
    let block_id: String = admin
        .query_one(
            "SELECT manuscript_block_id::text FROM storyos.manuscript_revision_members
              WHERE revision_id = $1::text::uuid ORDER BY block_order LIMIT 1",
            &[&revision_b],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let input = AcceptProposalInput {
        editor_session_id: EditorSessionId::new(editor_session_id),
        proposal_id: Uuid::now_v7().to_string(),
        proposal_revision_id: Uuid::now_v7().to_string(),
        validation_receipt_id: Uuid::now_v7().to_string(),
        selected_operation_ids: vec![Uuid::now_v7().to_string()],
        expected_authoritative_revision_id: revision_b,
    };
    admin
        .batch_execute(&format!(
            "BEGIN;
             SET LOCAL session_replication_role = replica;
             INSERT INTO storyos.proposals
               (owner_user_id, project_id, proposal_id, kind, chapter_id, manuscript_block_id,
                source_run_id, source_decision_id)
             VALUES ('{owner}', '{project}', '{proposal}', 'block_edit', '{chapter}',
                     '{block}', '{run}', '{decision}');
             INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{revision}', 'ready', 'valid',
                     'open', 'Accepted text', '{base_revision}');
             INSERT INTO storyos.proposal_heads
               (owner_user_id, project_id, proposal_id, current_revision_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{revision}');
             INSERT INTO storyos.proposal_operations
               (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                resolution, reservation_state, candidate_text)
             VALUES ('{owner}', '{project}', '{proposal}', '{operation}', '{block}',
                     'pending', 'unresolved', 'Accepted text');
             INSERT INTO storyos.validation_receipts
               (owner_user_id, project_id, validation_receipt_id, proposal_id,
                proposal_revision_id, result, base_authoritative_revision_id,
                manuscript_block_id, candidate_text, reservation_state)
             VALUES ('{owner}', '{project}', '{validation}', '{proposal}', '{revision}',
                     'valid', '{base_revision}', '{block}', 'Accepted text', 'unresolved');
             COMMIT;",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            proposal = input.proposal_id,
            chapter = chapter_b,
            block = block_id,
            run = Uuid::now_v7(),
            decision = Uuid::now_v7(),
            revision = input.proposal_revision_id,
            base_revision = input.expected_authoritative_revision_id,
            operation = input.selected_operation_ids[0],
            validation = input.validation_receipt_id,
        ))
        .await
        .unwrap();
    let chapter_a_head = admin
        .query_one(
            "SELECT current_revision_id::text FROM storyos.authoritative_heads
              WHERE project_id = $1::text::uuid AND current_revision_id <> $2::text::uuid",
            &[
                &scope.project_id.as_ref(),
                &input.expected_authoritative_revision_id,
            ],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    (scope, input, chapter_a_head)
}

/// One applicable Acceptance in a new Project, as the command that the sequence settles.
pub(crate) async fn accept_proposal_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<AcceptProposal> {
    let (scope, input, _chapter_a_head) = acceptable_proposal(store, admin, base).await;
    let call = issued(store, &scope, base + 9, &ACCEPT_PROPOSAL, input).await;
    CommandCall {
        envelope: call.envelope,
        input: AcceptProposal::new(call.input),
    }
}

/// One Acceptance in a new Project whose Validation Receipt does not exist, which is invalid.
pub(crate) async fn invalid_acceptance_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<AcceptProposal> {
    let (scope, input, _chapter_a_head) = acceptable_proposal(store, admin, base).await;
    let input = AcceptProposalInput {
        validation_receipt_id: Uuid::now_v7().to_string(),
        ..input
    };
    let call = issued(store, &scope, base + 9, &ACCEPT_PROPOSAL, input).await;
    CommandCall {
        envelope: call.envelope,
        input: AcceptProposal::new(call.input),
    }
}

/// Counts the `acceptance_receipts` and `proposal_validation_conditions` rows of one Receipt.
pub(crate) async fn acceptance_rows(admin: &Client, receipt_id: &str) -> [i64; 2] {
    let row = admin
        .query_one(
            "SELECT (SELECT count(*) FROM storyos.acceptance_receipts
                      WHERE acceptance_receipt_id = $1::text::uuid),
                    (SELECT count(*) FROM storyos.proposal_validation_conditions
                      WHERE acceptance_receipt_id = $1::text::uuid)",
            &[&receipt_id],
        )
        .await
        .unwrap();
    [0, 1].map(|index| row.get(index))
}

/// Moves the head of the Chapter of `from` to a new Revision with these Blocks, as an edit that
/// another client settled. Returns the new Revision.
pub(crate) async fn advance_chapter_head(
    admin: &Client,
    from: &str,
    blocks: &[(String, String)],
) -> String {
    let revision = Uuid::now_v7();
    let payload = crate::manuscript_block::persist_canonical_bytes(
        &blocks
            .iter()
            .map(|(manuscript_block_id, text)| ManuscriptBlock {
                manuscript_block_id: manuscript_block_id.clone(),
                block_kind: ManuscriptBlockKind::Paragraph,
                text: text.clone(),
            })
            .collect::<Vec<_>>(),
    );
    let source = format!("FROM storyos.authoritative_revisions WHERE revision_id = '{from}'::uuid");
    let mut statements = vec![
        format!(
            "INSERT INTO storyos.authoritative_payloads
             SELECT owner_user_id, project_id, '{revision}'::uuid,
                    convert_to('{}', 'UTF8') {source}",
            payload.replace('\'', "''")
        ),
        format!(
            "INSERT INTO storyos.authoritative_revisions
             SELECT owner_user_id, project_id, manuscript_object_id, '{revision}'::uuid,
                    '{revision}'::uuid {source}"
        ),
    ];
    for (order, (block_id, _)) in (1..).zip(blocks) {
        statements.push(format!(
            "INSERT INTO storyos.manuscript_blocks
             SELECT owner_user_id, project_id, '{block_id}'::uuid, manuscript_object_id,
                    'paragraph' {source} ON CONFLICT DO NOTHING;
             INSERT INTO storyos.manuscript_revision_members
             SELECT owner_user_id, project_id, manuscript_object_id, '{revision}'::uuid,
                    '{block_id}'::uuid, {order} {source}"
        ));
    }
    statements.push(format!(
        "UPDATE storyos.authoritative_heads SET current_revision_id = '{revision}'::uuid
          WHERE current_revision_id = '{from}'::uuid"
    ));
    admin.batch_execute(&statements.join(";\n")).await.unwrap();
    revision.to_string()
}

/// Records a conflicted Acceptance condition of `condition_kind` on one Proposal Revision, as an
/// earlier Acceptance did. The fixture skips the foreign key to that Acceptance.
pub(crate) async fn seed_condition(
    admin: &Client,
    scope: &ProjectScope,
    proposal_id: &str,
    revision_id: &str,
    condition_kind: &str,
) {
    run_without_foreign_keys(
        admin,
        &format!(
            "INSERT INTO storyos.proposal_validation_conditions
               (owner_user_id, project_id, proposal_id, proposal_revision_id,
                acceptance_receipt_id, validation, conflict_id, condition_kind)
             VALUES ('{owner}', '{project}', '{proposal_id}', '{revision_id}', '{receipt}',
                     'conflicted', '{conflict}', '{condition_kind}')",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            receipt = Uuid::now_v7(),
            conflict = Uuid::now_v7(),
        ),
    )
    .await;
}
