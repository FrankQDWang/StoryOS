use std::sync::Mutex;

use super::*;
use crate::{ProjectId, UserId};

struct Store(Mutex<usize>);

impl SetCurrentChapterStore for Store {
    async fn set_current_chapter(
        &self,
        command: &SetCurrentChapterCommand,
    ) -> Result<SetCurrentChapterSettlement, SetCurrentChapterError> {
        *self.0.lock().unwrap() += 1;
        Ok(SetCurrentChapterSettlement {
            ids: command.ids.clone(),
            effect: SetCurrentChapterSettlementEffect::Applied {
                current_chapter_id: command.chapter_id.clone(),
                base_snapshot_id: "snapshot".to_owned(),
            },
            receipt_created_at: "2026-08-29T00:00:00.000Z".to_owned(),
            project_activity_position: 2,
            project_activity_event_id: "event".to_owned(),
        })
    }
}

fn command() -> SetCurrentChapterCommand {
    let project_scope = ProjectScope::new(UserId::new("user"), ProjectId::new("project"));
    let client_binding = EditorClientBinding {
        binding_ref: "binding".to_owned(),
        session_generation: 1,
        client_contract_revision: "client".to_owned(),
        security_policy_revision: "security".to_owned(),
    };
    SetCurrentChapterCommand {
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
            method: "PUT".to_owned(),
            route_template: "/api/v1/projects/{project_id}/current-chapter".to_owned(),
            command_schema: "storyos.command.set-current-chapter.request.v1".to_owned(),
            command_kind: "setCurrentChapter".to_owned(),
            canonical_command_digest: "sha256:storyos.command.setCurrentChapter.jcs.v1:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_owned(),
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
        chapter_id: "chapter-b".to_owned(),
        expected_current_chapter_id: "chapter-a".to_owned(),
        expected_target_revision_id: "revision".to_owned(),
    }
}

#[tokio::test]
async fn a_matching_challenge_reaches_the_store() {
    let store = Store(Mutex::new(0));
    let settlement = set_current_chapter(&store, &command()).await.unwrap();
    assert_eq!(*store.0.lock().unwrap(), 1);
    assert!(matches!(
        settlement.effect,
        SetCurrentChapterSettlementEffect::Applied { .. }
    ));
}

#[tokio::test]
async fn a_mismatched_command_kind_is_a_binding_conflict() {
    let store = Store(Mutex::new(0));
    let mut mismatched = command();
    mismatched.challenge_binding.command_kind = "createVolume".to_owned();
    let error = set_current_chapter(&store, &mismatched).await.unwrap_err();
    assert!(matches!(error, SetCurrentChapterError::BindingConflict));
    assert_eq!(*store.0.lock().unwrap(), 0);
}
