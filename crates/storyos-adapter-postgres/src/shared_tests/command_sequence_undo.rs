//! The shared test support from `command_sequence_tests/undo_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::command_sequence_acceptance::{
    ACCEPT_PROPOSAL, accept_proposal, acceptable_proposal,
};
use crate::shared_tests::command_sequence_agent_run::seed_run;
use crate::shared_tests::command_sequence_proposal_decision::{
    reject_proposal_operations, reject_proposal_operations_call, withdraw_proposal,
    withdraw_proposal_call,
};
use crate::shared_tests::command_sequence_secondary_operation::{
    edit_secondary_candidate, two_operation_proposal,
};
use crate::shared_tests::command_sequence_structure::{
    CREATE_VOLUME, create_volume, set_current_chapter_call,
};
use crate::shared_tests::command_sequence_support::{
    CommandCall, Route, issued, issued_with_bytes, run_without_foreign_keys, two_chapter_writer,
};
use crate::shared_tests::create_volume_authority::{NamedEdit, apply_named_edit};
use crate::statement_cache::Client;
use crate::undo_latest_author_action::UndoLatestAuthorAction;
use storyos_application::{
    AuthorEditProposalTarget, CreateVolumeInput, EditorSessionId, ProjectCommandError,
    ProjectScope, UndoLatestAuthorActionInput, UndoLatestAuthorActionSettlement,
};
use storyos_contracts::CommandKind;

pub(crate) const UNDO_LATEST_AUTHOR_ACTION: Route = Route {
    kind: CommandKind::UndoLatestAuthorAction,
    method: storyos_contracts::UNDO_LATEST_AUTHOR_ACTION_METHOD,
    path: storyos_contracts::UNDO_LATEST_AUTHOR_ACTION_PATH,
    schema: storyos_contracts::UNDO_LATEST_AUTHOR_ACTION_REQUEST_SCHEMA_ID,
};

/// The Undo cases of the contract rows. Each case settles one Forward Author Action, or a
/// state that one Author Undo outcome needs.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Case {
    CreateVolume,
    SetCurrentChapter,
    WithdrawProposal,
    ApplyAuthorEdit,
    /// An Author Edit of the candidate of a Proposal Operation.
    EditProposalCandidate,
    /// An Author Edit of the candidate of the secondary Operation of a two-operation Proposal.
    EditSecondaryCandidate,
    AcceptProposal,
    /// An Acceptance whose Chapter head moved back to the prior Revision.
    AcceptanceReversal,
    /// A stale Author Undo Frontier.
    StaleFrontier,
    /// A Forward action with a Barrier disposition.
    Barrier,
    /// An Acceptance Undo that expects a head that is not current.
    AcceptanceWrongHead,
    /// An Acceptance whose current Revision Envelope does not agree with its payload.
    AcceptanceUnavailable,
}

/// The latest Forward Author Action of the Project that no Compensation compensates.
pub(crate) async fn latest_forward(admin: &Client, scope: &ProjectScope) -> u64 {
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
pub(crate) async fn resulting_revision(admin: &Client, receipt_id: &str) -> String {
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
pub(crate) async fn session_chapter_head(admin: &Client, editor_session_id: &str) -> String {
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
pub(crate) async fn edit_in_chapter_of(admin: &Client, editor_session_id: &str, revision_id: &str) {
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
pub(crate) async fn undo_call(
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
                    proposal_target: None,
                },
            )
            .await;
            let resulting =
                resulting_revision(admin, &format!("018f0000-0000-7001-8000-00000003{suffix}"))
                    .await;
            (scope, editor_session_id, resulting)
        }
        Case::EditProposalCandidate => {
            let (scope, input, _chapter_a_head) = acceptable_proposal(store, admin, base).await;
            let (chapter_b, block_id): (String, String) = admin
                .query_one(
                    "SELECT chapter_id::text, manuscript_block_id::text FROM storyos.proposals
                      WHERE proposal_id = $1::text::uuid",
                    &[&input.proposal_id],
                )
                .await
                .map(|row| (row.get(/*idx*/ 0), row.get(/*idx*/ 1)))
                .unwrap();
            let editor_session_id = input.editor_session_id.as_ref().to_owned();
            // The writer Editor Session edits Chapter B, the Chapter of the Proposal.
            edit_in_chapter_of(
                admin,
                &editor_session_id,
                &input.expected_authoritative_revision_id,
            )
            .await;
            let suffix = format!("{:04x}", base + 9);
            apply_named_edit(
                store,
                &scope,
                NamedEdit {
                    editor_session_id: &editor_session_id,
                    chapter_id: &chapter_b,
                    expected_revision_id: &input.expected_authoritative_revision_id,
                    suffix: &suffix,
                    local_intent_sequence: 1,
                    text: "x",
                    proposal_target: Some(AuthorEditProposalTarget {
                        proposal_id: input.proposal_id.clone(),
                        operation_id: input.selected_operation_ids[0].clone(),
                        revision_id: input.proposal_revision_id.clone(),
                        manuscript_block_id: block_id,
                    }),
                },
            )
            .await;
            (
                scope,
                editor_session_id,
                input.expected_authoritative_revision_id,
            )
        }
        Case::EditSecondaryCandidate => {
            let proposal = two_operation_proposal(store, admin, base).await;
            edit_secondary_candidate(store, admin, &proposal, base + 9).await;
            (
                proposal.scope,
                proposal.editor_session_id,
                proposal.chapter_revision_id,
            )
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

/// Issues one Author Undo whose canonical bytes and digest are those of its command body.
pub(crate) async fn issued_undo(
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
pub(crate) fn undo_body(input: &UndoLatestAuthorActionInput) -> Vec<u8> {
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

pub(crate) async fn undo(
    store: &PostgresProjectReader,
    call: &CommandCall<UndoLatestAuthorActionInput>,
) -> Result<UndoLatestAuthorActionSettlement, ProjectCommandError> {
    store
        .undo_latest_author_action(&call.envelope, &call.input)
        .await
}
