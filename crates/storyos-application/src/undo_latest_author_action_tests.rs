use std::sync::Mutex;

use super::*;
use crate::{ProjectId, UserId};

struct Store(Mutex<usize>);

impl UndoLatestAuthorActionStore for Store {
    async fn undo_latest_author_action(
        &self,
        command: &UndoLatestAuthorActionCommand,
    ) -> Result<UndoLatestAuthorActionSettlement, UndoLatestAuthorActionError> {
        *self.0.lock().unwrap() += 1;
        Ok(UndoLatestAuthorActionSettlement {
            ids: command.ids.clone(),
            effect: UndoLatestAuthorActionSettlementEffect::Unavailable {
                reason: storyos_core::UndoLatestAuthorActionUnavailable::NoFrontier,
            },
            receipt_created_at: "2026-08-30T00:00:00.000Z".to_owned(),
            project_activity_position: 0,
        })
    }
}

fn command() -> UndoLatestAuthorActionCommand {
    let project_scope = ProjectScope::new(UserId::new("user"), ProjectId::new("project"));
    let client_binding = EditorClientBinding {
        binding_ref: "binding".to_owned(),
        session_generation: 1,
        client_contract_revision: "client".to_owned(),
        security_policy_revision: "security".to_owned(),
    };
    UndoLatestAuthorActionCommand {
        project_scope: project_scope.clone(),
        client_binding: client_binding.clone(),
        challenge_binding: ProjectCommandChallengeBinding {
            project_scope,
            client_session_binding_digest: client_binding.binding_ref,
            client_session_generation: 1,
            client_contract_revision: "client".to_owned(),
            security_policy_revision: "security".to_owned(),
            limit_profile_revision: "limit".to_owned(),
            challenge_rate_policy_revision: "rate".to_owned(),
            method: "POST".to_owned(),
            route_template: "/api/v1/projects/{project_id}/author-actions/undo".to_owned(),
            command_schema: "storyos.command.undo-latest-author-action.request.v1".to_owned(),
            command_kind: "undoLatestAuthorAction".to_owned(),
            canonical_command_digest: "sha256:storyos.command.undoLatestAuthorAction.jcs.v1:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_owned(),
            idempotency_key: "key".to_owned(),
        },
        nonce_digest: "nonce".to_owned(),
        canonical_command_bytes: Vec::new(),
        correlation_id: "correlation".to_owned(),
        ids: AuthorCommandAdmissionIds {
            command_id: "command".to_owned(),
            author_command_admission_id: "admission".to_owned(),
            receipt_id: "receipt".to_owned(),
        },
        editor_session_id: EditorSessionId::new("session"),
        expected_author_undo_frontier_sequence: 1,
        expected_authoritative_revision_id: "revision".to_owned(),
    }
}

#[tokio::test]
async fn a_matching_challenge_reaches_the_store() {
    let store = Store(Mutex::new(0));
    let settlement = undo_latest_author_action(&store, &command()).await.unwrap();
    assert_eq!(*store.0.lock().unwrap(), 1);
    assert!(matches!(
        settlement.effect,
        UndoLatestAuthorActionSettlementEffect::Unavailable { .. }
    ));
}

#[tokio::test]
async fn a_mismatched_command_kind_is_a_binding_conflict() {
    let store = Store(Mutex::new(0));
    let mut mismatched = command();
    mismatched.challenge_binding.command_kind = "setCurrentChapter".to_owned();
    assert!(matches!(
        undo_latest_author_action(&store, &mismatched).await,
        Err(UndoLatestAuthorActionError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}
