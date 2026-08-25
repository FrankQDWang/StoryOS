use std::sync::Mutex;

use super::*;
use crate::{ProjectId, UserId};

struct Store(Mutex<usize>);

impl CreateProjectStore for Store {
    async fn create_project(
        &self,
        command: &CreateProjectCommand,
    ) -> Result<CreateProjectSettlement, CreateProjectError> {
        *self.0.lock().unwrap() += 1;
        Ok(CreateProjectSettlement {
            ids: command.ids.clone(),
            title: command.title.clone(),
            receipt_created_at: "2026-08-25T00:00:00.000Z".to_owned(),
            project_activity_position: 1,
            project_activity_event_id: "event".to_owned(),
        })
    }
}

fn command() -> CreateProjectCommand {
    let project_scope = ProjectScope::new(UserId::new("user"), ProjectId::new("project"));
    let client_binding = EditorClientBinding {
        binding_ref: "binding".to_owned(),
        session_generation: 1,
        client_contract_revision: "client".to_owned(),
        security_policy_revision: "security".to_owned(),
    };
    CreateProjectCommand {
        project_scope: project_scope.clone(),
        client_binding: client_binding.clone(),
        challenge_binding: CreateProjectChallengeBinding {
            owner_user_id: project_scope.owner_user_id.clone(),
            prospective_project_id: project_scope.project_id.clone(),
            client_session_binding_digest: client_binding.binding_ref,
            client_session_generation: 1,
            client_contract_revision: "client".to_owned(),
            security_policy_revision: "security".to_owned(),
            limit_profile_revision: "limit".to_owned(),
            method: "POST".to_owned(),
            route_template: "/api/v1/projects".to_owned(),
            command_schema: "storyos.command.create-project.request.v1".to_owned(),
            command_kind: "createProject".to_owned(),
            create_input_digest: "input".to_owned(),
            canonical_command_digest: "digest".to_owned(),
            idempotency_key: "key".to_owned(),
        },
        nonce_digest: "nonce".to_owned(),
        canonical_command_bytes: br#"{"title":"Novel"}"#.to_vec(),
        correlation_id: "correlation".to_owned(),
        title: "Novel".to_owned(),
        ids: AuthorCommandAdmissionIds {
            command_id: "command".to_owned(),
            author_command_admission_id: "admission".to_owned(),
            receipt_id: "receipt".to_owned(),
        },
    }
}

#[tokio::test]
async fn an_exact_create_project_binding_reaches_the_store() {
    let store = Store(Mutex::new(0));
    let settlement = create_project(&store, &command()).await.unwrap();
    assert_eq!(*store.0.lock().unwrap(), 1);
    assert_eq!(settlement.title, "Novel");
}

#[tokio::test]
async fn a_changed_create_project_binding_is_refused_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut changed = command();
    changed.challenge_binding.command_kind = "updateProject".to_owned();
    assert!(matches!(
        create_project(&store, &changed).await,
        Err(CreateProjectError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}
