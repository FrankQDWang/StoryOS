use super::*;
use storyos_application::{
    ArchiveProjectInput, ArchiveProjectSettlement, IssueProjectCommandChallenge, ProjectId,
    ProjectScope, UserId, issue_project_command_challenge,
};
use storyos_core::{
    ArchiveProjectApplied, ArchiveProjectConflict, ArchiveProjectNoEffect, TransitionOutcome,
};
use tokio_postgres::NoTls;

use crate::command_sequence::tests::{ARCHIVE_PROJECT, CommandCall, command_call, command_issue};

const USER_A: &str = "018f0000-0000-7001-8000-000000000201";
const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const PROJECT: &str = "018f0000-0000-7001-8000-000000000603";
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

fn project_scope() -> ProjectScope {
    ProjectScope::new(UserId::new(USER_A), ProjectId::new(PROJECT))
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
    let first_issue = command_issue(&project_scope(), "0601", &ARCHIVE_PROJECT, COMMAND_DIGEST);
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

    let stale_issue = command_issue(&project_scope(), "0603", &ARCHIVE_PROJECT, COMMAND_DIGEST);
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

    let later_issue = command_issue(&project_scope(), "0605", &ARCHIVE_PROJECT, &later_digest());
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
            row.get::<_, i64>(/*idx*/ 2)
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
