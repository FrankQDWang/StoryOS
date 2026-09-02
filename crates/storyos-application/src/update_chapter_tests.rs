use std::sync::Mutex;

use super::*;
use crate::{ProjectId, UserId};

struct Store(Mutex<usize>);

impl UpdateChapterStore for Store {
    async fn update_chapter(
        &self,
        command: &UpdateChapterCommand,
    ) -> Result<UpdateChapterSettlement, UpdateChapterError> {
        *self.0.lock().unwrap() += 1;
        Ok(UpdateChapterSettlement {
            ids: command.ids.clone(),
            effect: UpdateChapterSettlementEffect::Applied {
                title: command.title.clone(),
                order: command.order,
                tree_revision: command.expected_tree_revision + 1,
            },
            receipt_created_at: "2026-08-27T00:00:00.000Z".to_owned(),
            project_activity_position: 3,
            project_activity_event_id: "event".to_owned(),
        })
    }
}

fn command() -> UpdateChapterCommand {
    let project_scope = ProjectScope::new(UserId::new("user"), ProjectId::new("project"));
    let client_binding = EditorClientBinding {
        binding_ref: "binding".to_owned(),
        session_generation: 1,
        client_contract_revision: "client".to_owned(),
        security_policy_revision: "security".to_owned(),
    };
    UpdateChapterCommand {
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
            method: "PATCH".to_owned(),
            route_template: "/api/v1/projects/{project_id}/chapters/{chapter_id}".to_owned(),
            command_schema: "storyos.command.update-chapter.request.v1".to_owned(),
            command_kind: "updateChapter".to_owned(),
            canonical_command_digest: "sha256:storyos.command.updateChapter.jcs.v1:ec43e66fca51bf0b18b0fa53c3401fb67b79e6a4228e92fec3d315a42713f0d4".to_owned(),
            idempotency_key: "key".to_owned(),
        },
        nonce_digest: "nonce".to_owned(),
        canonical_command_bytes:
            br#"{"expected_tree_revision":"2","order":"2","title":"Chapter B"}"#.to_vec(),
        correlation_id: "correlation".to_owned(),
        chapter_id: ChapterId::new("chapter"),
        title: "Chapter B".to_owned(),
        order: 2,
        expected_tree_revision: 2,
        ids: AuthorCommandAdmissionIds {
            command_id: "command".to_owned(),
            author_command_admission_id: "admission".to_owned(),
            receipt_id: "receipt".to_owned(),
        },
    }
}

#[tokio::test]
async fn an_exact_update_chapter_binding_reaches_the_store() {
    let store = Store(Mutex::new(0));
    let settlement = update_chapter(&store, &command()).await.unwrap();
    assert_eq!(*store.0.lock().unwrap(), 1);
    assert_eq!(
        settlement.effect,
        UpdateChapterSettlementEffect::Applied {
            title: "Chapter B".to_owned(),
            order: 2,
            tree_revision: 3,
        }
    );
}

#[tokio::test]
async fn a_changed_update_chapter_binding_is_refused_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut changed = command();
    changed.challenge_binding.command_kind = "createChapter".to_owned();
    assert!(matches!(
        update_chapter(&store, &changed).await,
        Err(UpdateChapterError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}

#[tokio::test]
async fn an_invalid_title_or_order_is_refused_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut empty = command();
    empty.title.clear();
    assert!(matches!(
        update_chapter(&store, &empty).await,
        Err(UpdateChapterError::BindingConflict)
    ));
    let mut zero_order = command();
    zero_order.order = 0;
    assert!(matches!(
        update_chapter(&store, &zero_order).await,
        Err(UpdateChapterError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}

#[tokio::test]
async fn a_changed_retry_digest_is_refused_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut changed = command();
    changed.canonical_command_bytes = br#"{"title":"Other"}"#.to_vec();
    assert!(matches!(
        update_chapter(&store, &changed).await,
        Err(UpdateChapterError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}
