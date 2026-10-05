use super::*;
use storyos_application::{
    ArchiveProjectInput, ArchiveProjectSettlement, IssueProjectCommandChallenge,
    ProjectCommandChallengeBinding, ProjectId, ProjectScope, UserId,
    issue_project_command_challenge,
};
use storyos_core::{
    ArchiveProjectApplied, ArchiveProjectConflict, ArchiveProjectNoEffect, TransitionOutcome,
};
use tokio_postgres::NoTls;

use crate::command_sequence::tests::{CommandCall, command_call};

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
    issue: &IssueProjectCommandChallenge,
    ids_suffix: &str,
    expected_revision: u64,
    bytes: &[u8],
) -> CommandCall<ArchiveProjectInput> {
    command_call(
        issue.binding.clone(),
        &issue.nonce_digest,
        ids_suffix,
        bytes,
        ArchiveProjectInput { expected_revision },
    )
}

async fn archive_project(
    store: &PostgresProjectReader,
    call: &CommandCall<ArchiveProjectInput>,
) -> ArchiveProjectSettlement {
    store
        .archive_project(&call.envelope, &call.input)
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn archive_project_archives_only_its_scope_once_and_replays_the_captured_project() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
            &first_issue,
            "0602",
            /*expected_revision*/ 1,
            COMMAND_BYTES,
        ),
    )
    .await;
    let TransitionOutcome::Applied(applied) = &first.outcome else {
        panic!("the archival must apply, got {:?}", first.outcome);
    };
    assert_eq!(applied.effect, ArchiveProjectApplied { revision: 2 });

    let stale_issue = issue_request("0603");
    issue_project_command_challenge(&store, &stale_issue)
        .await
        .unwrap();
    let stale = archive_project(
        &store,
        &command(
            &stale_issue,
            "0604",
            /*expected_revision*/ 1,
            COMMAND_BYTES,
        ),
    )
    .await;
    assert_eq!(
        stale.outcome,
        TransitionOutcome::Conflicted(ArchiveProjectConflict::StaleProjectRevision)
    );

    let later_issue = issue_named_request("0605", &later_digest());
    issue_project_command_challenge(&store, &later_issue)
        .await
        .unwrap();
    let already = archive_project(
        &store,
        &command(
            &later_issue,
            "0606",
            /*expected_revision*/ 2,
            LATER_COMMAND_BYTES,
        ),
    )
    .await;
    assert_eq!(
        already.outcome,
        TransitionOutcome::NoEffect(ArchiveProjectNoEffect::AlreadyArchived)
    );

    let row = admin
        .query_one(
            "SELECT lifecycle_state, revision::text,
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
            row.get::<_, i64>(2)
        ),
        ("archived".to_owned(), "2".to_owned(), 1)
    );

    let frozen = archive_project(
        &store,
        &command(
            &first_issue,
            "0698",
            /*expected_revision*/ 1,
            COMMAND_BYTES,
        ),
    )
    .await;
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
