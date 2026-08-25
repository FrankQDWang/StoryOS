use std::sync::Mutex;

use super::*;
use crate::{ProjectId, UserId};

struct Store(Mutex<usize>);

impl ArchiveProjectStore for Store {
    async fn archive_project(
        &self,
        command: &ArchiveProjectCommand,
    ) -> Result<ArchiveProjectSettlement, ArchiveProjectError> {
        *self.0.lock().unwrap() += 1;
        Ok(ArchiveProjectSettlement {
            ids: command.ids.clone(),
            effect: ArchiveProjectSettlementEffect::Applied { revision: 2 },
            receipt_created_at: "2026-08-25T00:00:00.000Z".to_owned(),
            project_activity_position: 2,
            project_activity_event_id: "event".to_owned(),
        })
    }
}

fn command() -> ArchiveProjectCommand {
    let project_scope = ProjectScope::new(UserId::new("user"), ProjectId::new("project"));
    let client_binding = EditorClientBinding {
        binding_ref: "binding".to_owned(),
        session_generation: 1,
        client_contract_revision: "client".to_owned(),
        security_policy_revision: "security".to_owned(),
    };
    ArchiveProjectCommand {
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
            route_template: "/api/v1/projects/{project_id}/archival".to_owned(),
            command_schema: "storyos.command.archive-project.request.v1".to_owned(),
            command_kind: "archiveProject".to_owned(),
            canonical_command_digest: "sha256:storyos.command.archiveProject.jcs.v1:991082a47282fb31cb629fec61a6ee68b53b5aabfc409154bc8eae46b9c7a30e".to_owned(),
            idempotency_key: "key".to_owned(),
        },
        nonce_digest: "nonce".to_owned(),
        canonical_command_bytes: br#"{"expected_project_revision":"1"}"#.to_vec(),
        correlation_id: "correlation".to_owned(),
        expected_revision: 1,
        ids: AuthorCommandAdmissionIds {
            command_id: "command".to_owned(),
            author_command_admission_id: "admission".to_owned(),
            receipt_id: "receipt".to_owned(),
        },
    }
}

#[tokio::test]
async fn an_exact_archive_project_binding_reaches_the_store() {
    let store = Store(Mutex::new(0));
    let settlement = archive_project(&store, &command()).await.unwrap();
    assert_eq!(*store.0.lock().unwrap(), 1);
    assert_eq!(
        settlement.effect,
        ArchiveProjectSettlementEffect::Applied { revision: 2 }
    );
}

#[tokio::test]
async fn a_changed_archive_project_binding_is_refused_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut changed = command();
    changed.challenge_binding.command_kind = "updateProject".to_owned();
    assert!(matches!(
        archive_project(&store, &changed).await,
        Err(ArchiveProjectError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}

#[tokio::test]
async fn a_changed_retry_digest_is_refused_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut changed = command();
    changed.canonical_command_bytes = br#"{"expected_project_revision":"2"}"#.to_vec();
    assert!(matches!(
        archive_project(&store, &changed).await,
        Err(ArchiveProjectError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}

#[tokio::test]
async fn an_invalid_expected_revision_is_refused_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut zero = command();
    zero.expected_revision = 0;
    assert!(matches!(
        archive_project(&store, &zero).await,
        Err(ArchiveProjectError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}
