use storyos_application::{
    ApplyAuthorEditCommand, ApplyAuthorEditOutcome, AuthorCommandAdmissionIds,
    CommittedApplyAuthorEdit, EditorClientBinding, EditorSessionId, EditorSessionLookup,
    EditorSessionSnapshot, IssueProjectCommandChallenge, OpenEditorSession,
    ProjectCommandChallengeBinding, ProjectId, ProjectScope, ResolveApplyAuthorEditOutcome, UserId,
    create_editor_session, get_apply_author_edit_outcome, get_editor_session,
    issue_project_command_challenge,
};
use storyos_core::{
    AuthorEditPrimitive, AuthorEditUnit, ManuscriptBlock, ManuscriptBlockKind, SelectionSnapshot,
};
use tokio_postgres::NoTls;

use super::*;

const USER: &str = "018f0000-0000-7001-8000-000000000001";
const PROJECT: &str = "018f0000-0000-7001-8000-000000000002";
const CHAPTER: &str = "018f0000-0000-7001-8000-000000000003";
const REVISION: &str = "018f0000-0000-7001-8000-000000000005";
const BLOCK: &str = "018f0000-0000-7001-8000-0000000000b1";
pub(crate) static AUTHOR_EDIT_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[test]
fn payload_digest_is_lowercase_sha256() {
    assert_eq!(
        sha256_hex(b"Base!"),
        "f284a09f33779160d1efe782e2ad3f630b0ece4127b45c9f4caf3715c980fae0"
    );
}

pub(crate) fn binding(
    scope: &ProjectScope,
    kind: &str,
    path: &str,
    schema: &str,
    key: &str,
) -> ProjectCommandChallengeBinding {
    ProjectCommandChallengeBinding {
        project_scope: scope.clone(),
        client_session_binding_digest: "binding:author-edit".to_owned(),
        client_session_generation: 1,
        client_contract_revision: "storyos.web-client.release-1.v1".to_owned(),
        security_policy_revision: "storyos.web-security-policy.release-1.v1".to_owned(),
        limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
        challenge_rate_policy_revision: "storyos.project-command-challenge-rate.fixed-window.v1"
            .to_owned(),
        method: "POST".to_owned(),
        route_template: path.to_owned(),
        command_schema: schema.to_owned(),
        command_kind: kind.to_owned(),
        canonical_command_digest: format!("sha256:storyos.test:{key}"),
        idempotency_key: key.to_owned(),
    }
}

pub(crate) fn author_command(
    scope: &ProjectScope,
    editor_session_id: &str,
    key: &str,
    nonce_digest: &str,
    suffix: &str,
) -> ApplyAuthorEditCommand {
    let challenge_binding = binding(
        scope,
        "applyAuthorEdit",
        "/api/v1/projects/{project_id}/manuscript/author-edits",
        "storyos.command.apply-author-edit.request.v1",
        key,
    );
    let mut command = ApplyAuthorEditCommand {
        project_scope: scope.clone(),
        client_binding: EditorClientBinding {
            binding_ref: "binding:author-edit".to_owned(),
            session_generation: 1,
            client_contract_revision: "storyos.web-client.release-1.v1".to_owned(),
            security_policy_revision: "storyos.web-security-policy.release-1.v1".to_owned(),
        },
        challenge_binding,
        nonce_digest: nonce_digest.to_owned(),
        canonical_command_bytes: Vec::new(),
        correlation_id: format!("018f0000-0000-7001-8000-000000000{suffix}0"),
        ids: AuthorCommandAdmissionIds {
            command_id: format!("018f0000-0000-7001-8000-000000000{suffix}1"),
            author_command_admission_id: format!("018f0000-0000-7001-8000-000000000{suffix}2"),
            receipt_id: format!("018f0000-0000-7001-8000-000000000{suffix}3"),
        },
        editor_session_id: EditorSessionId::new(editor_session_id),
        writer_generation: 1,
        chapter_id: CHAPTER.to_owned(),
        expected_authoritative_revision_id: REVISION.to_owned(),
        expected_proposal_head_revision_ids: Vec::new(),
        proposal_target: None,
        target_refs: vec![format!("manuscript:{CHAPTER}")],
        observed_ownership_partition: "authoritative".to_owned(),
        editor_contract_revision: "storyos.editor-contract.release-1.v3".to_owned(),
        undo_group_id: format!("018f0000-0000-7001-8000-000000000{suffix}8"),
        completed_intent_record_id: format!("018f0000-0000-7001-8000-000000000{suffix}9"),
        local_intent_sequence: suffix.parse().unwrap(),
        author_edit_units: vec![
            AuthorEditUnit {
                normalized_primitives: vec![AuthorEditPrimitive::ReplaceSelection {
                    from: 15,
                    to: 15,
                    text: "x".to_owned(),
                }],
                selection_snapshot: SelectionSnapshot {
                    coordinate_profile: storyos_core::UTF16_COORDINATE_PROFILE.to_owned(),
                    from: 15,
                    to: 15,
                },
            },
            AuthorEditUnit {
                normalized_primitives: vec![AuthorEditPrimitive::ReplaceSelection {
                    from: 15,
                    to: 16,
                    text: "!".to_owned(),
                }],
                selection_snapshot: SelectionSnapshot {
                    coordinate_profile: storyos_core::UTF16_COORDINATE_PROFILE.to_owned(),
                    from: 15,
                    to: 16,
                },
            },
        ],
    };
    bind_canonical_payload(&mut command);
    command
}

