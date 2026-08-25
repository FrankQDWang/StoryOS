use super::*;
use storyos_application::{
    ArchiveProjectCommand, ArchiveProjectSettlementEffect, AuthorCommandAdmissionIds,
    EditorClientBinding, IssueProjectCommandChallenge, ProjectCommandChallengeBinding, ProjectId,
    ProjectScope, UserId, archive_project, issue_project_command_challenge,
};
use tokio_postgres::NoTls;

const USER_A: &str = "018f0000-0000-7001-8000-000000000201";
const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const PROJECT: &str = "018f0000-0000-7001-8000-000000000603";
const CLIENT: &str = "storyos.web-client.release-1.v3";
const SECURITY: &str = "storyos.web-security-policy.release-1.v1";
const COMMAND_BYTES: &[u8] = br#"{"expected_project_revision":"1"}"#;
const LATER_COMMAND_BYTES: &[u8] = br#"{"expected_project_revision":"2"}"#;
const COMMAND_DIGEST: &str = "sha256:storyos.command.archiveProject.jcs.v1:991082a47282fb31cb629fec61a6ee68b53b5aabfc409154bc8eae46b9c7a30e";

fn later_digest() -> String {
    use sha2::{Digest as _, Sha256};
    let value = Sha256::digest(LATER_COMMAND_BYTES).iter().fold(
        String::with_capacity(64),
        |mut value, byte| {
            use std::fmt::Write as _;
            write!(value, "{byte:02x}").expect("writing to String cannot fail");
            value
        },
    );
    format!("sha256:storyos.command.archiveProject.jcs.v1:{value}")
}

fn issue_request(idempotency_suffix: &str) -> IssueProjectCommandChallenge {
    issue_named_request(idempotency_suffix, COMMAND_DIGEST)
}

fn issue_named_request(
    idempotency_suffix: &str,
    canonical_command_digest: &str,
) -> IssueProjectCommandChallenge {
    IssueProjectCommandChallenge {
        binding: ProjectCommandChallengeBinding {
            project_scope: ProjectScope::new(UserId::new(USER_A), ProjectId::new(PROJECT)),
            client_session_binding_digest: "sha256:session-a".to_owned(),
            client_session_generation: 1,
            client_contract_revision: CLIENT.to_owned(),
            security_policy_revision: SECURITY.to_owned(),
            limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
            challenge_rate_policy_revision:
                "storyos.project-command-challenge-rate.fixed-window.v1".to_owned(),
            method: "PUT".to_owned(),
            route_template: "/api/v1/projects/{project_id}/archival".to_owned(),
            command_schema: "storyos.command.archive-project.request.v1".to_owned(),
            command_kind: "archiveProject".to_owned(),
            canonical_command_digest: canonical_command_digest.to_owned(),
            idempotency_key: format!("018f0000-0000-7001-8000-00000000{idempotency_suffix}"),
        },
        nonce: format!("opaque-nonce-{idempotency_suffix}"),
        nonce_digest: format!("sha256:nonce-{idempotency_suffix}"),
    }
}

