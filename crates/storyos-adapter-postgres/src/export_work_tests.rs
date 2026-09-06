use storyos_application::{
    AuthorCommandAdmissionIds, ClaimedExportWork, EditorClientBinding,
    ExportHumanReadableManuscriptCommand, ExportProjectArchiveCommand,
    HUMAN_READABLE_EXPORT_COMMAND_KIND, HUMAN_READABLE_EXPORT_DIGEST_PROFILE,
    HUMAN_READABLE_EXPORT_REQUEST_SCHEMA, HUMAN_READABLE_EXPORT_ROUTE,
    IssueProjectCommandChallenge, PROJECT_EXPORT_ARCHIVE_PATH_PROFILE,
    PROJECT_EXPORT_ARCHIVE_PROFILE, PROJECT_EXPORT_COMMAND_KIND, PROJECT_EXPORT_DIGEST_PROFILE,
    PROJECT_EXPORT_REQUEST_SCHEMA, PROJECT_EXPORT_ROUTE, ProjectCommandChallengeBinding, ProjectId,
    ProjectScope, UserId, claim_next_export_work, issue_project_command_challenge,
    request_export_project_archive, request_human_readable_manuscript_export,
};
use tokio_postgres::NoTls;

use crate::PostgresProjectReader;
use crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK;

