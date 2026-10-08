use storyos_application::EditorSessionId;
use storyos_application::{
    CloseEditorFlowDraftInput, CloseEditorFlowDraftSettlement, ExpandRefusedEditDraftSettlement,
    ExpandRefusedEditDraftToProposalInput, ProjectCommandError, ProjectScope,
};
use storyos_core::{
    EXCLUSIVE_AUTHORITATIVE_EDGES_V1, OpenInlineProposalAnchor, PROSEMIRROR_TOKEN_UTF16_V1,
    ReceiptResult, proposal_anchor_base_slice_digest,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::command_sequence::ProjectCommand;

use super::damaged_evidence::{ReplayError, damaged_replay, replay_with_receipt_payload};
use super::support::{
    CommandCall, Route, SequenceError, issued, run_without_foreign_keys, settlement_rows, stores,
    two_chapter_writer, with_new_request_ids,
};

pub(super) const CLOSE_EDITOR_FLOW_DRAFT: Route = Route {
    kind: "closeEditorFlowDraft",
    method: "POST",
    path: storyos_contracts::CLOSE_EDITOR_FLOW_DRAFT_PATH,
    schema: storyos_contracts::CLOSE_EDITOR_FLOW_DRAFT_REQUEST_SCHEMA_ID,
};

pub(super) const EXPAND_REFUSED_EDIT_DRAFT: Route = Route {
    kind: "expandRefusedEditDraftToProposal",
    method: "POST",
    path: storyos_contracts::EXPAND_REFUSED_EDIT_DRAFT_PATH,
    schema: storyos_contracts::EXPAND_REFUSED_EDIT_DRAFT_REQUEST_SCHEMA_ID,
};

pub(super) async fn close_editor_flow_draft(
    store: &PostgresProjectReader,
    call: &CommandCall<CloseEditorFlowDraftInput>,
) -> Result<CloseEditorFlowDraftSettlement, ProjectCommandError> {
    store
        .close_editor_flow_draft(&call.envelope, &call.input)
        .await
}

/// A new Project with a writer Editor Session and one open Refused Edit Draft in `retention`.
///
/// Returns the Scope and the input that discards the Draft. The fixture skips the creation
/// Receipt and lifecycle event of the Draft.
pub(super) async fn refused_edit_draft(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
    retention: &str,
) -> (ProjectScope, CloseEditorFlowDraftInput) {
    let (scope, _chapter_a, chapter_b, revision_b, editor_session_id) =
        two_chapter_writer(store, base).await;
    let payload = serde_json::json!({
        "schema_revision": "storyos.refused-edit-payload.v1",
        "chapter_id": chapter_b,
        "expected_authoritative_revision_id": revision_b,
        "expected_proposal_head_revision_ids": [],
        "target_refs": [],
        "author_edit_units": [],
        "undo_group_id": Uuid::now_v7().to_string(),
        "completed_intent_record_id": Uuid::now_v7().to_string(),
        "local_intent_sequence": "1",
    });
    let input = CloseEditorFlowDraftInput {
        editor_session_id: EditorSessionId::new(editor_session_id),
        writer_generation: 1,
        draft_id: Uuid::now_v7().to_string(),
        source_current_draft_revision_id: Uuid::now_v7().to_string(),
        source_draft_payload_digest: storyos_core::hex_sha256(
            storyos_core::canonical_json(&payload).as_bytes(),
        ),
        source_reopen_event_id: None,
    };
    admin
        .batch_execute(&format!(
            "BEGIN;
             SET LOCAL session_replication_role = replica;
             INSERT INTO storyos.draft_artifact_revisions
               (owner_user_id, project_id, draft_id, revision_id, payload, payload_digest)
             VALUES ('{owner}', '{project}', '{draft}', '{revision}', '{payload}'::jsonb,
                     '{digest}');
             INSERT INTO storyos.draft_artifacts
               (owner_user_id, project_id, draft_id, current_revision_id, retention_state)
             VALUES ('{owner}', '{project}', '{draft}', '{revision}', '{retention}');
             COMMIT;",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            draft = input.draft_id,
            revision = input.source_current_draft_revision_id,
            digest = input.source_draft_payload_digest,
        ))
        .await
        .unwrap();
    (scope, input)
}

/// Issues one Draft Discard call whose canonical bytes are the command body of `input`.
pub(super) async fn discard_call(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: u16,
    input: CloseEditorFlowDraftInput,
) -> CommandCall<CloseEditorFlowDraftInput> {
    let body = serde_json::json!({
        "close_editor_flow_draft_input": {
            "draft_id": input.draft_id,
            "draft_kind": "refused_edit",
            "source_current_draft_revision_id": input.source_current_draft_revision_id,
            "source_draft_payload_digest": input.source_draft_payload_digest,
            "expected_closure": "open",
            "close_reason": "abandoned",
            "editor_session_id": input.editor_session_id.as_ref(),
            "writer_generation": input.writer_generation.to_string(),
        }
    });
    let mut call = issued(store, scope, suffix, &CLOSE_EDITOR_FLOW_DRAFT, input).await;
    call.envelope.canonical_command_bytes = body.to_string().into_bytes();
    call
}

/// One applicable Draft Discard in a new Project.
pub(super) async fn close_editor_flow_draft_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<CloseEditorFlowDraftInput> {
    let (scope, input) = refused_edit_draft(store, admin, base, "retained").await;
    discard_call(store, &scope, base + 9, input).await
}

pub(super) async fn expand_refused_edit_draft(
    store: &PostgresProjectReader,
    call: &CommandCall<ExpandRefusedEditDraftToProposalInput>,
) -> Result<ExpandRefusedEditDraftSettlement, ProjectCommandError> {
    store
        .expand_refused_edit_draft(&call.envelope, &call.input)
        .await
}

/// A new Project with a writer Editor Session and one text Block in Chapter B. One open Refused
/// Edit Draft in `retention` replaces a slice of that Block.
///
/// Returns the Scope and the input that expands the Draft. The fixture skips the creation
/// Receipt and lifecycle event of the Draft and the Block history of the Chapter.
pub(super) async fn refused_edit_expansion(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
    retention: &str,
) -> (ProjectScope, ExpandRefusedEditDraftToProposalInput) {
    let (scope, _chapter_a, chapter_b, revision_b, editor_session_id) =
        two_chapter_writer(store, base).await;
    let block_id = Uuid::now_v7().to_string();
    let block_text = "Guard the narrator voice in this passage.";
    let manuscript = serde_json::json!({
        "format": "storyos.manuscript-payload.v1",
        "schema_version": 1,
        "coordinate_version": 1,
        "blocks": [{"manuscript_block_id": block_id, "block_kind": "paragraph", "text": block_text}],
    });
    let payload = serde_json::json!({
        "schema_revision": "storyos.refused-edit-payload.v1",
        "chapter_id": chapter_b,
        "expected_authoritative_revision_id": revision_b,
        "expected_proposal_head_revision_ids": [],
        "target_refs": [block_id],
        "author_edit_units": [{
            "normalized_primitives": [{
                "kind": "replace_structured_selection",
                "replacement": [{"block_kind": "paragraph", "text": "A steadier voice."}],
            }],
            "selection_snapshot": {
                "coordinate_profile": PROSEMIRROR_TOKEN_UTF16_V1,
                "from": 10,
                "to": 24,
            },
        }],
        "undo_group_id": Uuid::now_v7().to_string(),
        "completed_intent_record_id": Uuid::now_v7().to_string(),
        "local_intent_sequence": "1",
    });
    let input = ExpandRefusedEditDraftToProposalInput {
        editor_session_id: EditorSessionId::new(editor_session_id),
        writer_generation: 1,
        draft_id: Uuid::now_v7().to_string(),
        source_current_draft_revision_id: Uuid::now_v7().to_string(),
        source_draft_payload_digest: storyos_core::hex_sha256(
            storyos_core::canonical_json(&payload).as_bytes(),
        ),
        source_reopen_event_id: None,
        chapter_id: chapter_b.clone(),
        target_ref: block_id.clone(),
        expected_target_revision_id: revision_b.clone(),
        anchor: OpenInlineProposalAnchor {
            manuscript_block_id: block_id.clone(),
            base_authoritative_revision_id: revision_b.clone(),
            manuscript_schema_version: 1,
            coordinate_profile: PROSEMIRROR_TOKEN_UTF16_V1.to_owned(),
            from: 10,
            to: 24,
            boundary_profile: EXCLUSIVE_AUTHORITATIVE_EDGES_V1.to_owned(),
            base_slice_digest: proposal_anchor_base_slice_digest(
                &block_id,
                "paragraph",
                /*manuscript_schema_version*/ 1,
                PROSEMIRROR_TOKEN_UTF16_V1,
                /*from*/ 10,
                /*to*/ 24,
                &block_text[10..24],
            ),
        },
    };
    run_without_foreign_keys(
        admin,
        &format!(
            "INSERT INTO storyos.manuscript_blocks
               (owner_user_id, project_id, manuscript_block_id, manuscript_object_id, block_kind)
             VALUES ('{owner}', '{project}', '{block}', '{chapter}', 'paragraph');
             DELETE FROM storyos.manuscript_revision_members
              WHERE owner_user_id = '{owner}' AND project_id = '{project}'
                AND manuscript_object_id = '{chapter}' AND revision_id = '{head}';
             INSERT INTO storyos.manuscript_revision_members
               (owner_user_id, project_id, manuscript_object_id, revision_id,
                manuscript_block_id, block_order)
             VALUES ('{owner}', '{project}', '{chapter}', '{head}', '{block}', 1);
             UPDATE storyos.authoritative_payloads AS payload
                SET canonical_bytes = convert_to('{manuscript}', 'UTF8')
               FROM storyos.authoritative_revisions AS revision
              WHERE (revision.owner_user_id, revision.project_id, revision.manuscript_object_id,
                     revision.revision_id) = ('{owner}', '{project}', '{chapter}', '{head}')
                AND (payload.owner_user_id, payload.project_id, payload.payload_id) =
                    (revision.owner_user_id, revision.project_id, revision.payload_id);
             INSERT INTO storyos.draft_artifact_revisions
               (owner_user_id, project_id, draft_id, revision_id, payload, payload_digest)
             VALUES ('{owner}', '{project}', '{draft}', '{revision}', '{payload}'::jsonb,
                     '{digest}');
             INSERT INTO storyos.draft_artifacts
               (owner_user_id, project_id, draft_id, current_revision_id, retention_state)
             VALUES ('{owner}', '{project}', '{draft}', '{revision}', '{retention}')",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            block = block_id,
            chapter = chapter_b,
            head = revision_b,
            draft = input.draft_id,
            revision = input.source_current_draft_revision_id,
            digest = input.source_draft_payload_digest,
        ),
    )
    .await;
    (scope, input)
}

/// Issues one Draft expansion call whose canonical bytes carry the source Draft binding of `input`.
pub(super) async fn expansion_call(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: u16,
    input: ExpandRefusedEditDraftToProposalInput,
) -> CommandCall<ExpandRefusedEditDraftToProposalInput> {
    let body = serde_json::json!({
        "expand_refused_edit_draft_to_proposal_input": {
            "draft_id": input.draft_id,
            "source_current_draft_revision_id": input.source_current_draft_revision_id,
            "source_draft_payload_digest": input.source_draft_payload_digest,
            "chapter_id": input.chapter_id,
            "target_refs": [input.target_ref],
            "expected_target_revisions": [input.expected_target_revision_id],
            "editor_session_id": input.editor_session_id.as_ref(),
            "writer_generation": input.writer_generation.to_string(),
        }
    });
    let mut call = issued(store, scope, suffix, &EXPAND_REFUSED_EDIT_DRAFT, input).await;
    call.envelope.canonical_command_bytes = body.to_string().into_bytes();
    call
}

/// One applicable Draft expansion in a new Project.
pub(super) async fn expand_refused_edit_draft_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<ExpandRefusedEditDraftToProposalInput> {
    let (scope, input) = refused_edit_expansion(store, admin, base, "retained").await;
    expansion_call(store, &scope, base + 9, input).await
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_draft_discard_replays_without_a_response_record_and_a_damaged_close_event_is_a_fault() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let call = close_editor_flow_draft_call(&store, &admin, /*base*/ 0x7d40).await;
    let first = close_editor_flow_draft(&store, &call).await.unwrap();
    let key = &call.envelope.challenge_binding.idempotency_key;
    admin
        .batch_execute(&format!(
            "UPDATE storyos.command_idempotency
                SET acknowledgement_format = NULL, response_project = NULL
              WHERE idempotency_key = '{key}'"
        ))
        .await
        .unwrap();
    let pre_capture = close_editor_flow_draft(&store, &with_new_request_ids(&call)).await;
    admin
        .batch_execute(&format!(
            "BEGIN;
             SET LOCAL session_replication_role = replica;
             UPDATE storyos.draft_close_events
                SET author_action_sequence = author_action_sequence + 1000
              WHERE receipt_id = '{}';
             COMMIT;",
            first.ids.receipt_id
        ))
        .await
        .unwrap();
    let damaged = close_editor_flow_draft(&store, &with_new_request_ids(&call)).await;
    admin
        .batch_execute(&format!(
            "UPDATE storyos.command_idempotency
                SET canonical_command_digest = 'sha256:closeEditorFlowDraft:damaged'
              WHERE idempotency_key = '{key}'"
        ))
        .await
        .unwrap();
    let other_digest = close_editor_flow_draft(&store, &with_new_request_ids(&call)).await;
    assert_eq!(pre_capture.unwrap(), first);
    assert!(matches!(damaged, Err(ProjectCommandError::Unavailable(_))));
    assert!(matches!(
        other_digest,
        Err(ProjectCommandError::BindingConflict)
    ));
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_draft_discard_refuses_before_admission_and_requires_the_client_writer_generation() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, open) = refused_edit_draft(&store, &admin, /*base*/ 0x7d50, "retained").await;
    let stale_writer = CloseEditorFlowDraftInput {
        writer_generation: 2,
        ..open.clone()
    };
    let missing = CloseEditorFlowDraftInput {
        draft_id: Uuid::now_v7().to_string(),
        ..open.clone()
    };
    let (damaged_scope, damaged) =
        refused_edit_draft(&store, &admin, /*base*/ 0x7d60, "retained").await;
    admin
        .batch_execute(&format!(
            "BEGIN;
             SET LOCAL session_replication_role = replica;
             UPDATE storyos.draft_artifact_revisions SET payload_digest = repeat('0', 64)
              WHERE draft_id = '{}';
             COMMIT;",
            damaged.draft_id
        ))
        .await
        .unwrap();
    let mut observed = Vec::new();
    for (scope, suffix, input) in [
        (&scope, 0x7d59, stale_writer),
        (&scope, 0x7d5a, missing),
        (&damaged_scope, 0x7d69, damaged),
    ] {
        let call = discard_call(&store, scope, suffix, input).await;
        let refused = close_editor_flow_draft(&store, &call).await;
        observed.push((
            match refused {
                Err(error) => ReplayError::from(error),
                Ok(settled) => panic!("the Discard must refuse before Admission, got {settled:?}"),
            },
            settlement_rows(&admin, &call.envelope.ids.receipt_id).await,
        ));
    }
    let open_call = discard_call(&store, &scope, /*suffix*/ 0x7d5b, open).await;
    let settled = close_editor_flow_draft(&store, &open_call).await.unwrap();
    assert_eq!(
        (observed, settled.outcome.receipt_result()),
        (
            vec![
                (ReplayError::WriterIneligible, [0; 5]),
                (ReplayError::MissingProject, [0; 5]),
                (ReplayError::BindingConflict, [0; 5]),
            ],
            ReceiptResult::AuthoritativeApplied,
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_draft_expansion_replays_from_its_proposal_revision_and_damaged_effect_rows_are_faults() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let call = expand_refused_edit_draft_call(&store, &admin, /*base*/ 0x8e40).await;
    let first = expand_refused_edit_draft(&store, &call).await.unwrap();
    let key = &call.envelope.challenge_binding.idempotency_key;
    let receipt_id = &first.ids.receipt_id;
    admin
        .batch_execute(&format!(
            "UPDATE storyos.command_idempotency
                SET acknowledgement_format = NULL, response_project = NULL
              WHERE idempotency_key = '{key}'"
        ))
        .await
        .unwrap();
    let pre_capture = expand_refused_edit_draft(&store, &with_new_request_ids(&call)).await;
    let mut damaged = Vec::new();
    for statement in [
        format!(
            "UPDATE storyos.draft_close_events
                SET author_action_sequence = author_action_sequence + 1000
              WHERE receipt_id = '{receipt_id}'"
        ),
        format!(
            "DELETE FROM storyos.proposal_revisions AS revision
               USING storyos.domain_receipts AS receipt
              WHERE receipt.receipt_id = '{receipt_id}'
                AND revision.revision_id = receipt.proposal_revision_ids[1]"
        ),
        format!("DELETE FROM storyos.author_action_entries WHERE receipt_id = '{receipt_id}'"),
        format!("DELETE FROM storyos.draft_close_events WHERE receipt_id = '{receipt_id}'"),
    ] {
        run_without_foreign_keys(&admin, &statement).await;
        let replayed = expand_refused_edit_draft(&store, &with_new_request_ids(&call)).await;
        damaged.push(match replayed {
            Err(error) => ReplayError::from(error),
            Ok(settled) => panic!("a damaged expansion must not replay, got {settled:?}"),
        });
    }
    admin
        .batch_execute(&format!(
            "UPDATE storyos.command_idempotency
                SET canonical_command_digest = 'sha256:expandRefusedEditDraftToProposal:damaged'
              WHERE idempotency_key = '{key}'"
        ))
        .await
        .unwrap();
    let other_digest = expand_refused_edit_draft(&store, &with_new_request_ids(&call)).await;
    assert_eq!(pre_capture.unwrap(), first);
    assert_eq!(damaged, vec![ReplayError::Unavailable; 4]);
    assert!(matches!(
        other_digest,
        Err(ProjectCommandError::BindingConflict)
    ));
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_draft_expansion_refuses_before_admission_and_requires_the_client_writer_generation() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, open) = refused_edit_expansion(&store, &admin, /*base*/ 0x8e50, "retained").await;
    let stale_writer = ExpandRefusedEditDraftToProposalInput {
        writer_generation: 2,
        ..open.clone()
    };
    let missing = ExpandRefusedEditDraftToProposalInput {
        draft_id: Uuid::now_v7().to_string(),
        ..open.clone()
    };
    let (damaged_scope, damaged) =
        refused_edit_expansion(&store, &admin, /*base*/ 0x8e60, "retained").await;
    run_without_foreign_keys(
        &admin,
        &format!(
            "UPDATE storyos.draft_artifact_revisions
                SET payload = jsonb_set(payload, '{{local_intent_sequence}}', '\"2\"')
              WHERE draft_id = '{}'",
            damaged.draft_id
        ),
    )
    .await;
    let mut observed = Vec::new();
    for (scope, suffix, input) in [
        (&scope, 0x8e59, stale_writer),
        (&scope, 0x8e5a, missing),
        (&damaged_scope, 0x8e69, damaged),
    ] {
        let call = expansion_call(&store, scope, suffix, input).await;
        let refused = expand_refused_edit_draft(&store, &call).await;
        observed.push((
            match refused {
                Err(error) => ReplayError::from(error),
                Ok(settled) => {
                    panic!("the expansion must refuse before Admission, got {settled:?}")
                }
            },
            settlement_rows(&admin, &call.envelope.ids.receipt_id).await,
        ));
    }
    let open_call = expansion_call(&store, &scope, /*suffix*/ 0x8e5b, open).await;
    let settled = expand_refused_edit_draft(&store, &open_call).await.unwrap();
    assert_eq!(
        (observed, settled.outcome.receipt_result()),
        (
            vec![
                (ReplayError::WriterIneligible, [0; 5]),
                (ReplayError::MissingProject, [0; 5]),
                (ReplayError::BindingConflict, [0; 5]),
            ],
            ReceiptResult::AuthoritativeApplied,
        )
    );
}

fn changed_admission_payload(receipt_id: &str) -> String {
    format!(
        "UPDATE storyos.author_command_admissions AS admission
            SET command_payload = admission.command_payload || '{{\"damaged\": true}}'::jsonb
           FROM storyos.domain_receipts AS receipt
          WHERE receipt.receipt_id = '{receipt_id}'
            AND admission.author_command_admission_id = receipt.author_command_admission_id"
    )
}

fn changed_draft_reference(receipt_id: &str) -> String {
    format!(
        "UPDATE storyos.domain_receipts SET draft_artifact_refs = ARRAY['{}']
          WHERE receipt_id = '{receipt_id}'",
        Uuid::now_v7()
    )
}

pub(super) fn compensation_action(receipt_id: &str) -> String {
    format!(
        "UPDATE storyos.author_action_entries
            SET disposition = 'compensation', compensated_source_sequence = author_action_sequence
          WHERE receipt_id = '{receipt_id}'"
    )
}

fn zero_authority_action(receipt_id: &str) -> String {
    format!(
        "INSERT INTO storyos.author_action_entries
           (owner_user_id, project_id, author_action_sequence, disposition, receipt_id,
            receipt_result_kind)
         SELECT owner_user_id, project_id, 1000, 'forward', receipt_id, 'draft_closure_changed'
           FROM storyos.domain_receipts WHERE receipt_id = '{receipt_id}'"
    )
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_draft_replay_binds_its_admission_and_draft_and_requires_its_author_action_shape() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    for (base, damage) in [
        (0x9a00, changed_admission_payload as fn(&str) -> String),
        (0x9a10, changed_draft_reference),
        (0x9a20, compensation_action),
    ] {
        observed.push(
            damaged_replay(
                &store,
                &admin,
                &close_editor_flow_draft_call(&store, &admin, base).await,
                damage,
            )
            .await,
        );
        observed.push(
            damaged_replay(
                &store,
                &admin,
                &expand_refused_edit_draft_call(&store, &admin, base + 0x100).await,
                damage,
            )
            .await,
        );
    }
    let (scope, tombstoned) =
        refused_edit_draft(&store, &admin, /*base*/ 0x9a30, "tombstoned").await;
    let refused_discard = discard_call(&store, &scope, /*suffix*/ 0x9a39, tombstoned).await;
    observed.push(damaged_replay(&store, &admin, &refused_discard, zero_authority_action).await);
    let (scope, tombstoned) =
        refused_edit_expansion(&store, &admin, /*base*/ 0x9b30, "tombstoned").await;
    let refused_expansion = expansion_call(&store, &scope, /*suffix*/ 0x9b39, tombstoned).await;
    observed.push(damaged_replay(&store, &admin, &refused_expansion, zero_authority_action).await);
    assert_eq!(
        observed,
        vec![
            ReplayError::BindingConflict,
            ReplayError::BindingConflict,
            ReplayError::BindingConflict,
            ReplayError::BindingConflict,
            ReplayError::Unavailable,
            ReplayError::Unavailable,
            ReplayError::Unavailable,
            ReplayError::Unavailable,
        ]
    );
}

/// Sets one Receipt payload field of `call` to the JSON text `value`, replays the call, and
/// returns the replay error.
async fn replay_with_receipt_field<C: ProjectCommand<Error: SequenceError> + Clone>(
    store: &PostgresProjectReader,
    admin: &Client,
    call: &CommandCall<C>,
    field: &str,
    value: &str,
) -> ReplayError {
    replay_with_receipt_payload(
        store,
        admin,
        call,
        &format!("jsonb_set(result_payload, '{{{field}}}', '{value}'::jsonb)"),
    )
    .await
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_draft_replay_refuses_a_receipt_field_of_the_wrong_type_or_shape() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    for (base, field, value) in [
        (0x9e00, "draft_revision_id", "123"),
        (0x9e10, "draft_revision_id", "\"not a revision\""),
        (0x9e20, "payload_digest", "{}"),
        (0x9e30, "payload_digest", "\"damaged\""),
        (0x9e40, "observed_closure", "\"damaged\""),
        (0x9e50, "event_id", "7"),
    ] {
        observed.push(
            replay_with_receipt_field(
                &store,
                &admin,
                &close_editor_flow_draft_call(&store, &admin, base).await,
                field,
                value,
            )
            .await,
        );
        observed.push(
            replay_with_receipt_field(
                &store,
                &admin,
                &expand_refused_edit_draft_call(&store, &admin, base + 0x100).await,
                field,
                value,
            )
            .await,
        );
    }
    for (base, changed) in [
        (0xa000, "result_payload - 'current_target_revision_id'"),
        (
            0xa010,
            "jsonb_set(result_payload, '{current_target_revision_id}', 'null'::jsonb)",
        ),
        (
            0xa020,
            "jsonb_set(result_payload, '{current_target_revision_id}', '9'::jsonb)",
        ),
    ] {
        observed.push(
            replay_with_receipt_payload(
                &store,
                &admin,
                &expand_refused_edit_draft_call(&store, &admin, base).await,
                changed,
            )
            .await,
        );
    }
    assert_eq!(observed, vec![ReplayError::Unavailable; 15]);
}
