use storyos_application::{
    ApplyAuthorEditCommand, AuthorCommandAdmissionIds, EditorClientBinding, EditorSessionId,
    IssueProjectCommandChallenge, OpenEditorSession, ProjectCommandChallengeBinding, ProjectId,
    ProjectScope, UserId, create_editor_session, issue_project_command_challenge,
};
use storyos_core::{AuthorEditPrimitive, AuthorEditUnit, SelectionSnapshot};
use tokio_postgres::NoTls;

use super::*;

const USER: &str = "018f0000-0000-7001-8000-000000000001";
const PROJECT: &str = "018f0000-0000-7001-8000-000000000002";
const CHAPTER: &str = "018f0000-0000-7001-8000-000000000003";
const REVISION: &str = "018f0000-0000-7001-8000-000000000005";

#[test]
fn payload_digest_is_lowercase_sha256() {
    assert_eq!(
        sha256_hex(b"Base!"),
        "f284a09f33779160d1efe782e2ad3f630b0ece4127b45c9f4caf3715c980fae0"
    );
}

fn binding(
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

fn author_command(
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
            revision_id: format!("018f0000-0000-7001-8000-000000000{suffix}4"),
            payload_id: format!("018f0000-0000-7001-8000-000000000{suffix}5"),
            authoritative_commit_id: format!("018f0000-0000-7001-8000-000000000{suffix}6"),
            project_activity_event_id: format!("018f0000-0000-7001-8000-000000000{suffix}7"),
        },
        editor_session_id: EditorSessionId::new(editor_session_id),
        writer_generation: 1,
        chapter_id: CHAPTER.to_owned(),
        expected_authoritative_revision_id: REVISION.to_owned(),
        expected_proposal_head_revision_ids: Vec::new(),
        target_refs: vec![format!("manuscript:{CHAPTER}")],
        observed_ownership_partition: "authoritative".to_owned(),
        editor_contract_revision: "storyos.editor-contract.release-1.v1".to_owned(),
        undo_group_id: format!("018f0000-0000-7001-8000-000000000{suffix}8"),
        completed_intent_record_id: format!("018f0000-0000-7001-8000-000000000{suffix}9"),
        local_intent_sequence: suffix.parse().unwrap(),
        author_edit_units: vec![AuthorEditUnit {
            normalized_primitives: vec![AuthorEditPrimitive::ReplaceSelection {
                from: 15,
                to: 15,
                text: "!".to_owned(),
            }],
            selection_snapshot: SelectionSnapshot {
                coordinate_profile: storyos_core::UTF16_COORDINATE_PROFILE.to_owned(),
                from: 15,
                to: 15,
            },
        }],
    };
    bind_canonical_payload(&mut command);
    command
}