fn command(
    binding: ProjectCommandChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
    expected_revision: u64,
    bytes: &[u8],
) -> ArchiveProjectCommand {
    ArchiveProjectCommand {
        project_scope: binding.project_scope.clone(),
        client_binding: EditorClientBinding {
            binding_ref: binding.client_session_binding_digest.clone(),
            session_generation: binding.client_session_generation,
            client_contract_revision: binding.client_contract_revision.clone(),
            security_policy_revision: binding.security_policy_revision.clone(),
        },
        challenge_binding: binding,
        nonce_digest: nonce_digest.to_owned(),
        canonical_command_bytes: bytes.to_vec(),
        correlation_id: format!("018f0000-0000-7001-8000-00000000{ids_suffix}"),
        expected_revision,
        ids: AuthorCommandAdmissionIds {
            command_id: format!("018f0000-0000-7001-8000-00000001{ids_suffix}"),
            author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{ids_suffix}"),
            receipt_id: format!("018f0000-0000-7001-8000-00000003{ids_suffix}"),
        },
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn archive_project_is_atomic_replayable_and_scope_safe() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK.lock().await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let (admin, admin_connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        admin_connection.await.unwrap();
    });
    admin
        .execute(
            "INSERT INTO storyos.users (user_id) VALUES ($1::text::uuid)
             ON CONFLICT DO NOTHING",
            &[&USER_A],
        )
        .await
        .unwrap();
    admin
        .execute(
            "INSERT INTO storyos.projects (owner_user_id, project_id, title, current_chapter_id)
             VALUES ($1::text::uuid, $2::text::uuid, 'Empty Novel', NULL)",
            &[&USER_A, &PROJECT],
        )
        .await
        .unwrap();
    let store = PostgresProjectReader::new(runtime_url.clone());
    let first_issue = issue_request("0601");
    issue_project_command_challenge(&store, &first_issue)
        .await
        .unwrap();
    let first = archive_project(
        &store,
        &command(
            first_issue.binding.clone(),
            &first_issue.nonce_digest,
            "0602",
            1,
            COMMAND_BYTES,
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        first.effect,
        ArchiveProjectSettlementEffect::Applied { revision: 2 }
    );
    let replay = archive_project(
        &store,
        &command(
            first_issue.binding.clone(),
            &first_issue.nonce_digest,
            "0699",
            1,
            COMMAND_BYTES,
        ),
    )
    .await
    .unwrap();
    assert_eq!(replay, first);

    let stale_issue = issue_request("0603");
    issue_project_command_challenge(&store, &stale_issue)
        .await
        .unwrap();
    let stale = archive_project(
        &store,
        &command(
            stale_issue.binding.clone(),
            &stale_issue.nonce_digest,
            "0604",
            1,
            COMMAND_BYTES,
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        stale.effect,
        ArchiveProjectSettlementEffect::Conflicted {
            reason: storyos_core::ArchiveProjectConflict::StaleProjectRevision,
        }
    );

    let later_issue = issue_named_request("0605", &later_digest());
    issue_project_command_challenge(&store, &later_issue)
        .await
        .unwrap();
    let already = archive_project(
        &store,
        &command(
            later_issue.binding.clone(),
            &later_issue.nonce_digest,
            "0606",
            2,
            LATER_COMMAND_BYTES,
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        already.effect,
        ArchiveProjectSettlementEffect::NoEffect {
            reason: storyos_core::ArchiveProjectNoEffect::AlreadyArchived,
        }
    );

    let row = admin
        .query_one(
            "SELECT lifecycle_state, revision::text,
                    (SELECT count(*) FROM storyos.domain_receipts
                      WHERE project_id = $1::text::uuid AND command_kind = 'archiveProject'),
                    (SELECT count(*) FROM storyos.project_activity_event_payloads
                      WHERE project_id = $1::text::uuid
                        AND event_kind = 'project_archival_changed'),
                    (SELECT count(*) FROM storyos.project_archival_decisions
                      WHERE project_id = $1::text::uuid)
               FROM storyos.projects
              WHERE project_id = $1::text::uuid",
            &[&PROJECT],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            row.get::<_, String>(0),
            row.get::<_, String>(1),
            row.get::<_, i64>(2),
            row.get::<_, i64>(3),
            row.get::<_, i64>(4)
        ),
        ("archived".to_owned(), "2".to_owned(), 3, 1, 1)
    );

    let frozen = archive_project(
        &store,
        &command(
            first_issue.binding.clone(),
            &first_issue.nonce_digest,
            "0698",
            1,
            COMMAND_BYTES,
        ),
    )
    .await
    .unwrap();
    assert_eq!(frozen, first);

    let (mut runtime, connection) = tokio_postgres::connect(&runtime_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        connection.await.unwrap();
    });
    let foreign = runtime.transaction().await.unwrap();
    foreign
        .execute("SELECT set_config('storyos.user_id', $1, true)", &[&USER_B])
        .await
        .unwrap();
    let hidden = foreign
        .query_one(
            "SELECT count(*) FROM storyos.projects WHERE project_id = $1::text::uuid",
            &[&PROJECT],
        )
        .await
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(hidden, 0);
}
