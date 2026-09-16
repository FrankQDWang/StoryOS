use std::sync::Mutex;

use storyos_core::AssistanceAvailability;

use super::*;
use crate::{Project, ProjectId, UserId};

struct Store(Mutex<usize>);

impl UpdateProjectAssistanceStore for Store {
    async fn update_project_assistance(
        &self,
        command: &UpdateProjectAssistanceCommand,
    ) -> Result<UpdateProjectAssistanceSettlement, UpdateProjectAssistanceError> {
        *self.0.lock().unwrap() += 1;
        Ok(UpdateProjectAssistanceSettlement {
            ids: command.ids.clone(),
            effect: UpdateProjectAssistanceSettlementEffect::Initialized {
                availability: command.availability,
                revision: 1,
            },
            receipt_created_at: "2026-09-16T00:00:00.000Z".to_owned(),
            project_activity_position: 1,
            project_activity_event_id: "event".to_owned(),
            response_project: Project {
                project_id: command.project_scope.project_id.clone(),
                title: "Empty Novel".to_owned(),
                current_chapter_id: None,
            },
            assistance: Some(ProjectAssistanceRecord {
                availability: command.availability,
                revision: 1,
                model_registration_revision: "reg".to_owned(),
                processing_destination_identity: "dest".to_owned(),
                processing_destination_identity_evidence_revision: 1,
                project_model_use_binding_revision: "bind".to_owned(),
                external_compatibility_decision: "dec".to_owned(),
            }),
        })
    }
}

fn command() -> UpdateProjectAssistanceCommand {
    let project_scope = ProjectScope::new(UserId::new("user"), ProjectId::new("project"));
    let client_binding = EditorClientBinding {
        binding_ref: "binding".to_owned(),
        session_generation: 1,
        client_contract_revision: "client".to_owned(),
        security_policy_revision: "security".to_owned(),
    };
    UpdateProjectAssistanceCommand {
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
            route_template: "/api/v1/projects/{project_id}/assistance".to_owned(),
            command_schema: "storyos.command.update-project-assistance.request.v1".to_owned(),
            command_kind: "updateProjectAssistance".to_owned(),
            canonical_command_digest: "sha256:storyos.command.updateProjectAssistance.jcs.v1:ffaa2f3a48c9a54d872221dd5552a4b936b5be24587ce275b5fbdd1963b5457a".to_owned(),
            idempotency_key: "key".to_owned(),
        },
        nonce_digest: "nonce".to_owned(),
        canonical_command_bytes: br#"{"availability":"available"}"#.to_vec(),
        correlation_id: "correlation".to_owned(),
        availability: AssistanceAvailability::Available,
        expected_revision: 0,
        ids: AuthorCommandAdmissionIds {
            command_id: "command".to_owned(),
            author_command_admission_id: "admission".to_owned(),
            receipt_id: "receipt".to_owned(),
        },
    }
}

#[tokio::test]
async fn an_exact_assistance_binding_reaches_the_store() {
    let store = Store(Mutex::new(0));
    let settlement = update_project_assistance(&store, &command()).await.unwrap();
    assert_eq!(*store.0.lock().unwrap(), 1);
    assert_eq!(
        settlement.effect,
        UpdateProjectAssistanceSettlementEffect::Initialized {
            availability: AssistanceAvailability::Available,
            revision: 1,
        }
    );
}

#[tokio::test]
async fn a_changed_assistance_binding_is_refused_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut changed = command();
    changed.challenge_binding.command_kind = "updateProject".to_owned();
    assert!(matches!(
        update_project_assistance(&store, &changed).await,
        Err(UpdateProjectAssistanceError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}

#[tokio::test]
async fn a_changed_retry_digest_is_refused_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut changed = command();
    changed.canonical_command_bytes = br#"{"availability":"unavailable"}"#.to_vec();
    assert!(matches!(
        update_project_assistance(&store, &changed).await,
        Err(UpdateProjectAssistanceError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}
