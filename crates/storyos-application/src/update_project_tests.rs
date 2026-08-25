use std::sync::Mutex;

use super::*;
use crate::{ProjectId, UserId};

struct Store(Mutex<usize>);

impl UpdateProjectStore for Store {
    async fn update_project(
        &self,
        command: &UpdateProjectCommand,
    ) -> Result<UpdateProjectSettlement, UpdateProjectError> {
        *self.0.lock().unwrap() += 1;
        Ok(UpdateProjectSettlement {
            ids: command.ids.clone(),
            effect: UpdateProjectSettlementEffect::Applied {
                title: command.title.clone(),
                revision: command.expected_revision + 1,
            },
            receipt_created_at: "2026-08-25T00:00:00.000Z".to_owned(),
            project_activity_position: 2,
            project_activity_event_id: "event".to_owned(),
        })
    }
}

fn command() -> UpdateProjectCommand {
    let project_scope = ProjectScope::new(UserId::new("user"), ProjectId::new("project"));
    let client_binding = EditorClientBinding {
        binding_ref: "binding".to_owned(),
        session_generation: 1,
        client_contract_revision: "client".to_owned(),
        security_policy_revision: "security".to_owned(),
    };
    UpdateProjectCommand {
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
            route_template: "/api/v1/projects/{project_id}".to_owned(),
            command_schema: "storyos.command.update-project.request.v1".to_owned(),
            command_kind: "updateProject".to_owned(),
            canonical_command_digest: "sha256:storyos.command.updateProject.jcs.v1:66b57fdc630e2dac89aa30ad5d2ccb87e8ca64e89d59e95b6c4f2c68fa1b3e6d".to_owned(),
            idempotency_key: "key".to_owned(),
        },
        nonce_digest: "nonce".to_owned(),
        canonical_command_bytes: br#"{"title":"Renamed Novel"}"#.to_vec(),
        correlation_id: "correlation".to_owned(),
        title: "Renamed Novel".to_owned(),
        expected_revision: 1,
        ids: AuthorCommandAdmissionIds {
            command_id: "command".to_owned(),
            author_command_admission_id: "admission".to_owned(),
            receipt_id: "receipt".to_owned(),
        },
    }
}

#[tokio::test]
async fn an_exact_update_project_binding_reaches_the_store() {
    let store = Store(Mutex::new(0));
    let settlement = update_project(&store, &command()).await.unwrap();
    assert_eq!(*store.0.lock().unwrap(), 1);
    assert_eq!(
        settlement.effect,
        UpdateProjectSettlementEffect::Applied {
            title: "Renamed Novel".to_owned(),
            revision: 2,
        }
    );
}

#[tokio::test]
async fn a_changed_update_project_binding_is_refused_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut changed = command();
    changed.challenge_binding.command_kind = "createProject".to_owned();
    assert!(matches!(
        update_project(&store, &changed).await,
        Err(UpdateProjectError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}

#[tokio::test]
async fn an_invalid_title_is_refused_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut empty = command();
    empty.title.clear();
    assert!(matches!(
        update_project(&store, &empty).await,
        Err(UpdateProjectError::BindingConflict)
    ));
    let mut too_long = command();
    too_long.title = "n".repeat(1025);
    assert!(matches!(
        update_project(&store, &too_long).await,
        Err(UpdateProjectError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}

#[tokio::test]
async fn a_changed_retry_digest_is_refused_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut changed = command();
    changed.canonical_command_bytes = br#"{"title":"Other"}"#.to_vec();
    assert!(matches!(
        update_project(&store, &changed).await,
        Err(UpdateProjectError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}
