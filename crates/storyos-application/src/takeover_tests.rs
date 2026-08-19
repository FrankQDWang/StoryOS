use std::sync::Mutex;

use super::*;
use crate::{ProjectId, UserId};

struct Store(Mutex<usize>);

impl TakeOverProjectWriterStore for Store {
    async fn take_over_project_writer(
        &self,
        command: &TakeOverProjectWriterCommand,
    ) -> Result<TakeOverProjectWriterSettlement, TakeOverProjectWriterError> {
        *self.0.lock().unwrap() += 1;
        Ok(TakeOverProjectWriterSettlement {
            ids: command.ids.clone(),
            receipt_created_at: "2026-08-19T00:00:00.000Z".to_owned(),
            expected_heads: vec!["revision".to_owned()],
            prior_heads: vec!["revision".to_owned()],
            resulting_heads: vec!["revision".to_owned()],
            effect: TakeOverProjectWriterEffect::TakeoverApplied {
                prior_editor_session_id: "writer".to_owned(),
                prior_writer_generation: 1,
                resulting_editor_session_id: command.editor_session_id.as_ref().to_owned(),
                resulting_writer_generation: 2,
                resulting_snapshot_id: "snapshot".to_owned(),
                resulting_snapshot_activity_position: 1,
            },
        })
    }
}

fn command() -> TakeOverProjectWriterCommand {
    let project_scope = ProjectScope::new(UserId::new("user"), ProjectId::new("project"));
    let client_binding = EditorClientBinding {
        binding_ref: "binding".to_owned(),
        session_generation: 1,
        client_contract_revision: "client".to_owned(),
        security_policy_revision: "security".to_owned(),
    };
    TakeOverProjectWriterCommand {
        project_scope: project_scope.clone(),
        client_binding: client_binding.clone(),
        challenge_binding: ProjectCommandChallengeBinding {
            project_scope,
            client_session_binding_digest: client_binding.binding_ref.clone(),
            client_session_generation: 1,
            client_contract_revision: "client".to_owned(),
            security_policy_revision: "security".to_owned(),
            limit_profile_revision: "limit".to_owned(),
            challenge_rate_policy_revision: "rate".to_owned(),
            method: "POST".to_owned(),
            route_template:
                "/api/v1/projects/{project_id}/editor-sessions/{editor_session_id}/takeovers"
                    .to_owned(),
            command_schema: "storyos.command.take-over-project-writer.request.v1".to_owned(),
            command_kind: "takeOverProjectWriter".to_owned(),
            canonical_command_digest: "sha256:storyos.command.takeOverProjectWriter.jcs.v1:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a".to_owned(),
            idempotency_key: "key".to_owned(),
        },
        nonce_digest: "nonce".to_owned(),
        canonical_command_bytes: br#"{}"#.to_vec(),
        correlation_id: "correlation".to_owned(),
        ids: AuthorCommandAdmissionIds {
            command_id: "command".to_owned(),
            author_command_admission_id: "admission".to_owned(),
            receipt_id: "receipt".to_owned(),
        },
        editor_session_id: EditorSessionId::new("observer"),
        observed_writer_generation: 1,
        editor_contract_revision: "storyos.editor-contract.release-1.v2".to_owned(),
    }
}

#[tokio::test]
async fn matching_bindings_reach_the_store_once() {
    let store = Store(Mutex::new(0));
    let command = command();
    let settlement = take_over_project_writer(&store, &command).await.unwrap();
    assert_eq!(
        settlement.effect,
        TakeOverProjectWriterEffect::TakeoverApplied {
            prior_editor_session_id: "writer".to_owned(),
            prior_writer_generation: 1,
            resulting_editor_session_id: "observer".to_owned(),
            resulting_writer_generation: 2,
            resulting_snapshot_id: "snapshot".to_owned(),
            resulting_snapshot_activity_position: 1,
        }
    );
    assert_eq!(*store.0.lock().unwrap(), 1);
}

#[tokio::test]
async fn canonical_payload_substitution_fails_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut command = command();
    command.canonical_command_bytes = br#"{"changed":true}"#.to_vec();
    assert!(matches!(
        take_over_project_writer(&store, &command).await,
        Err(TakeOverProjectWriterError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}
