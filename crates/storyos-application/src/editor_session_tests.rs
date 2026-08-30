use std::sync::Mutex;

use super::*;
use crate::{ManuscriptBlock, ManuscriptBlockKind, ProjectId, UserId};

struct Store(Mutex<Vec<OpenEditorSession>>);

impl EditorSessionStore for Store {
    async fn create_editor_session(
        &self,
        request: &OpenEditorSession,
    ) -> Result<EditorSession, EditorSessionError> {
        self.0.lock().unwrap().push(request.clone());
        Ok(session(&request.editor_session_id, &request.client_binding))
    }

    async fn read_editor_session(
        &self,
        request: &EditorSessionLookup,
    ) -> Result<Option<EditorSession>, EditorSessionError> {
        Ok(Some(session(
            &request.editor_session_id,
            &request.client_binding,
        )))
    }
}

fn session(id: &EditorSessionId, binding: &EditorClientBinding) -> EditorSession {
    EditorSession {
        editor_session_id: id.clone(),
        client_binding: binding.clone(),
        opened_at: "2026-08-13T08:00:00.000Z".to_owned(),
        writer: EditorWriterState::CurrentWriter {
            writer_generation: 1,
        },
        base_snapshot: EditorSessionSnapshot {
            snapshot_id: "snapshot".to_owned(),
            chapter_id: "chapter".to_owned(),
            authoritative_revision_id: "revision".to_owned(),
            project_activity_position: 0,
            body: "Body".to_owned(),
            blocks: vec![ManuscriptBlock {
                manuscript_block_id: "block".to_owned(),
                block_kind: ManuscriptBlockKind::Paragraph,
                text: "Body".to_owned(),
            }],
            payload_digest_hex: "a".repeat(64),
            created_at: "2026-08-13T08:00:00.000Z".to_owned(),
        },
        author_undo_frontier_sequence: None,
    }
}

#[tokio::test]
async fn use_cases_preserve_the_exact_session_binding() {
    let scope = ProjectScope::new(UserId::new("user"), ProjectId::new("project"));
    let binding = EditorClientBinding {
        binding_ref: "binding".to_owned(),
        session_generation: 7,
        client_contract_revision: "client".to_owned(),
        security_policy_revision: "security".to_owned(),
    };
    let request = OpenEditorSession {
        project_scope: scope.clone(),
        editor_session_id: EditorSessionId::new("session"),
        snapshot_id: "snapshot".to_owned(),
        client_binding: binding.clone(),
        challenge_binding: crate::ProjectCommandChallengeBinding {
            project_scope: scope.clone(),
            client_session_binding_digest: "binding".to_owned(),
            client_session_generation: 7,
            client_contract_revision: "client".to_owned(),
            security_policy_revision: "security".to_owned(),
            limit_profile_revision: "limit".to_owned(),
            challenge_rate_policy_revision: "rate".to_owned(),
            method: "POST".to_owned(),
            route_template: "/editor-sessions".to_owned(),
            command_schema: "schema".to_owned(),
            command_kind: "createEditorSession".to_owned(),
            canonical_command_digest: "digest".to_owned(),
            idempotency_key: "key".to_owned(),
        },
        nonce_digest: "nonce".to_owned(),
    };
    let store = Store(Mutex::new(Vec::new()));

    let created = create_editor_session(&store, &request).await.unwrap();
    let read = get_editor_session(
        &store,
        &EditorSessionLookup {
            project_scope: scope,
            editor_session_id: request.editor_session_id.clone(),
            client_binding: binding,
        },
    )
    .await
    .unwrap();

    assert_eq!(read, Some(created));
    assert_eq!(store.0.into_inner().unwrap(), vec![request]);
}

#[tokio::test]
async fn create_refuses_mismatched_challenge_and_session_bindings() {
    let scope = ProjectScope::new(UserId::new("user"), ProjectId::new("project"));
    let binding = EditorClientBinding {
        binding_ref: "session-binding".to_owned(),
        session_generation: 1,
        client_contract_revision: "client".to_owned(),
        security_policy_revision: "security".to_owned(),
    };
    let request = OpenEditorSession {
        project_scope: scope.clone(),
        editor_session_id: EditorSessionId::new("session"),
        snapshot_id: "snapshot".to_owned(),
        client_binding: binding,
        challenge_binding: crate::ProjectCommandChallengeBinding {
            project_scope: scope,
            client_session_binding_digest: "different-binding".to_owned(),
            client_session_generation: 1,
            client_contract_revision: "client".to_owned(),
            security_policy_revision: "security".to_owned(),
            limit_profile_revision: "limit".to_owned(),
            challenge_rate_policy_revision: "rate".to_owned(),
            method: "POST".to_owned(),
            route_template: "/editor-sessions".to_owned(),
            command_schema: "schema".to_owned(),
            command_kind: "createEditorSession".to_owned(),
            canonical_command_digest: "digest".to_owned(),
            idempotency_key: "key".to_owned(),
        },
        nonce_digest: "nonce".to_owned(),
    };
    let store = Store(Mutex::new(Vec::new()));

    assert!(matches!(
        create_editor_session(&store, &request).await,
        Err(EditorSessionError::BindingConflict)
    ));
    assert!(store.0.into_inner().unwrap().is_empty());
}
