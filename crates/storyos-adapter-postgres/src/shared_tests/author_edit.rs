//! The shared test support from `author_edit_tests.rs`.

use crate::author_edit::*;
use storyos_application::{
    ApplyAuthorEditCommand, AuthorCommandAdmissionIds, EditorClientBinding, EditorSessionId,
    ProjectCommandChallengeBinding, ProjectScope,
};
use storyos_contracts::CommandKind;
use storyos_core::{AuthorEditPrimitive, AuthorEditUnit, SelectionSnapshot};

pub(crate) const CHAPTER: &str = "018f0000-0000-7001-8000-000000000003";

pub(crate) const REVISION: &str = "018f0000-0000-7001-8000-000000000005";

pub(crate) static AUTHOR_EDIT_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub(crate) fn binding(
    scope: &ProjectScope,
    kind: &str,
    path: &str,
    schema: &str,
    key: &str,
) -> ProjectCommandChallengeBinding {
    ProjectCommandChallengeBinding {
        project_scope: scope.clone(),
        client_session_binding_digest: "binding:author-edit".to_owned(),
        client_session_generation: 1,
        client_contract_revision: "storyos.web-client.release-1.v1".to_owned(),
        security_policy_revision: "storyos.web-security-policy.release-1.v1".to_owned(),
        limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
        challenge_rate_policy_revision: "storyos.project-command-challenge-rate.fixed-window.v1"
            .to_owned(),
        method: "POST".to_owned(),
        route_template: path.to_owned(),
        command_schema: schema.to_owned(),
        command_kind: kind.to_owned(),
        canonical_command_digest: format!("sha256:storyos.test:{key}"),
        idempotency_key: key.to_owned(),
    }
}

pub(crate) fn author_command(
    scope: &ProjectScope,
    editor_session_id: &str,
    key: &str,
    nonce_digest: &str,
    suffix: &str,
) -> ApplyAuthorEditCommand {
    let challenge_binding = binding(
        scope,
        CommandKind::ApplyAuthorEdit.as_str(),
        "/api/v1/projects/{project_id}/manuscript/author-edits",
        "storyos.command.apply-author-edit.request.v2",
        key,
    );
    let mut command = ApplyAuthorEditCommand {
        project_scope: scope.clone(),
        client_binding: EditorClientBinding {
            binding_ref: "binding:author-edit".to_owned(),
            session_generation: 1,
            client_contract_revision: "storyos.web-client.release-1.v1".to_owned(),
            security_policy_revision: "storyos.web-security-policy.release-1.v1".to_owned(),
        },
        challenge_binding,
        nonce_digest: nonce_digest.to_owned(),
        canonical_command_bytes: Vec::new(),
        correlation_id: format!("018f0000-0000-7001-8000-000000000{suffix}0"),
        ids: AuthorCommandAdmissionIds {
            command_id: format!("018f0000-0000-7001-8000-000000000{suffix}1"),
            author_command_admission_id: format!("018f0000-0000-7001-8000-000000000{suffix}2"),
            receipt_id: format!("018f0000-0000-7001-8000-000000000{suffix}3"),
        },
        editor_session_id: EditorSessionId::new(editor_session_id),
        chapter_id: CHAPTER.to_owned(),
        expected_authoritative_revision_id: REVISION.to_owned(),
        expected_proposal_head_revision_ids: Vec::new(),
        proposal_target: None,
        target_refs: vec![format!("manuscript:{CHAPTER}")],
        observed_ownership_partition: "authoritative".to_owned(),
        editor_contract_revision: "storyos.editor-contract.release-1.v3".to_owned(),
        undo_group_id: format!("018f0000-0000-7001-8000-000000000{suffix}8"),
        completed_intent_record_id: format!("018f0000-0000-7001-8000-000000000{suffix}9"),
        local_intent_sequence: suffix.parse().unwrap(),
        author_edit_units: vec![
            AuthorEditUnit {
                normalized_primitives: vec![AuthorEditPrimitive::ReplaceSelection {
                    from: 15,
                    to: 15,
                    text: "x".to_owned(),
                }],
                selection_snapshot: SelectionSnapshot {
                    coordinate_profile: storyos_core::UTF16_COORDINATE_PROFILE.to_owned(),
                    from: 15,
                    to: 15,
                },
            },
            AuthorEditUnit {
                normalized_primitives: vec![AuthorEditPrimitive::ReplaceSelection {
                    from: 15,
                    to: 16,
                    text: "!".to_owned(),
                }],
                selection_snapshot: SelectionSnapshot {
                    coordinate_profile: storyos_core::UTF16_COORDINATE_PROFILE.to_owned(),
                    from: 15,
                    to: 16,
                },
            },
        ],
    };
    bind_canonical_payload(&mut command);
    command
}

pub(crate) fn bind_canonical_payload(command: &mut ApplyAuthorEditCommand) {
    let author_edit_units = command
        .author_edit_units
        .iter()
        .map(|unit| {
            let [storyos_core::AuthorEditPrimitive::ReplaceSelection { from, to, text }] =
                unit.normalized_primitives.as_slice()
            else {
                panic!("test Author Edit unit must contain one ReplaceSelection")
            };
            serde_json::json!({
                "normalized_primitives": [{"kind": "replace_selection", "from": from,
                    "to": to, "text": text}],
                "selection_snapshot": {
                    "coordinate_profile": unit.selection_snapshot.coordinate_profile,
                    "from": unit.selection_snapshot.from,
                    "to": unit.selection_snapshot.to,
                }
            })
        })
        .collect::<Vec<_>>();
    let payload = serde_json::json!({
        "command_schema": command.challenge_binding.command_schema,
        "client_contract_revision": command.client_binding.client_contract_revision,
        "security_policy_revision": command.client_binding.security_policy_revision,
        "correlation_id": command.correlation_id,
        "editor_session_id": command.editor_session_id.as_ref(),
        "chapter_id": command.chapter_id,
        "expected_authoritative_revision_id": command.expected_authoritative_revision_id,
        "expected_proposal_head_revision_ids": command.expected_proposal_head_revision_ids,
        "target_refs": command.target_refs,
        "observed_ownership_partition": command.observed_ownership_partition,
        "editor_contract_revision": command.editor_contract_revision,
        "undo_group_id": command.undo_group_id,
        "completed_intent_record_id": command.completed_intent_record_id,
        "local_intent_sequence": command.local_intent_sequence.to_string(),
        "author_edit_units": author_edit_units,
    });
    command.canonical_command_bytes = serde_json::to_vec(&payload).unwrap();
    command.challenge_binding.canonical_command_digest = format!(
        "sha256:storyos.command.applyAuthorEdit.jcs.v1:{}",
        sha256_hex(&command.canonical_command_bytes)
    );
}
