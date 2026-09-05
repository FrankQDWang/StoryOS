use std::sync::Mutex;

use super::*;
use crate::{ProjectId, UserId};

struct Store(Mutex<usize>);

impl CreateChapterStore for Store {
    async fn create_chapter(
        &self,
        command: &CreateChapterCommand,
    ) -> Result<CreateChapterSettlement, CreateChapterError> {
        *self.0.lock().unwrap() += 1;
        Ok(CreateChapterSettlement {
            ids: command.ids.clone(),
            effect: CreateChapterSettlementEffect::Applied {
                tree_revision: 3,
                chapter_id: "chapter".to_owned(),
                current: storyos_core::CreateChapterCurrent::SelectCreated,
                order: CreateChapterPublicOrder::CanonicalSiblingOrder(1),
            },
            receipt_created_at: "2026-08-27T00:00:00.000Z".to_owned(),
            project_activity_position: 3,
            project_activity_event_id: "event".to_owned(),
        })
    }
}

fn command() -> CreateChapterCommand {
    let project_scope = ProjectScope::new(UserId::new("user"), ProjectId::new("project"));
    let client_binding = EditorClientBinding {
        binding_ref: "binding".to_owned(),
        session_generation: 1,
        client_contract_revision: "client".to_owned(),
        security_policy_revision: "security".to_owned(),
    };
    CreateChapterCommand {
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
            route_template: "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters".to_owned(),
            command_schema: "storyos.command.create-chapter.request.v1".to_owned(),
            command_kind: "createChapter".to_owned(),
            canonical_command_digest: "sha256:storyos.command.createChapter.jcs.v1:fdee6f3020b94d08f6e75dfab8be7caf86d1a8c84c521254b03e6a45153a1de2".to_owned(),
            idempotency_key: "key".to_owned(),
        },
        nonce_digest: "nonce".to_owned(),
        canonical_command_bytes: br#"{"expected_tree_revision":"2","title":"Chapter A"}"#.to_vec(),
        correlation_id: "correlation".to_owned(),
        volume_id: "volume".to_owned(),
        title: "Chapter A".to_owned(),
        expected_tree_revision: 2,
        ids: AuthorCommandAdmissionIds {
            command_id: "command".to_owned(),
            author_command_admission_id: "admission".to_owned(),
            receipt_id: "receipt".to_owned(),
        },
    }
}

#[tokio::test]
async fn an_exact_create_chapter_binding_reaches_the_store() {
    let store = Store(Mutex::new(0));
    let settlement = create_chapter(&store, &command()).await.unwrap();
    assert_eq!(*store.0.lock().unwrap(), 1);
    assert_eq!(
        settlement.effect,
        CreateChapterSettlementEffect::Applied {
            tree_revision: 3,
            chapter_id: "chapter".to_owned(),
            current: storyos_core::CreateChapterCurrent::SelectCreated,
            order: CreateChapterPublicOrder::CanonicalSiblingOrder(1),
        }
    );
}

#[tokio::test]
async fn a_changed_create_chapter_binding_is_refused_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut changed = command();
    changed.challenge_binding.command_kind = "createVolume".to_owned();
    assert!(matches!(
        create_chapter(&store, &changed).await,
        Err(CreateChapterError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}

#[tokio::test]
async fn a_changed_retry_digest_is_refused_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut changed = command();
    changed.canonical_command_bytes =
        br#"{"expected_tree_revision":"3","title":"Chapter A"}"#.to_vec();
    assert!(matches!(
        create_chapter(&store, &changed).await,
        Err(CreateChapterError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}

#[tokio::test]
async fn an_invalid_expected_tree_revision_is_refused_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut zero = command();
    zero.expected_tree_revision = 0;
    assert!(matches!(
        create_chapter(&store, &zero).await,
        Err(CreateChapterError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}

#[tokio::test]
async fn an_empty_volume_join_is_refused_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut empty = command();
    empty.volume_id.clear();
    assert!(matches!(
        create_chapter(&store, &empty).await,
        Err(CreateChapterError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}
