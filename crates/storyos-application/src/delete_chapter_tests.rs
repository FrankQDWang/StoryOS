use std::sync::Mutex;

use super::*;
use crate::{ChapterId, ProjectId, UserId};

struct Store(Mutex<usize>);

impl DeleteChapterStore for Store {
    async fn delete_chapter(
        &self,
        command: &DeleteChapterCommand,
    ) -> Result<DeleteChapterSettlement, DeleteChapterError> {
        *self.0.lock().unwrap() += 1;
        Ok(DeleteChapterSettlement {
            ids: command.ids.clone(),
            effect: DeleteChapterSettlementEffect::Applied {
                tree_revision: command.expected_chapter_revision + 1,
                volume_id: "volume".to_owned(),
                current: storyos_core::DeleteChapterCurrent::PreserveExisting,
            },
            receipt_created_at: "2026-08-27T00:00:00.000Z".to_owned(),
            project_activity_position: 5,
            project_activity_event_id: "event".to_owned(),
        })
    }
}

fn command() -> DeleteChapterCommand {
    let project_scope = ProjectScope::new(UserId::new("user"), ProjectId::new("project"));
    let client_binding = EditorClientBinding {
        binding_ref: "binding".to_owned(),
        session_generation: 1,
        client_contract_revision: "client".to_owned(),
        security_policy_revision: "security".to_owned(),
    };
    let canonical_command_bytes =
        br#"{"command_schema":"storyos.command.delete-chapter.request.v1"}"#.to_vec();
    let digest = {
        use sha2::{Digest as _, Sha256};
        let value = Sha256::digest(&canonical_command_bytes).iter().fold(
            String::with_capacity(64),
            |mut value, byte| {
                use std::fmt::Write as _;
                write!(value, "{byte:02x}").expect("writing to String cannot fail");
                value
            },
        );
        format!("sha256:storyos.command.deleteChapter.jcs.v1:{value}")
    };
    DeleteChapterCommand {
        project_scope: project_scope.clone(),
        client_binding: client_binding.clone(),
        challenge_binding: ProjectCommandChallengeBinding {
            project_scope,
            client_session_binding_digest: client_binding.binding_ref,
            client_session_generation: client_binding.session_generation,
            client_contract_revision: client_binding.client_contract_revision,
            security_policy_revision: client_binding.security_policy_revision,
            limit_profile_revision: "limit".to_owned(),
            challenge_rate_policy_revision: "rate".to_owned(),
            method: "DELETE".to_owned(),
            route_template: "/api/v1/projects/{project_id}/chapters/{chapter_id}".to_owned(),
            command_schema: "storyos.command.delete-chapter.request.v1".to_owned(),
            command_kind: "deleteChapter".to_owned(),
            canonical_command_digest: digest,
            idempotency_key: "idempotency".to_owned(),
        },
        nonce_digest: "nonce".to_owned(),
        canonical_command_bytes,
        correlation_id: "correlation".to_owned(),
        chapter_id: ChapterId::new("chapter"),
        expected_chapter_revision: 4,
        ids: AuthorCommandAdmissionIds {
            command_id: "command".to_owned(),
            author_command_admission_id: "admission".to_owned(),
            receipt_id: "receipt".to_owned(),
        },
    }
}

#[tokio::test]
async fn an_admitted_delete_chapter_reaches_the_store() {
    let store = Store(Mutex::new(0));
    let settlement = delete_chapter(&store, &command()).await.unwrap();
    assert_eq!(*store.0.lock().unwrap(), 1);
    assert_eq!(
        settlement.effect,
        DeleteChapterSettlementEffect::Applied {
            tree_revision: 5,
            volume_id: "volume".to_owned(),
            current: storyos_core::DeleteChapterCurrent::PreserveExisting,
        }
    );
}

#[tokio::test]
async fn a_mismatched_delete_chapter_challenge_is_a_binding_conflict() {
    let store = Store(Mutex::new(0));
    let mut mismatched = command();
    mismatched.challenge_binding.command_kind = "updateChapter".to_owned();
    assert!(matches!(
        delete_chapter(&store, &mismatched).await,
        Err(DeleteChapterError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}