pub(super) fn bind_canonical_payload(command: &mut ApplyAuthorEditCommand) {
    let author_edit_units = command
        .author_edit_units
        .iter()
        .map(|unit| {
            let [storyos_core::AuthorEditPrimitive::ReplaceSelection { from, to, text }] =
                unit.normalized_primitives.as_slice()
            else {
                panic!("test Author Edit unit must contain one ReplaceSelection")
            };
            serde_json::json!({
                "normalized_primitives": [{"kind": "replace_selection", "from": from,
                    "to": to, "text": text}],
                "selection_snapshot": {
                    "coordinate_profile": unit.selection_snapshot.coordinate_profile,
                    "from": unit.selection_snapshot.from,
                    "to": unit.selection_snapshot.to,
                }
            })
        })
        .collect::<Vec<_>>();
    let payload = serde_json::json!({
        "command_schema": command.challenge_binding.command_schema,
        "client_contract_revision": command.client_binding.client_contract_revision,
        "security_policy_revision": command.client_binding.security_policy_revision,
        "correlation_id": command.correlation_id,
        "editor_session_id": command.editor_session_id.as_ref(),
        "writer_generation": command.writer_generation.to_string(),
        "chapter_id": command.chapter_id,
        "expected_authoritative_revision_id": command.expected_authoritative_revision_id,
        "expected_proposal_head_revision_ids": command.expected_proposal_head_revision_ids,
        "target_refs": command.target_refs,
        "observed_ownership_partition": command.observed_ownership_partition,
        "editor_contract_revision": command.editor_contract_revision,
        "undo_group_id": command.undo_group_id,
        "completed_intent_record_id": command.completed_intent_record_id,
        "local_intent_sequence": command.local_intent_sequence.to_string(),
        "author_edit_units": author_edit_units,
    });
    command.canonical_command_bytes = serde_json::to_vec(&payload).unwrap();
    command.challenge_binding.canonical_command_digest = format!(
        "sha256:storyos.command.applyAuthorEdit.jcs.v1:{}",
        sha256_hex(&command.canonical_command_bytes)
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_cut_after_the_commit_resolves_through_the_outcome_query_and_keeps_the_receipt_shapes() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(&runtime_url);
    let scope = ProjectScope::new(UserId::new(USER), ProjectId::new(PROJECT));
    let session_key = "018f0000-0000-7001-8000-000000000701";
    let session_nonce_digest = "sha256:session-author-edit";
    let session_binding = binding(
        &scope,
        "createEditorSession",
        "/api/v1/projects/{project_id}/editor-sessions",
        "storyos.command.create-editor-session.request.v1",
        session_key,
    );
    issue_project_command_challenge(
        &store,
        &IssueProjectCommandChallenge {
            binding: session_binding.clone(),
            nonce: "session-nonce".to_owned(),
            nonce_digest: session_nonce_digest.to_owned(),
            earlier_revision_nonces: Vec::new(),
        },
    )
    .await
    .unwrap();
    let editor_session_id = "018f0000-0000-7001-8000-000000000702";
    create_editor_session(
        &store,
        &OpenEditorSession {
            project_scope: scope.clone(),
            editor_session_id: EditorSessionId::new(editor_session_id),
            snapshot_id: "018f0000-0000-7001-8000-000000000703".to_owned(),
            client_binding: EditorClientBinding {
                binding_ref: "binding:author-edit".to_owned(),
                session_generation: 1,
                client_contract_revision: "storyos.web-client.release-1.v1".to_owned(),
                security_policy_revision: "storyos.web-security-policy.release-1.v1".to_owned(),
            },
            challenge_binding: session_binding,
            nonce_digest: session_nonce_digest.to_owned(),
        },
    )
    .await
    .unwrap();

    let command = author_command(
        &scope,
        editor_session_id,
        "018f0000-0000-7001-8000-000000000731",
        "sha256:cut-3",
        "73",
    );
    issue_project_command_challenge(
        &store,
        &IssueProjectCommandChallenge {
            binding: command.challenge_binding.clone(),
            nonce: "nonce-73".to_owned(),
            nonce_digest: command.nonce_digest.clone(),
            earlier_revision_nonces: Vec::new(),
        },
    )
    .await
    .unwrap();
    let error = store
        .apply_author_edit_with_fault(
            &command,
            AuthorEditFault::CoreAfterCommitBeforeAcknowledgement,
        )
        .await
        .expect_err("the deterministic fault must stop acknowledgement");
    assert!(
        format!("{error:?}").contains("CFP-CORE-AFTER-COMMIT-BEFORE-ACK"),
        "{error:?}"
    );
    let committed_outcome = get_apply_author_edit_outcome(
        &store,
        &ResolveApplyAuthorEditOutcome {
            project_scope: scope.clone(),
            client_binding: command.client_binding.clone(),
            limit_profile_revision: command.challenge_binding.limit_profile_revision.clone(),
            idempotency_key: command.challenge_binding.idempotency_key.clone(),
            nonce_digest: command.nonce_digest.clone(),
        },
    )
    .await
    .unwrap();
    let replay = store.apply_author_edit(&command).await.unwrap();
    assert_eq!(
        committed_outcome,
        ApplyAuthorEditOutcome::Committed(Box::new(CommittedApplyAuthorEdit {
            correlation_id: command.correlation_id.clone(),
            canonical_command_digest: command.challenge_binding.canonical_command_digest.clone(),
            idempotency_key: command.challenge_binding.idempotency_key.clone(),
            expected_authoritative_revision_id: command.expected_authoritative_revision_id.clone(),
            settlement: replay.clone(),
        }))
    );
    let replay_applied_ids = match &replay.effect {
        storyos_application::AuthorEditSettlementEffect::AuthoritativeApplied { ids, .. } => {
            ids.clone()
        }
        _ => unreachable!(),
    };
    assert_eq!(
        replay.effect,
        storyos_application::AuthorEditSettlementEffect::AuthoritativeApplied {
            ids: replay_applied_ids.clone(),
            body: "Authoritative A!".to_owned(),
            blocks: vec![ManuscriptBlock {
                manuscript_block_id: BLOCK.to_owned(),
                block_kind: ManuscriptBlockKind::Paragraph,
                text: "Authoritative A!".to_owned(),
            }],
            author_action_sequence: 1,
            project_activity_position: 1,
        }
    );
    assert_eq!(replay.ids, command.ids);
    let refreshed_session = get_editor_session(
        &store,
        &EditorSessionLookup {
            project_scope: scope.clone(),
            editor_session_id: EditorSessionId::new(editor_session_id),
            client_binding: EditorClientBinding {
                binding_ref: "binding:author-edit".to_owned(),
                session_generation: 1,
                client_contract_revision: "storyos.web-client.release-1.v1".to_owned(),
                security_policy_revision: "storyos.web-security-policy.release-1.v1".to_owned(),
            },
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_ne!(
        refreshed_session.base_snapshot.snapshot_id,
        "018f0000-0000-7001-8000-000000000703"
    );
    assert_eq!(
        refreshed_session.base_snapshot,
        EditorSessionSnapshot {
            snapshot_id: refreshed_session.base_snapshot.snapshot_id.clone(),
            chapter_id: CHAPTER.to_owned(),
            authoritative_revision_id: replay_applied_ids.revision_id.clone(),
            project_activity_position: 1,
            body: "Authoritative A!".to_owned(),
            blocks: vec![ManuscriptBlock {
                manuscript_block_id: BLOCK.to_owned(),
                block_kind: ManuscriptBlockKind::Paragraph,
                text: "Authoritative A!".to_owned(),
            }],
            payload_digest_hex: sha256_hex(b"Authoritative A!"),
            created_at: refreshed_session.base_snapshot.created_at.clone(),
        }
    );

    let mut outcomes = Vec::new();
    for (index, (key, nonce_digest, suffix)) in [
        (
            "018f0000-0000-7001-8000-000000000741",
            "sha256:outcome-1",
            "74",
        ),
        (
            "018f0000-0000-7001-8000-000000000751",
            "sha256:outcome-2",
            "75",
        ),
        (
            "018f0000-0000-7001-8000-000000000761",
            "sha256:outcome-3",
            "76",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut outcome_command =
            author_command(&scope, editor_session_id, key, nonce_digest, suffix);
        if index > 0 {
            outcome_command.expected_authoritative_revision_id =
                replay_applied_ids.revision_id.clone();
        }
        if index == 1 {
            let storyos_core::AuthorEditPrimitive::ReplaceSelection { from, to, text } =
                &mut outcome_command.author_edit_units[0].normalized_primitives[0]
            else {
                panic!("legacy command must use ReplaceSelection")
            };
            *from = 16;
            *to = 16;
            text.clear();
            outcome_command.author_edit_units[0].selection_snapshot.from = 16;
            outcome_command.author_edit_units[0].selection_snapshot.to = 16;
        } else if index == 2 {
            // A valid first unit and an invalid second unit refuse the whole batch.
            outcome_command.author_edit_units[1].selection_snapshot.to = 15;
        }
        bind_canonical_payload(&mut outcome_command);
        issue_project_command_challenge(
            &store,
            &IssueProjectCommandChallenge {
                binding: outcome_command.challenge_binding.clone(),
                nonce: format!("outcome-nonce-{suffix}"),
                nonce_digest: nonce_digest.to_owned(),
                earlier_revision_nonces: Vec::new(),
            },
        )
        .await
        .unwrap();
        outcomes.push(store.apply_author_edit(&outcome_command).await.unwrap());
    }
    assert!(matches!(
        outcomes[0].effect,
        storyos_application::AuthorEditSettlementEffect::Conflicted {
            reason: storyos_core::AuthorEditConflict::StaleAuthoritativeHead,
            ..
        }
    ));
    assert!(matches!(
        outcomes[1].effect,
        storyos_application::AuthorEditSettlementEffect::NoEffect {
            reason: storyos_core::AuthorEditNoEffect::ContentUnchanged
        }
    ));
    assert!(matches!(
        outcomes[2].effect,
        storyos_application::AuthorEditSettlementEffect::Refused {
            reason: storyos_core::AuthorEditRefusal::InvalidSelection
        }
    ));

    let (mut injected_runtime, connection) =
        tokio_postgres::connect(&runtime_url, NoTls).await.unwrap();
    tokio::spawn(async move { connection.await.unwrap() });
    let injected = injected_runtime.transaction().await.unwrap();
    injected
        .batch_execute(
            "SET LOCAL storyos.owner_user_id = '018f0000-0000-7001-8000-000000000001';
             SET LOCAL storyos.project_id = '018f0000-0000-7001-8000-000000000002';",
        )
        .await
        .unwrap();
    injected
        .execute(
            "INSERT INTO storyos.authoritative_payloads
               (owner_user_id, project_id, payload_id, canonical_bytes)
             VALUES ($1::text::uuid, $2::text::uuid,
                     '018f0000-0000-7001-8000-000000000781'::uuid,
                     convert_to('forbidden zero authority', 'UTF8'))",
            &[&USER, &PROJECT],
        )
        .await
        .unwrap();
    injected
        .execute(
            "INSERT INTO storyos.authoritative_revisions
               (owner_user_id, project_id, manuscript_object_id, revision_id, payload_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid,
                     '018f0000-0000-7001-8000-000000000782'::uuid,
                     '018f0000-0000-7001-8000-000000000781'::uuid)",
            &[&USER, &PROJECT, &CHAPTER],
        )
        .await
        .unwrap();
    injected
        .execute(
            "INSERT INTO storyos.authoritative_revision_envelopes
               (owner_user_id, project_id, manuscript_object_id, revision_id,
                parent_revision_id, schema_revision, creator_kind, creator_ref,
                receipt_id, receipt_result_kind, cause_kind, payload_digest)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid,
                     '018f0000-0000-7001-8000-000000000782'::uuid,
                     $4::text::uuid, 'storyos.authoritative-revision-envelope.v1',
                     'author_command_admission', $5::text::uuid, $6::text::uuid,
                     'authoritative_applied',
                     'direct_author_action', 'sha256:forbidden-zero-authority')",
            &[
                &USER,
                &PROJECT,
                &CHAPTER,
                &replay_applied_ids.revision_id,
                &outcomes[0].ids.author_command_admission_id,
                &outcomes[0].ids.receipt_id,
            ],
        )
        .await
        .unwrap();
    injected
        .execute(
            "INSERT INTO storyos.authoritative_commits
                (owner_user_id, project_id, authoritative_commit_id,
                authoritative_commit_sequence, manuscript_object_id, prior_revision_id,
                resulting_revision_id, author_command_admission_id,
                receipt_id, receipt_result_kind)
             VALUES ($1::text::uuid, $2::text::uuid,
                     '018f0000-0000-7001-8000-000000000783'::uuid, 2,
                     $3::text::uuid, $4::text::uuid,
                     '018f0000-0000-7001-8000-000000000782'::uuid, $5::text::uuid,
                     $6::text::uuid, 'authoritative_applied')",
            &[
                &USER,
                &PROJECT,
                &CHAPTER,
                &replay_applied_ids.revision_id,
                &outcomes[0].ids.author_command_admission_id,
                &outcomes[0].ids.receipt_id,
            ],
        )
        .await
        .unwrap();
    assert!(injected.commit().await.is_err());

    let (mut admin, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move { connection.await.unwrap() });
    // The refused batch writes no part of its valid first unit.
    let refused_authority = admin
        .query_one(
            "SELECT
                (SELECT count(*) FROM storyos.authoritative_revision_envelopes
                  WHERE receipt_id = $1::text::uuid),
                (SELECT count(*) FROM storyos.authoritative_commits
                  WHERE receipt_id = $1::text::uuid)",
            &[&outcomes[2].ids.receipt_id],
        )
        .await
        .unwrap();
    assert_eq!(
        [0, 1].map(|index| refused_authority.get::<_, i64>(index)),
        [0, 0]
    );

    let duplicate_activity = admin
        .execute(
            "INSERT INTO storyos.project_activity_events
               (owner_user_id, project_id, project_activity_position,
                project_activity_event_id, event_kind, receipt_id,
                receipt_result_kind, authoritative_commit_id,
                resulting_revision_id, author_action_sequence)
             VALUES ($1::text::uuid, $2::text::uuid, 2,
                     '018f0000-0000-7001-8000-000000000799'::uuid,
                     'authoritative_author_edit_applied', $3::text::uuid,
                     'authoritative_applied', $4::text::uuid, $5::text::uuid, 1)",
            &[
                &USER,
                &PROJECT,
                &command.ids.receipt_id,
                &replay_applied_ids.authoritative_commit_id,
                &replay_applied_ids.revision_id,
            ],
        )
        .await;
    assert!(duplicate_activity.is_err());

    let malformed_zero_receipt = admin
        .execute(
            "UPDATE storyos.domain_receipts SET result_payload = '{}'::jsonb
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND receipt_id = $3::text::uuid",
            &[&USER, &PROJECT, &outcomes[1].ids.receipt_id],
        )
        .await;
    assert!(malformed_zero_receipt.is_err());

    let cleanup = admin.transaction().await.unwrap();
    cleanup
        .batch_execute("SET CONSTRAINTS ALL DEFERRED")
        .await
        .unwrap();
    cleanup
        .execute(
            "UPDATE storyos.authoritative_heads SET current_revision_id = $3::text::uuid
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND manuscript_object_id = $4::text::uuid",
            &[&USER, &PROJECT, &REVISION, &CHAPTER],
        )
        .await
        .unwrap();
    for table in [
        "project_activity_events",
        "author_action_entries",
        "author_command_admission_outcome_unknown_observations",
        "author_command_admission_reconfirmations",
        "author_command_admission_settlements",
        "domain_receipts",
        "authoritative_commits",
        "authoritative_revision_envelopes",
        "author_command_admissions",
    ] {
        cleanup
            .execute(
                &format!(
                    "DELETE FROM storyos.{table}
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid"
                ),
                &[&USER, &PROJECT],
            )
            .await
            .unwrap();
    }
    cleanup
        .execute(
            "DELETE FROM storyos.editor_session_base_snapshots
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND editor_session_id = $3::text::uuid",
            &[&USER, &PROJECT, &editor_session_id],
        )
        .await
        .unwrap();
    for revision_id in [&replay_applied_ids.revision_id] {
        cleanup
            .execute(
                "DELETE FROM storyos.manuscript_revision_members
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND revision_id = $3::text::uuid",
                &[&USER, &PROJECT, revision_id],
            )
            .await
            .unwrap();
        cleanup
            .execute(
                "DELETE FROM storyos.authoritative_revisions
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND revision_id = $3::text::uuid",
                &[&USER, &PROJECT, revision_id],
            )
            .await
            .unwrap();
    }
    for payload_id in [&replay_applied_ids.payload_id] {
        cleanup
            .execute(
                "DELETE FROM storyos.authoritative_payloads
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND payload_id = $3::text::uuid",
                &[&USER, &PROJECT, payload_id],
            )
            .await
            .unwrap();
    }
    cleanup
        .execute(
            "DELETE FROM storyos.project_writer_generations
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&USER, &PROJECT],
        )
        .await
        .unwrap();
    cleanup
        .execute(
            "DELETE FROM storyos.editor_sessions
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND editor_session_id = $3::text::uuid",
            &[&USER, &PROJECT, &editor_session_id],
        )
        .await
        .unwrap();
    cleanup
        .execute(
            "DELETE FROM storyos.project_command_challenges
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&USER, &PROJECT],
        )
        .await
        .unwrap();
    cleanup
        .execute(
            "DELETE FROM storyos.command_idempotency
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&USER, &PROJECT],
        )
        .await
        .unwrap();
    cleanup
        .execute(
            "DELETE FROM storyos.project_command_challenge_rate_windows
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND client_session_generation = 1",
            &[&USER, &PROJECT],
        )
        .await
        .unwrap();
    cleanup
        .execute(
            "DELETE FROM storyos.project_command_challenge_rate_guards
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND client_session_generation = 1",
            &[&USER, &PROJECT],
        )
        .await
        .unwrap();
    cleanup
        .execute(
            "DELETE FROM storyos.scope_counters
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&USER, &PROJECT],
        )
        .await
        .unwrap();
    cleanup.commit().await.unwrap();
}
