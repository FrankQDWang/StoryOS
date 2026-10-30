//! The Author Edit command builders of the adapter tests. `create_structure_tests.rs` holds the
//! Create Volume tests. The builders keep this module path, which other test files import.

use storyos_application::{
    ApplyAuthorEditCommand, AuthorCommandAdmissionIds, EditorClientBinding, EditorSessionId,
    IssueProjectCommandChallenge, ProjectCommandChallengeBinding, ProjectScope,
    issue_project_command_challenge,
};
use storyos_core::{AuthorEditPrimitive, AuthorEditUnit, SelectionSnapshot};

use crate::PostgresProjectReader;

const CLIENT: &str = "storyos.web-client.release-1.v3";
const SECURITY: &str = "storyos.web-security-policy.release-1.v1";

pub(crate) struct NamedEdit<'a> {
    pub(crate) editor_session_id: &'a str,
    pub(crate) chapter_id: &'a str,
    pub(crate) expected_revision_id: &'a str,
    pub(crate) suffix: &'a str,
    pub(crate) local_intent_sequence: u64,
    pub(crate) text: &'a str,
    /// The Proposal Operation whose candidate the edit changes, or `None` for a prose edit.
    pub(crate) proposal_target: Option<storyos_application::AuthorEditProposalTarget>,
}

pub(crate) async fn apply_named_edit(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    edit: NamedEdit<'_>,
) -> storyos_application::AuthorEditSettlement {
    let command = named_edit_command(store, scope, edit).await;
    store.apply_author_edit(&command).await.unwrap()
}

/// The Author Edit command of `edit` with its issued Command Challenge.
pub(crate) async fn named_edit_command(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    edit: NamedEdit<'_>,
) -> ApplyAuthorEditCommand {
    adjusted_edit_command(store, scope, edit, |_command| {}).await
}

/// The Author Edit command of `edit` after `adjust`, with its canonical payload and its issued
/// Command Challenge.
pub(crate) async fn adjusted_edit_command(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    edit: NamedEdit<'_>,
    adjust: impl FnOnce(&mut ApplyAuthorEditCommand),
) -> ApplyAuthorEditCommand {
    let NamedEdit {
        editor_session_id,
        chapter_id,
        expected_revision_id,
        suffix,
        local_intent_sequence,
        text,
        proposal_target,
    } = edit;
    let mut command = ApplyAuthorEditCommand {
        project_scope: scope.clone(),
        client_binding: EditorClientBinding {
            binding_ref: "sha256:session-a".to_owned(),
            session_generation: 1,
            client_contract_revision: CLIENT.to_owned(),
            security_policy_revision: SECURITY.to_owned(),
        },
        challenge_binding: ProjectCommandChallengeBinding {
            project_scope: scope.clone(),
            client_session_binding_digest: "sha256:session-a".to_owned(),
            client_session_generation: 1,
            client_contract_revision: CLIENT.to_owned(),
            security_policy_revision: SECURITY.to_owned(),
            limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
            challenge_rate_policy_revision:
                "storyos.project-command-challenge-rate.fixed-window.v1".to_owned(),
            method: "POST".to_owned(),
            route_template: "/api/v1/projects/{project_id}/manuscript/author-edits".to_owned(),
            command_schema: "storyos.command.apply-author-edit.request.v1".to_owned(),
            command_kind: "applyAuthorEdit".to_owned(),
            canonical_command_digest: String::new(),
            idempotency_key: format!("018f0000-0000-7001-8000-00000000{suffix}"),
        },
        nonce_digest: format!("sha256:nonce-{suffix}"),
        canonical_command_bytes: Vec::new(),
        correlation_id: format!("018f0000-0000-7001-8000-00000000{suffix}"),
        ids: AuthorCommandAdmissionIds {
            command_id: format!("018f0000-0000-7001-8000-00000001{suffix}"),
            author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{suffix}"),
            receipt_id: format!("018f0000-0000-7001-8000-00000003{suffix}"),
        },
        editor_session_id: EditorSessionId::new(editor_session_id),
        writer_generation: 1,
        chapter_id: chapter_id.to_owned(),
        expected_authoritative_revision_id: expected_revision_id.to_owned(),
        expected_proposal_head_revision_ids: proposal_target
            .iter()
            .map(|target| target.revision_id.clone())
            .collect(),
        target_refs: vec![format!("manuscript:{chapter_id}")],
        observed_ownership_partition: if proposal_target.is_some() {
            "mixed"
        } else {
            "authoritative"
        }
        .to_owned(),
        proposal_target,
        editor_contract_revision: "storyos.editor-contract.release-1.v3".to_owned(),
        undo_group_id: format!("018f0000-0000-7001-8000-00000004{suffix}"),
        completed_intent_record_id: format!("018f0000-0000-7001-8000-00000005{suffix}"),
        local_intent_sequence,
        author_edit_units: vec![AuthorEditUnit {
            normalized_primitives: vec![AuthorEditPrimitive::ReplaceSelection {
                from: 0,
                to: 0,
                text: text.to_owned(),
            }],
            selection_snapshot: SelectionSnapshot {
                coordinate_profile: storyos_core::UTF16_COORDINATE_PROFILE.to_owned(),
                from: 0,
                to: 0,
            },
        }],
    };
    adjust(&mut command);
    let payload = serde_json::json!({
        "command_schema": command.challenge_binding.command_schema,
        "client_contract_revision": command.client_binding.client_contract_revision,
        "security_policy_revision": command.client_binding.security_policy_revision,
        "correlation_id": command.correlation_id,
        "editor_session_id": command.editor_session_id.as_ref(),
        "writer_generation": command.writer_generation.to_string(),
        "chapter_id": command.chapter_id,
        "expected_authoritative_revision_id": command.expected_authoritative_revision_id,
        "expected_proposal_head_revision_ids": command.expected_proposal_head_revision_ids,
        "target_refs": command.target_refs,
        "observed_ownership_partition": command.observed_ownership_partition,
        "editor_contract_revision": command.editor_contract_revision,
        "undo_group_id": command.undo_group_id,
        "completed_intent_record_id": command.completed_intent_record_id,
        "local_intent_sequence": local_intent_sequence.to_string(),
        "author_edit_units": serde_json::to_value(&command.author_edit_units).unwrap(),
    });
    let mut payload = payload;
    if let Some(target) = &command.proposal_target {
        payload["proposal_target"] = serde_json::json!({
            "proposal_id": target.proposal_id,
            "operation_id": target.operation_id,
            "revision_id": target.revision_id,
            "manuscript_block_id": target.manuscript_block_id,
        });
    }
    command.canonical_command_bytes = serde_json::to_vec(&payload).unwrap();
    command.challenge_binding.canonical_command_digest = format!(
        "sha256:storyos.command.applyAuthorEdit.jcs.v1:{}",
        crate::author_edit::sha256_hex(&command.canonical_command_bytes)
    );
    issue_project_command_challenge(
        store,
        &IssueProjectCommandChallenge {
            binding: command.challenge_binding.clone(),
            nonce: format!("edit-nonce-{suffix}"),
            nonce_digest: command.nonce_digest.clone(),
            earlier_revision_nonces: Vec::new(),
        },
    )
    .await
    .unwrap();
    command
}
