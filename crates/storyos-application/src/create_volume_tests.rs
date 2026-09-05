use std::sync::Mutex;

use super::*;
use crate::{ProjectId, UserId};

struct Store(Mutex<usize>);

impl CreateVolumeStore for Store {
    async fn create_volume(
        &self,
        command: &CreateVolumeCommand,
    ) -> Result<CreateVolumeSettlement, CreateVolumeError> {
        *self.0.lock().unwrap() += 1;
        Ok(CreateVolumeSettlement {
            ids: command.ids.clone(),
            effect: CreateVolumeSettlementEffect::Applied {
                tree_revision: 2,
                volume_id: "volume".to_owned(),
                order: CreateVolumePublicOrder::CanonicalSiblingOrder(1),
            },
            receipt_created_at: "2026-08-26T00:00:00.000Z".to_owned(),
            project_activity_position: 2,
            project_activity_event_id: "event".to_owned(),
        })
    }
}

fn command() -> CreateVolumeCommand {
    let project_scope = ProjectScope::new(UserId::new("user"), ProjectId::new("project"));
    let client_binding = EditorClientBinding {
        binding_ref: "binding".to_owned(),
        session_generation: 1,
        client_contract_revision: "client".to_owned(),
        security_policy_revision: "security".to_owned(),
    };
    CreateVolumeCommand {
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
            route_template: "/api/v1/projects/{project_id}/volumes".to_owned(),
            command_schema: "storyos.command.create-volume.request.v1".to_owned(),
            command_kind: "createVolume".to_owned(),
            canonical_command_digest: "sha256:storyos.command.createVolume.jcs.v1:2b02ae40bec5ed5ccf7ec412519d2178186dc80e3829100851514a6f3422cedf".to_owned(),
            idempotency_key: "key".to_owned(),
        },
        nonce_digest: "nonce".to_owned(),
        canonical_command_bytes: br#"{"expected_tree_revision":"1","title":"Volume A"}"#.to_vec(),
        correlation_id: "correlation".to_owned(),
        title: "Volume A".to_owned(),
        expected_tree_revision: 1,
        ids: AuthorCommandAdmissionIds {
            command_id: "command".to_owned(),
            author_command_admission_id: "admission".to_owned(),
            receipt_id: "receipt".to_owned(),
        },
    }
}

#[tokio::test]
async fn an_exact_create_volume_binding_reaches_the_store() {
    let store = Store(Mutex::new(0));
    let settlement = create_volume(&store, &command()).await.unwrap();
    assert_eq!(*store.0.lock().unwrap(), 1);
    assert_eq!(
        settlement.effect,
        CreateVolumeSettlementEffect::Applied {
            tree_revision: 2,
            volume_id: "volume".to_owned(),
            order: CreateVolumePublicOrder::CanonicalSiblingOrder(1),
        }
    );
}

#[tokio::test]
async fn a_changed_create_volume_binding_is_refused_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut changed = command();
    changed.challenge_binding.command_kind = "updateProject".to_owned();
    assert!(matches!(
        create_volume(&store, &changed).await,
        Err(CreateVolumeError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}

#[tokio::test]
async fn a_changed_retry_digest_is_refused_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut changed = command();
    changed.canonical_command_bytes =
        br#"{"expected_tree_revision":"2","title":"Volume A"}"#.to_vec();
    assert!(matches!(
        create_volume(&store, &changed).await,
        Err(CreateVolumeError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}

#[tokio::test]
async fn an_invalid_expected_tree_revision_is_refused_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut zero = command();
    zero.expected_tree_revision = 0;
    assert!(matches!(
        create_volume(&store, &zero).await,
        Err(CreateVolumeError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}
