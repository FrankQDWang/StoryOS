use storyos_application::{
    ApplyAuthorEditOutcome, ApplyAuthorEditReconfirmationReason, AuthorCommandAdmissionIds,
    CanonicalSnapshot, EditorClientBinding, EditorSessionId, ExportProjectArchiveCommand,
    IssueProjectCommandChallenge, OpenEditorSession, PROJECT_EXPORT_ARCHIVE_PATH_PROFILE,
    PROJECT_EXPORT_ARCHIVE_PROFILE, PROJECT_EXPORT_COMMAND_KIND, PROJECT_EXPORT_DIGEST_PROFILE,
    PROJECT_EXPORT_REQUEST_SCHEMA, PROJECT_EXPORT_ROUTE, ProjectCommandChallengeBinding, ProjectId,
    ProjectScope, ReadApplyAuthorEditOutcome, RequiresReconfirmationApplyAuthorEdit, UserId,
    VerifiedExportArchive, create_editor_session, get_apply_author_edit_outcome,
    get_verified_export_archive, issue_project_command_challenge, request_export_project_archive,
};
use tokio_postgres::NoTls;

use crate::PostgresProjectReader;
use crate::author_edit::AuthorEditFault;
use crate::author_edit::tests::{AUTHOR_EDIT_TEST_LOCK, author_command, binding};

const USER: &str = "018f0000-0000-7001-8000-000000000001";
const PROJECT: &str = "018f0000-0000-7001-8000-000000000002";
const RECONFIRMATION_PATH: &str = "canonical/author_command_admission_reconfirmations.json";

/// StoryOS Project Export ZIP files use STORE only, so local headers carry the
/// uncompressed payload immediately after the name and extra fields.
fn zip_store_files(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    let mut offset = 0;
    while offset + 30 <= bytes.len() {
        if &bytes[offset..offset + 4] != b"PK\x03\x04" {
            break;
        }
        let name_len =
            u16::from_le_bytes(bytes[offset + 26..offset + 28].try_into().unwrap()) as usize;
        let extra_len =
            u16::from_le_bytes(bytes[offset + 28..offset + 30].try_into().unwrap()) as usize;
        let size = u32::from_le_bytes(bytes[offset + 18..offset + 22].try_into().unwrap()) as usize;
        let name_start = offset + 30;
        let name = String::from_utf8(bytes[name_start..name_start + name_len].to_vec()).unwrap();
        let data_start = name_start + name_len + extra_len;
        files.push((name, bytes[data_start..data_start + size].to_vec()));
        offset = data_start + size;
    }
    files
}

fn export_command(scope: &ProjectScope) -> ExportProjectArchiveCommand {
    let client_binding = EditorClientBinding {
        binding_ref: "binding:author-edit".to_owned(),
        session_generation: 1,
        client_contract_revision: "storyos.web-client.release-1.v1".to_owned(),
        security_policy_revision: "storyos.web-security-policy.release-1.v1".to_owned(),
    };
    let canonical_command_bytes =
        br#"{"command_schema":"storyos.command.export-project-archive.request.v1"}"#.to_vec();
    let command_digest = {
        use sha2::{Digest as _, Sha256};
        let value = Sha256::digest(&canonical_command_bytes).iter().fold(
            String::with_capacity(64),
            |mut value, byte| {
                use std::fmt::Write as _;
                write!(value, "{byte:02x}").expect("writing to String cannot fail");
                value
            },
        );
        format!("sha256:{PROJECT_EXPORT_DIGEST_PROFILE}:{value}")
    };
    ExportProjectArchiveCommand {
        project_scope: scope.clone(),
        client_binding: client_binding.clone(),
        challenge_binding: ProjectCommandChallengeBinding {
            project_scope: scope.clone(),
            client_session_binding_digest: client_binding.binding_ref.clone(),
            client_session_generation: 1,
            client_contract_revision: client_binding.client_contract_revision.clone(),
            security_policy_revision: client_binding.security_policy_revision.clone(),
            limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
            challenge_rate_policy_revision:
                "storyos.project-command-challenge-rate.fixed-window.v1".to_owned(),
            method: "POST".to_owned(),
            route_template: PROJECT_EXPORT_ROUTE.to_owned(),
            command_schema: PROJECT_EXPORT_REQUEST_SCHEMA.to_owned(),
            command_kind: PROJECT_EXPORT_COMMAND_KIND.to_owned(),
            canonical_command_digest: command_digest,
            idempotency_key: "018f0000-0000-7001-8000-0000000009e4".to_owned(),
        },
        nonce_digest: "sha256:nonce-export-reconfirm".to_owned(),
        canonical_command_bytes,
        correlation_id: "018f0000-0000-7001-8000-0000000009e0".to_owned(),
        ids: AuthorCommandAdmissionIds {
            command_id: "018f0000-0000-7001-8000-0000000009e1".to_owned(),
            author_command_admission_id: "018f0000-0000-7001-8000-0000000009e2".to_owned(),
            receipt_id: "018f0000-0000-7001-8000-0000000009e3".to_owned(),
        },
        export_id: "018f0000-0000-7001-8000-0000000009e6".to_owned(),
        archive_profile: PROJECT_EXPORT_ARCHIVE_PROFILE.to_owned(),
        archive_path_profile: PROJECT_EXPORT_ARCHIVE_PATH_PROFILE.to_owned(),
        source_snapshot: CanonicalSnapshot {
            snapshot_id: "018f0000-0000-7001-8000-0000000000a0".to_owned(),
            project_activity_position: 0,
            replay_generation: 1,
            floor_position: 0,
            redaction_profile: "storyos.author.v1".to_owned(),
            schema_profile: "storyos.public.release.1".to_owned(),
            created_at: "2026-09-01T00:00:00.000Z".to_owned(),
            expires_at: None,
        },
    }
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
        "createEditorSession",
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
        },
    )
    .await
    .unwrap();
    store
        .apply_author_edit_with_fault(&command, AuthorEditFault::AfterAdmissionBeforeCore)
        .await
        .expect_err("the reconfirmation cut must stop before Core");
    let (admin, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
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
        &ReadApplyAuthorEditOutcome {
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
            recovery_draft_ref: None,
        });
    assert_eq!(queried, expected);

    let export = export_command(&scope);
    issue_project_command_challenge(
        &store,
        &IssueProjectCommandChallenge {
            binding: export.challenge_binding.clone(),
            nonce: "nonce-export-reconfirm".to_owned(),
            nonce_digest: export.nonce_digest.clone(),
        },
    )
    .await
    .unwrap();
    request_export_project_archive(&store, &export)
        .await
        .unwrap();
    let VerifiedExportArchive::Ready(zip_bytes) =
        get_verified_export_archive(&store, &scope, &export.export_id)
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
    assert!(rows[0]["recovery_draft_ref"].is_null());
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
        &ReadApplyAuthorEditOutcome {
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
    cleanup.commit().await.unwrap();
}