const USER: &str = "018f0000-0000-7001-8000-000000000001";
const PROJECT: &str = "018f0000-0000-7001-8000-000000000002";

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_empty_combined_export_claim_uses_one_session_and_one_transaction() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let (admin, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    remove_export_work_rows(&admin).await;
    let previous_trace = enable_export_claim_statement_trace(&admin).await;
    let application_name = format!("storyos-export-work-claim-{}", fresh_id());
    let store = PostgresProjectReader::new(with_application_name(&runtime_url, &application_name));
    let since = unix_seconds_now();
    let claimed = claim_next_export_work(&store).await.unwrap();
    let trace = wait_for_export_claim_postgres_trace(&application_name, since).await;
    restore_export_claim_statement_trace(&admin, previous_trace).await;
    remove_export_work_rows(&admin).await;

    assert_eq!(claimed, None);
    assert_eq!(
        trace.authorized_sessions, 1,
        "an empty combined claim must authorize one PostgreSQL session: {trace:?}"
    );
    assert_eq!(
        trace.begin_statements, 1,
        "an empty combined claim must begin one PostgreSQL transaction: {trace:?}"
    );
    assert_eq!(
        trace.commit_statements, 1,
        "an empty combined claim must commit one PostgreSQL transaction: {trace:?}"
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_combined_export_claim_takes_readable_work_before_archive_work() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(&runtime_url);
    let scope = ProjectScope::new(UserId::new(USER), ProjectId::new(PROJECT));
    let (admin, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    remove_export_work_rows(&admin).await;
    let readable_export_id = admit_readable_export(&store, &scope).await;
    let archive_export_id = admit_archive_export(&store, &scope).await;

    let claimed = claim_next_export_work(&store)
        .await
        .unwrap()
        .expect("readable work must be claimed");
    let ClaimedExportWork::Readable(readable) = claimed else {
        panic!("readable-first selection must claim readable work: {claimed:?}");
    };
    assert_eq!(readable.export_id, readable_export_id);
    let archive = admin
        .query_one(
            "SELECT claim_generation, wakeup_pending
               FROM storyos.project_export_operations
              WHERE export_id = $1::text::uuid",
            &[&archive_export_id],
        )
        .await
        .unwrap();
    assert_eq!(archive.get::<_, i64>(0), 0);
    assert!(archive.get::<_, bool>(1));
    remove_export_work_rows(&admin).await;
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_combined_export_claim_falls_back_to_archive_work() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(&runtime_url);
    let scope = ProjectScope::new(UserId::new(USER), ProjectId::new(PROJECT));
    let (admin, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    remove_export_work_rows(&admin).await;
    let archive_export_id = admit_archive_export(&store, &scope).await;

    let claimed = claim_next_export_work(&store)
        .await
        .unwrap()
        .expect("Archive work must be claimed");
    let ClaimedExportWork::Archive(archive) = claimed else {
        panic!("Archive fallback must claim Archive work: {claimed:?}");
    };
    assert_eq!(archive.export_id, archive_export_id);
    remove_export_work_rows(&admin).await;
}

#[derive(Debug)]
struct ExportClaimPostgresTrace {
    authorized_sessions: usize,
    begin_statements: usize,
    commit_statements: usize,
}

fn with_application_name(database_url: &str, application_name: &str) -> String {
    let separator = if database_url.contains('?') { '&' } else { '?' };
    format!("{database_url}{separator}application_name={application_name}")
}

fn unix_seconds_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time is after the Unix epoch")
        .as_secs()
}

struct PostgresLogSettings {
    log_connections: String,
    log_statement: String,
    log_line_prefix: String,
}

async fn enable_export_claim_statement_trace(
    admin: &tokio_postgres::Client,
) -> PostgresLogSettings {
    let previous = admin
        .query_one(
            "SELECT current_setting('log_connections'),
                    current_setting('log_statement'),
                    current_setting('log_line_prefix')",
            &[],
        )
        .await
        .unwrap();
    let previous = PostgresLogSettings {
        log_connections: previous.get(0),
        log_statement: previous.get(1),
        log_line_prefix: previous.get(2),
    };
    for statement in [
        "ALTER SYSTEM SET log_connections = on",
        "ALTER SYSTEM SET log_statement = 'all'",
        "ALTER SYSTEM SET log_line_prefix = '%m [%p] %a '",
        "SELECT pg_reload_conf()",
    ] {
        admin.batch_execute(statement).await.unwrap();
    }
    previous
}

async fn restore_export_claim_statement_trace(
    admin: &tokio_postgres::Client,
    previous: PostgresLogSettings,
) {
    let log_connections = quote_postgres_literal(&previous.log_connections);
    let log_statement = quote_postgres_literal(&previous.log_statement);
    let log_line_prefix = quote_postgres_literal(&previous.log_line_prefix);
    for statement in [
        format!("ALTER SYSTEM SET log_connections = {log_connections}"),
        format!("ALTER SYSTEM SET log_statement = {log_statement}"),
        format!("ALTER SYSTEM SET log_line_prefix = {log_line_prefix}"),
        "SELECT pg_reload_conf()".to_owned(),
    ] {
        admin.batch_execute(&statement).await.unwrap();
    }
}

fn quote_postgres_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

async fn wait_for_export_claim_postgres_trace(
    application_name: &str,
    since: u64,
) -> ExportClaimPostgresTrace {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let trace = export_claim_postgres_trace(application_name, since);
            if trace.authorized_sessions >= 1
                && trace.begin_statements >= 1
                && trace.commit_statements >= 1
            {
                return trace;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the controlled PostgreSQL trace must record the empty combined claim")
}

fn export_claim_postgres_trace(application_name: &str, since: u64) -> ExportClaimPostgresTrace {
    let container = std::env::var("STORYOS_TEST_POSTGRES_CONTAINER")
        .expect("run through scripts/verify-project-scope.sh");
    let output = std::process::Command::new("docker")
        .args(["logs", "--since", &since.to_string(), &container])
        .output()
        .expect("docker logs must read the controlled PostgreSQL trace");
    let logs = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let authorized_sessions = logs
        .lines()
        .filter(|line| line.contains("connection authorized:") && line.contains(application_name))
        .count();
    let begin_statements = logs
        .lines()
        .filter(|line| {
            line.contains(application_name)
                && (line.contains("statement: BEGIN")
                    || line.contains("statement: START TRANSACTION"))
        })
        .count();
    let commit_statements = logs
        .lines()
        .filter(|line| line.contains(application_name) && line.contains("statement: COMMIT"))
        .count();
    ExportClaimPostgresTrace {
        authorized_sessions,
        begin_statements,
        commit_statements,
    }
}

async fn admit_readable_export(store: &PostgresProjectReader, scope: &ProjectScope) -> String {
    let command = readable_export_command(scope);
    issue_project_command_challenge(
        store,
        &IssueProjectCommandChallenge {
            binding: command.challenge_binding.clone(),
            nonce: command
                .nonce_digest
                .trim_start_matches("sha256:")
                .to_owned(),
            nonce_digest: command.nonce_digest.clone(),
        },
    )
    .await
    .unwrap();
    request_human_readable_manuscript_export(store, &command)
        .await
        .unwrap();
    command.export_id
}

async fn admit_archive_export(store: &PostgresProjectReader, scope: &ProjectScope) -> String {
    let command = archive_export_command(scope);
    issue_project_command_challenge(
        store,
        &IssueProjectCommandChallenge {
            binding: command.challenge_binding.clone(),
            nonce: command
                .nonce_digest
                .trim_start_matches("sha256:")
                .to_owned(),
            nonce_digest: command.nonce_digest.clone(),
        },
    )
    .await
    .unwrap();
    request_export_project_archive(store, &command)
        .await
        .unwrap();
    command.export_id
}

fn fresh_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

fn readable_export_command(scope: &ProjectScope) -> ExportHumanReadableManuscriptCommand {
    let client_binding = EditorClientBinding {
        binding_ref: "binding:author-edit".to_owned(),
        session_generation: 1,
        client_contract_revision: "storyos.web-client.release-1.v1".to_owned(),
        security_policy_revision: "storyos.web-security-policy.release-1.v1".to_owned(),
    };
    let canonical_command_bytes =
        br#"{"command_schema":"storyos.command.export-human-readable-manuscript.request.v1"}"#
            .to_vec();
    let command_digest = command_digest(
        HUMAN_READABLE_EXPORT_DIGEST_PROFILE,
        &canonical_command_bytes,
    );
    let export_id = fresh_id();
    ExportHumanReadableManuscriptCommand {
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
            route_template: HUMAN_READABLE_EXPORT_ROUTE.to_owned(),
            command_schema: HUMAN_READABLE_EXPORT_REQUEST_SCHEMA.to_owned(),
            command_kind: HUMAN_READABLE_EXPORT_COMMAND_KIND.to_owned(),
            canonical_command_digest: command_digest,
            idempotency_key: fresh_id(),
        },
        nonce_digest: format!("sha256:nonce-export-work-readable-{export_id}"),
        canonical_command_bytes,
        correlation_id: fresh_id(),
        ids: AuthorCommandAdmissionIds {
            command_id: fresh_id(),
            author_command_admission_id: fresh_id(),
            receipt_id: fresh_id(),
        },
        export_id,
    }
}

fn archive_export_command(scope: &ProjectScope) -> ExportProjectArchiveCommand {
    let client_binding = EditorClientBinding {
        binding_ref: "binding:author-edit".to_owned(),
        session_generation: 1,
        client_contract_revision: "storyos.web-client.release-1.v1".to_owned(),
        security_policy_revision: "storyos.web-security-policy.release-1.v1".to_owned(),
    };
    let canonical_command_bytes =
        br#"{"command_schema":"storyos.command.export-project-archive.request.v1"}"#.to_vec();
    let command_digest = command_digest(PROJECT_EXPORT_DIGEST_PROFILE, &canonical_command_bytes);
    let export_id = fresh_id();
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
            idempotency_key: fresh_id(),
        },
        nonce_digest: format!("sha256:nonce-export-work-archive-{export_id}"),
        canonical_command_bytes,
        correlation_id: fresh_id(),
        ids: AuthorCommandAdmissionIds {
            command_id: fresh_id(),
            author_command_admission_id: fresh_id(),
            receipt_id: fresh_id(),
        },
        export_id,
        archive_profile: PROJECT_EXPORT_ARCHIVE_PROFILE.to_owned(),
        archive_path_profile: PROJECT_EXPORT_ARCHIVE_PATH_PROFILE.to_owned(),
    }
}

fn command_digest(profile: &str, canonical_command_bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    let value = Sha256::digest(canonical_command_bytes).iter().fold(
        String::with_capacity(64),
        |mut value, byte| {
            use std::fmt::Write as _;
            write!(value, "{byte:02x}").expect("writing to String cannot fail");
            value
        },
    );
    format!("sha256:{profile}:{value}")
}

async fn remove_export_work_rows(admin: &tokio_postgres::Client) {
    admin
        .batch_execute(
            "DELETE FROM storyos.human_readable_manuscript_exports;
             DELETE FROM storyos.human_readable_manuscript_export_operations;
             DELETE FROM storyos.project_export_entries;
             DELETE FROM storyos.project_export_manifests;
             DELETE FROM storyos.project_export_operations;",
        )
        .await
        .unwrap();
}
