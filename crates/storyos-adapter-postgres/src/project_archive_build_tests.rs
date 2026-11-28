use storyos_application::{
    ApplyAuthorEditOutcome, ApplyAuthorEditReconfirmationReason, EditorClientBinding,
    EditorSessionId, IssueProjectCommandChallenge, OpenEditorSession, ProjectId, ProjectScope,
    RequiresReconfirmationApplyAuthorEdit, ResolveApplyAuthorEditOutcome, UserId,
    VerifiedExportArchive, claim_next_archive_export, complete_archive_export,
    create_editor_session, get_apply_author_edit_outcome, get_verified_export_archive,
    issue_project_command_challenge,
};
use storyos_contracts::CommandKind;
use storyos_core::PROJECT_EXPORT_TABLES;
use tokio_postgres::NoTls;

use crate::PostgresProjectReader;
use crate::author_edit::AuthorEditFault;
use crate::shared_tests::author_edit::{AUTHOR_EDIT_TEST_LOCK, author_command, binding};
use crate::shared_tests::project_archive_build::{export_command, zip_store_files};

const USER: &str = "018f0000-0000-7001-8000-000000000001";
const PROJECT: &str = "018f0000-0000-7001-8000-000000000002";
const RECONFIRMATION_PATH: &str = "canonical/author_command_admission_reconfirmations.json";

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_project_export_table_is_a_live_table() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let (client, connection) = tokio_postgres::connect(&runtime_url, NoTls).await.unwrap();
    let client = crate::statement_cache::Client::new(client);
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let live: std::collections::BTreeSet<String> = client
        .query(
            "SELECT tablename FROM pg_catalog.pg_tables WHERE schemaname = 'storyos'",
            &[],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(0))
        .collect();

    let missing: Vec<&str> = PROJECT_EXPORT_TABLES
        .into_iter()
        .filter(|table| !live.contains(*table))
        .collect();

    assert_eq!(missing, Vec::<&str>::new());
    for global in ["tool_registration_revisions", "tool_registration_tools"] {
        assert!(live.contains(global) && !PROJECT_EXPORT_TABLES.contains(&global));
    }
    // The archive does not hold the Embedding Attempts of the Project (ADR 0095 section 3).
    assert!(
        live.contains("embedding_attempts")
            && !super::EXPORT_TABLES
                .iter()
                .any(|(table, _)| *table == "embedding_attempts")
    );
    // StoryOS builds the passage vectors again from the Chapters, so the archive does not hold
    // them or their index work (ADR 0095 sections 2 and 4).
    assert!(
        live.contains("passage_index_work")
            && !PROJECT_EXPORT_TABLES.contains(&"passage_index_work")
            && !super::EXPORT_TABLES
                .iter()
                .any(|(table, _)| *table == "passage_index_work")
    );
    assert!(
        live.contains("passage_vectors")
            && !PROJECT_EXPORT_TABLES.contains(&"passage_vectors")
            && !super::EXPORT_TABLES
                .iter()
                .any(|(table, _)| *table == "passage_vectors")
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn project_export_packs_and_reloads_requires_reconfirmation() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(&runtime_url);
    let scope = ProjectScope::new(UserId::new(USER), ProjectId::new(PROJECT));
    let session_key = "018f0000-0000-7001-8000-000000000981";
    let session_nonce_digest = "sha256:session-export-reconfirm";
    let session_binding = binding(
        &scope,
        CommandKind::CreateEditorSession.as_str(),
        "/api/v1/projects/{project_id}/editor-sessions",
        "storyos.command.create-editor-session.request.v1",
        session_key,
    );
    issue_project_command_challenge(
        &store,
        &IssueProjectCommandChallenge {
            binding: session_binding.clone(),
            nonce: "session-nonce-export-reconfirm".to_owned(),
            nonce_digest: session_nonce_digest.to_owned(),
            earlier_revision_nonces: Vec::new(),
        },
    )
    .await
    .unwrap();
    let editor_session_id = "018f0000-0000-7001-8000-000000000982";
    let snapshot_id = "018f0000-0000-7001-8000-000000000983";
    create_editor_session(
        &store,
        &OpenEditorSession {
            project_scope: scope.clone(),
            editor_session_id: EditorSessionId::new(editor_session_id),
            snapshot_id: snapshot_id.to_owned(),
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
        "018f0000-0000-7001-8000-000000000884",
        "sha256:cut-export-reconfirm",
        "88",
    );
    issue_project_command_challenge(
        &store,
        &IssueProjectCommandChallenge {
            binding: command.challenge_binding.clone(),
            nonce: "nonce-88".to_owned(),
            nonce_digest: command.nonce_digest.clone(),
            earlier_revision_nonces: Vec::new(),
        },
    )
    .await
    .unwrap();
    store
        .apply_author_edit_with_fault(&command, AuthorEditFault::AfterAdmissionBeforeCore)
        .await
        .expect_err("the reconfirmation cut must stop before Core");
    let (admin, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    let mut admin = crate::statement_cache::Client::new(admin);
    tokio::spawn(async move {
        let _ = connection.await;
    });
    admin
        .execute(
            "UPDATE storyos.author_command_admissions
                SET command_payload = command_payload - 'author_edit_units'
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND author_command_admission_id = $3::text::uuid",
            &[&USER, &PROJECT, &command.ids.author_command_admission_id],
        )
        .await
        .unwrap();

    let queried = get_apply_author_edit_outcome(
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
    let expected =
        ApplyAuthorEditOutcome::RequiresReconfirmation(RequiresReconfirmationApplyAuthorEdit {
            command_id: command.ids.command_id.clone(),
            author_command_admission_id: command.ids.author_command_admission_id.clone(),
            reconfirmation_reason:
                ApplyAuthorEditReconfirmationReason::DirectEditIntentUnrecoverable,
        });
    assert_eq!(queried, expected);

    let (export, input) = export_command(&scope, "9e");
    issue_project_command_challenge(
        &store,
        &IssueProjectCommandChallenge {
            binding: export.challenge_binding.clone(),
            nonce: "nonce-export-reconfirm".to_owned(),
            nonce_digest: export.nonce_digest.clone(),
            earlier_revision_nonces: Vec::new(),
        },
    )
    .await
    .unwrap();
    store.export_project_archive(&export, &input).await.unwrap();
    let claimed = claim_next_archive_export(&store)
        .await
        .unwrap()
        .expect("the admitted archive must be claimable");
    complete_archive_export(&store, &claimed).await.unwrap();
    let VerifiedExportArchive::Ready(zip_bytes) =
        get_verified_export_archive(&store, &scope, &input.export_id)
            .await
            .unwrap()
    else {
        panic!("Project Export must return verified ZIP bytes");
    };
    let files = zip_store_files(&zip_bytes);
    let paths: Vec<&str> = files.iter().map(|(path, _)| path.as_str()).collect();
    assert!(
        !paths.iter().any(|path| path.contains("challenge")),
        "challenge secrets must stay out of the archive: {paths:?}"
    );
    let packed = files
        .iter()
        .find(|(path, _)| path == RECONFIRMATION_PATH)
        .map(|(_, bytes)| bytes.clone())
        .expect("the archive must pack Reconfirmation evidence");
    let rows: Vec<serde_json::Value> = serde_json::from_slice(&packed).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["owner_user_id"], USER);
    assert_eq!(rows[0]["project_id"], PROJECT);
    assert_eq!(
        rows[0]["author_command_admission_id"],
        command.ids.author_command_admission_id
    );
    assert_eq!(rows[0]["command_id"], command.ids.command_id);
    assert_eq!(
        rows[0]["reconfirmation_reason"],
        "direct_edit_intent_unrecoverable"
    );
    assert!(rows[0]["settled_at"].is_string());

    admin
        .execute(
            "DELETE FROM storyos.author_command_admission_reconfirmations
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND author_command_admission_id = $3::text::uuid",
            &[&USER, &PROJECT, &command.ids.author_command_admission_id],
        )
        .await
        .unwrap();
    admin
        .execute(
            "INSERT INTO storyos.author_command_admission_reconfirmations
             SELECT *
               FROM jsonb_populate_recordset(
                      NULL::storyos.author_command_admission_reconfirmations,
                      $1::text::jsonb
                    )",
            &[&std::str::from_utf8(&packed).unwrap()],
        )
        .await
        .unwrap();
    let reloaded = get_apply_author_edit_outcome(
        &store,
        &ResolveApplyAuthorEditOutcome {
            project_scope: scope,
            client_binding: command.client_binding.clone(),
            limit_profile_revision: command.challenge_binding.limit_profile_revision.clone(),
            idempotency_key: command.challenge_binding.idempotency_key.clone(),
            nonce_digest: command.nonce_digest.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(reloaded, expected);

    let cleanup = admin.transaction().await.unwrap();
    cleanup
        .batch_execute("SET CONSTRAINTS ALL DEFERRED")
        .await
        .unwrap();
    for table in [
        "project_export_entries",
        "project_export_manifests",
        "project_export_operations",
        "pinned_export_sources",
        "project_activity_events",
        "project_activity_event_payloads",
        "author_command_admission_outcome_unknown_observations",
        "author_command_admission_reconfirmations",
        "author_command_admission_settlements",
        "domain_receipts",
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
            "DELETE FROM storyos.project_snapshots
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND snapshot_id = $3::text::uuid",
            &[&USER, &PROJECT, &snapshot_id],
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