fn bind_canonical_payload(command: &mut ApplyAuthorEditCommand) {
    let storyos_core::AuthorEditPrimitive::ReplaceSelection { from, to, text } =
        &command.author_edit_units[0].normalized_primitives[0];
    let snapshot = &command.author_edit_units[0].selection_snapshot;
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
        "author_edit_units": [{
            "normalized_primitives": [{"kind": "replace_selection", "from": from,
                "to": to, "text": text}],
            "selection_snapshot": {"coordinate_profile": snapshot.coordinate_profile,
                "from": snapshot.from, "to": snapshot.to}
        }]
    });
    command.canonical_command_bytes = serde_json::to_vec(&payload).unwrap();
    command.challenge_binding.canonical_command_digest = format!(
        "sha256:storyos.command.applyAuthorEdit.jcs.v1:{}",
        sha256_hex(&command.canonical_command_bytes)
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn three_author_edit_fault_cuts_have_complete_negative_evidence() {
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

    let cuts = [
        (
            "018f0000-0000-7001-8000-000000000711",
            "sha256:cut-1",
            "71",
            AuthorEditFault::AfterAdmissionBeforeCore,
        ),
        (
            "018f0000-0000-7001-8000-000000000721",
            "sha256:cut-2",
            "72",
            AuthorEditFault::CoreBeforeCommit,
        ),
        (
            "018f0000-0000-7001-8000-000000000731",
            "sha256:cut-3",
            "73",
            AuthorEditFault::CoreAfterCommitBeforeAcknowledgement,
        ),
    ];
    let mut committed_command = None;
    for (key, nonce_digest, suffix, fault) in cuts {
        let command = author_command(&scope, editor_session_id, key, nonce_digest, suffix);
        issue_project_command_challenge(
            &store,
            &IssueProjectCommandChallenge {
                binding: command.challenge_binding.clone(),
                nonce: format!("nonce-{suffix}"),
                nonce_digest: nonce_digest.to_owned(),
            },
        )
        .await
        .unwrap();
        let error = store
            .apply_author_edit_with_fault(&command, fault)
            .await
            .expect_err("the deterministic fault must stop acknowledgement");
        let expected_point = match fault {
            AuthorEditFault::AfterAdmissionBeforeCore => "CFP-ADMISSION-BEFORE-CORE",
            AuthorEditFault::CoreBeforeCommit => "CFP-CORE-BEFORE-COMMIT",
            AuthorEditFault::CoreAfterCommitBeforeAcknowledgement => {
                "CFP-CORE-AFTER-COMMIT-BEFORE-ACK"
            }
            AuthorEditFault::None => unreachable!(),
        };
        assert!(format!("{error:?}").contains(expected_point), "{error:?}");
        if fault == AuthorEditFault::CoreAfterCommitBeforeAcknowledgement {
            committed_command = Some(command);
        }
    }
    let command = committed_command.unwrap();
    let replay = storyos_application::apply_author_edit(&store, &command)
        .await
        .unwrap();
    assert_eq!(
        replay.effect,
        storyos_application::AuthorEditSettlementEffect::AuthoritativeApplied {
            body: "Authoritative A!".to_owned(),
            author_action_sequence: 1,
        }
    );
    assert_eq!(replay.ids, command.ids);

    let outcome_specs = [
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
    ];
    let mut outcomes = Vec::new();
    for (index, (key, nonce_digest, suffix)) in outcome_specs.into_iter().enumerate() {
        let mut outcome_command =
            author_command(&scope, editor_session_id, key, nonce_digest, suffix);
        if index > 0 {
            outcome_command.expected_authoritative_revision_id = command.ids.revision_id.clone();
        }
        if index == 1 {
            let storyos_core::AuthorEditPrimitive::ReplaceSelection { from, to, text } =
                &mut outcome_command.author_edit_units[0].normalized_primitives[0];
            *from = 16;
            *to = 16;
            text.clear();
            outcome_command.author_edit_units[0].selection_snapshot.from = 16;
            outcome_command.author_edit_units[0].selection_snapshot.to = 16;
        } else if index == 2 {
            outcome_command.author_edit_units[0].selection_snapshot.to = 14;
        }
        bind_canonical_payload(&mut outcome_command);
        issue_project_command_challenge(
            &store,
            &IssueProjectCommandChallenge {
                binding: outcome_command.challenge_binding.clone(),
                nonce: format!("outcome-nonce-{suffix}"),
                nonce_digest: nonce_digest.to_owned(),
            },
        )
        .await
        .unwrap();
        let settlement = storyos_application::apply_author_edit(&store, &outcome_command)
            .await
            .unwrap();
        let exact_retry = storyos_application::apply_author_edit(&store, &outcome_command)
            .await
            .unwrap();
        assert_eq!(exact_retry.effect, settlement.effect);
        assert_eq!(exact_retry.ids.receipt_id, settlement.ids.receipt_id);
        outcomes.push(settlement);
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

    let (mut admin, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move { connection.await.unwrap() });

    admin
        .execute(
            "UPDATE storyos.domain_receipts
                SET expected_heads = ARRAY[$4::text::uuid]
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND receipt_id = $3::text::uuid",
            &[
                &USER,
                &PROJECT,
                &command.ids.receipt_id,
                &command.ids.revision_id,
            ],
        )
        .await
        .unwrap();
    assert!(matches!(
        storyos_application::apply_author_edit(&store, &command).await,
        Err(storyos_application::AuthorEditError::BindingConflict)
    ));
    admin
        .execute(
            "UPDATE storyos.domain_receipts
                SET expected_heads = ARRAY[$4::text::uuid]
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND receipt_id = $3::text::uuid",
            &[&USER, &PROJECT, &command.ids.receipt_id, &REVISION],
        )
        .await
        .unwrap();

    admin
        .execute(
            "UPDATE storyos.author_command_admissions
                SET target_refs = ARRAY['manuscript:corrupt']::text[]
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND author_command_admission_id = $3::text::uuid",
            &[&USER, &PROJECT, &command.ids.author_command_admission_id],
        )
        .await
        .unwrap();
    assert!(matches!(
        storyos_application::apply_author_edit(&store, &command).await,
        Err(storyos_application::AuthorEditError::BindingConflict)
    ));
    admin
        .execute(
            "UPDATE storyos.author_command_admissions
                SET target_refs = ARRAY[$4]::text[]
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND author_command_admission_id = $3::text::uuid",
            &[
                &USER,
                &PROJECT,
                &command.ids.author_command_admission_id,
                &format!("manuscript:{CHAPTER}"),
            ],
        )
        .await
        .unwrap();

    let corrupt_prior = admin.transaction().await.unwrap();
    corrupt_prior
        .execute(
            "UPDATE storyos.authoritative_revision_envelopes
                SET parent_revision_id = $4::text::uuid,
                    payload_digest = 'sha256:corrupt'
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND creator_ref = $3::text::uuid",
            &[
                &USER,
                &PROJECT,
                &command.ids.author_command_admission_id,
                &command.ids.revision_id,
            ],
        )
        .await
        .unwrap();
    corrupt_prior
        .execute(
            "UPDATE storyos.authoritative_commits SET prior_revision_id = $4::text::uuid
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND author_command_admission_id = $3::text::uuid",
            &[
                &USER,
                &PROJECT,
                &command.ids.author_command_admission_id,
                &command.ids.revision_id,
            ],
        )
        .await
        .unwrap();
    corrupt_prior.commit().await.unwrap();
    assert!(matches!(
        storyos_application::apply_author_edit(&store, &command).await,
        Err(storyos_application::AuthorEditError::BindingConflict)
    ));
    let restore_prior = admin.transaction().await.unwrap();
    restore_prior
        .execute(
            "UPDATE storyos.authoritative_revision_envelopes
                SET parent_revision_id = $4::text::uuid, payload_digest = $5
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND creator_ref = $3::text::uuid",
            &[
                &USER,
                &PROJECT,
                &command.ids.author_command_admission_id,
                &REVISION,
                &sha256_hex(b"Authoritative A!"),
            ],
        )
        .await
        .unwrap();
    restore_prior
        .execute(
            "UPDATE storyos.authoritative_commits SET prior_revision_id = $4::text::uuid
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND author_command_admission_id = $3::text::uuid",
            &[
                &USER,
                &PROJECT,
                &command.ids.author_command_admission_id,
                &REVISION,
            ],
        )
        .await
        .unwrap();
    restore_prior.commit().await.unwrap();

    let evidence = admin
        .query_one(
            "SELECT
           (SELECT count(*) FROM storyos.author_command_admissions
             WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
           (SELECT count(*) FROM storyos.domain_receipts
             WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
           (SELECT count(*) FROM storyos.author_action_entries
             WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
           (SELECT count(*) FROM storyos.project_activity_events
             WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
           (SELECT count(*) FROM storyos.author_command_admissions AS admission
             WHERE admission.owner_user_id = $1::text::uuid
               AND admission.project_id = $2::text::uuid
               AND NOT EXISTS (
                 SELECT 1 FROM storyos.author_command_admission_settlements AS settlement
                  WHERE (settlement.owner_user_id, settlement.project_id,
                         settlement.author_command_admission_id) =
                        (admission.owner_user_id, admission.project_id,
                         admission.author_command_admission_id))),
           (SELECT count(*) FROM storyos.author_command_admissions
             WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
               AND command_payload->>'chapter_id' = $3
               AND challenge_consumed_at IS NOT NULL
               AND challenge_expires_at > challenge_consumed_at),
           (SELECT count(DISTINCT result_kind) FROM storyos.domain_receipts
             WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
           (SELECT count(*) FROM storyos.domain_receipts
             WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
               AND ((result_kind = 'authoritative_applied'
                     AND (authoritative_commit_id IS NULL OR resulting_revision_id IS NULL
                          OR author_action_sequence IS NULL))
                 OR (result_kind <> 'authoritative_applied'
                     AND (authoritative_commit_id IS NOT NULL OR resulting_revision_id IS NOT NULL
                          OR author_action_sequence IS NOT NULL)))),
           (SELECT concat(author_action_sequence, '/', authoritative_commit_sequence, '/',
                          project_activity_position)
              FROM storyos.scope_counters
             WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
           (SELECT convert_from(payload.canonical_bytes, 'UTF8')
              FROM storyos.authoritative_heads AS head
              JOIN storyos.authoritative_revisions AS revision
                ON (revision.owner_user_id, revision.project_id, revision.manuscript_object_id,
                    revision.revision_id) =
                   (head.owner_user_id, head.project_id, head.manuscript_object_id,
                    head.current_revision_id)
              JOIN storyos.authoritative_payloads AS payload
                ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
                   (revision.owner_user_id, revision.project_id, revision.payload_id)
             WHERE head.owner_user_id = $1::text::uuid AND head.project_id = $2::text::uuid
               AND head.manuscript_object_id = $3::text::uuid)",
            &[&USER, &PROJECT, &CHAPTER],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            evidence.get::<_, i64>(0),
            evidence.get::<_, i64>(1),
            evidence.get::<_, i64>(2),
            evidence.get::<_, i64>(3),
            evidence.get::<_, i64>(4),
            evidence.get::<_, i64>(5),
            evidence.get::<_, i64>(6),
            evidence.get::<_, i64>(7),
            evidence.get::<_, String>(8),
            evidence.get::<_, String>(9),
        ),
        (
            6,
            4,
            1,
            4,
            2,
            6,
            4,
            0,
            "1/1/4".to_owned(),
            "Authoritative A!".to_owned()
        )
    );

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
            "DELETE FROM storyos.authoritative_revisions
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND revision_id = $3::text::uuid",
            &[&USER, &PROJECT, &command.ids.revision_id],
        )
        .await
        .unwrap();
    cleanup
        .execute(
            "DELETE FROM storyos.authoritative_payloads
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND payload_id = $3::text::uuid",
            &[&USER, &PROJECT, &command.ids.payload_id],
        )
        .await
        .unwrap();
    cleanup
        .execute(
            "DELETE FROM storyos.editor_session_base_snapshots
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND editor_session_id = $3::text::uuid",
            &[&USER, &PROJECT, &editor_session_id],
        )
        .await
        .unwrap();
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
