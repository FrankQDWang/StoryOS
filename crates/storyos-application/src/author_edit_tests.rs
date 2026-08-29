use std::sync::Mutex;

use storyos_core::{AuthorEditPrimitive, AuthorEditUnit, SelectionSnapshot};

use super::*;
use crate::{ManuscriptBlock, ManuscriptBlockKind, ProjectId, UserId};

struct Store(Mutex<usize>);

impl AuthorEditStore for Store {
    async fn apply_author_edit(
        &self,
        command: &ApplyAuthorEditCommand,
    ) -> Result<AuthorEditSettlement, AuthorEditError> {
        *self.0.lock().unwrap() += 1;
        Ok(AuthorEditSettlement {
            ids: command.ids.clone(),
            effect: AuthorEditSettlementEffect::AuthoritativeApplied {
                ids: AuthoritativeAppliedIds {
                    revision_id: "revision-2".to_owned(),
                    payload_id: "payload".to_owned(),
                    authoritative_commit_id: "commit".to_owned(),
                    project_activity_event_id: "event".to_owned(),
                },
                body: "Base!".to_owned(),
                blocks: vec![ManuscriptBlock {
                    manuscript_block_id: "block".to_owned(),
                    block_kind: ManuscriptBlockKind::Paragraph,
                    text: "Base!".to_owned(),
                }],
                author_action_sequence: 1,
                project_activity_position: 1,
            },
            receipt_created_at: "2026-08-15T00:00:00.000Z".to_owned(),
            completed_intent_record_id: command.completed_intent_record_id.clone(),
            local_intent_sequence: command.local_intent_sequence,
        })
    }
}

fn command() -> ApplyAuthorEditCommand {
    let project_scope = ProjectScope::new(UserId::new("user"), ProjectId::new("project"));
    let client_binding = EditorClientBinding {
        binding_ref: "binding".to_owned(),
        session_generation: 1,
        client_contract_revision: "client".to_owned(),
        security_policy_revision: "security".to_owned(),
    };
    ApplyAuthorEditCommand {
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
            route_template: "/api/v1/projects/{project_id}/manuscript/author-edits".to_owned(),
            command_schema: "storyos.command.apply-author-edit.request.v1".to_owned(),
            command_kind: "applyAuthorEdit".to_owned(),
            canonical_command_digest: "sha256:storyos.command.applyAuthorEdit.jcs.v1:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a".to_owned(),
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
        editor_session_id: EditorSessionId::new("editor"),
        writer_generation: 1,
        chapter_id: "chapter".to_owned(),
        expected_authoritative_revision_id: "revision-1".to_owned(),
        expected_proposal_head_revision_ids: Vec::new(),
        target_refs: vec!["manuscript:chapter".to_owned()],
        observed_ownership_partition: "authoritative".to_owned(),
        editor_contract_revision: "storyos.editor-contract.release-1.v2".to_owned(),
        undo_group_id: "undo".to_owned(),
        completed_intent_record_id: "intent".to_owned(),
        local_intent_sequence: 1,
        author_edit_units: vec![AuthorEditUnit {
            normalized_primitives: vec![AuthorEditPrimitive::ReplaceSelection {
                from: 4,
                to: 4,
                text: "!".to_owned(),
            }],
            selection_snapshot: SelectionSnapshot {
                coordinate_profile: storyos_core::UTF16_COORDINATE_PROFILE.to_owned(),
                from: 4,
                to: 4,
            },
        }],
    }
}

#[tokio::test]
async fn matching_bindings_reach_the_atomic_store_once() {
    let store = Store(Mutex::new(0));
    let mut command = command();
    command.author_edit_units.push(AuthorEditUnit {
        normalized_primitives: vec![AuthorEditPrimitive::ReplaceSelection {
            from: 5,
            to: 5,
            text: "?".to_owned(),
        }],
        selection_snapshot: SelectionSnapshot {
            coordinate_profile: storyos_core::UTF16_COORDINATE_PROFILE.to_owned(),
            from: 5,
            to: 5,
        },
    });
    let settlement = apply_author_edit(&store, &command).await.unwrap();
    assert_eq!(
        settlement.effect,
        AuthorEditSettlementEffect::AuthoritativeApplied {
            ids: AuthoritativeAppliedIds {
                revision_id: "revision-2".to_owned(),
                payload_id: "payload".to_owned(),
                authoritative_commit_id: "commit".to_owned(),
                project_activity_event_id: "event".to_owned(),
            },
            body: "Base!".to_owned(),
            blocks: vec![ManuscriptBlock {
                manuscript_block_id: "block".to_owned(),
                block_kind: ManuscriptBlockKind::Paragraph,
                text: "Base!".to_owned(),
            }],
            author_action_sequence: 1,
            project_activity_position: 1,
        }
    );
    assert_eq!(*store.0.lock().unwrap(), 1);
}

#[tokio::test]
async fn scope_substitution_fails_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut command = command();
    command.challenge_binding.project_scope.project_id = ProjectId::new("foreign");
    assert!(matches!(
        apply_author_edit(&store, &command).await,
        Err(AuthorEditError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}

#[tokio::test]
async fn canonical_payload_substitution_fails_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut command = command();
    command.canonical_command_bytes = br#"{\"changed\":true}"#.to_vec();
    assert!(matches!(
        apply_author_edit(&store, &command).await,
        Err(AuthorEditError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}
