use super::*;
use storyos_application::{
    IssueProjectCommandChallenge, ProjectCommandChallengeBinding, ProjectId, ProjectScope,
    UpdateProjectInput, UpdateProjectSettlement, UserId, issue_project_command_challenge,
};
use storyos_core::{TransitionOutcome, UpdateProjectApplied, UpdateProjectConflict};
use tokio_postgres::NoTls;

use crate::command_sequence::tests::{CommandCall, command_call};

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const PROJECT: &str = "018f0000-0000-7001-8000-000000000602";
const CLIENT: &str = "storyos.web-client.release-1.v3";
const SECURITY: &str = "storyos.web-security-policy.release-1.v1";
const TITLE: &str = "Renamed Novel";
const LATER_TITLE: &str = "Later Novel";
const COMMAND_BYTES: &[u8] = br#"{"title":"Renamed Novel"}"#;
const LATER_COMMAND_BYTES: &[u8] = br#"{"title":"Later Novel"}"#;
const COMMAND_DIGEST: &str = "sha256:storyos.command.updateProject.jcs.v1:66b57fdc630e2dac89aa30ad5d2ccb87e8ca64e89d59e95b6c4f2c68fa1b3e6d";
const LATER_COMMAND_DIGEST: &str = "sha256:storyos.command.updateProject.jcs.v1:d9633f219b249234e3cee881242660df9e21d42a88d46ed5a9e5bafc32607df9";

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
            method: "PATCH".to_owned(),
            route_template: "/api/v1/projects/{project_id}".to_owned(),
            command_schema: "storyos.command.update-project.request.v1".to_owned(),
            command_kind: "updateProject".to_owned(),
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
    title: &str,
    bytes: &[u8],
) -> CommandCall<UpdateProjectInput> {
    command_call(
        issue.binding.clone(),
        &issue.nonce_digest,
        ids_suffix,
        bytes,
        UpdateProjectInput {
            title: title.to_owned(),
            expected_revision,
        },
    )
}

async fn update_project(
    store: &PostgresProjectReader,
    call: &CommandCall<UpdateProjectInput>,
) -> UpdateProjectSettlement {
    store
        .update_project(&call.envelope, &call.input)
        .await
        .unwrap()
}

fn applied_effect(settlement: &UpdateProjectSettlement) -> &UpdateProjectApplied {
    let TransitionOutcome::Applied(applied) = &settlement.outcome else {
        panic!("the rename must apply, got {:?}", settlement.outcome);
    };
    &applied.effect
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn update_project_renames_only_its_scope_and_replays_the_captured_project() {
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
            "INSERT INTO storyos.projects (owner_user_id, project_id, title, current_chapter_id)
             VALUES ($1::text::uuid, $2::text::uuid, 'Empty Novel', NULL)",
            &[&USER_A, &PROJECT],
        )
        .await
        .unwrap();
    let store = PostgresProjectReader::new(runtime_url.clone());
    let first_issue = issue_request("0501");
    issue_project_command_challenge(&store, &first_issue)
        .await
        .unwrap();
    let first = update_project(
        &store,
        &command(
            &first_issue,
            "0502",
            /*expected_revision*/ 1,
            TITLE,
            COMMAND_BYTES,
        ),
    )
    .await;
    assert_eq!(
        applied_effect(&first),
        &UpdateProjectApplied {
            title: TITLE.to_owned(),
            revision: 2,
        }
    );

    let stale_issue = issue_request("0503");
    issue_project_command_challenge(&store, &stale_issue)
        .await
        .unwrap();
    let stale = update_project(
        &store,
        &command(
            &stale_issue,
            "0504",
            /*expected_revision*/ 1,
            TITLE,
            COMMAND_BYTES,
        ),
    )
    .await;
    assert_eq!(
        stale.outcome,
        TransitionOutcome::Conflicted(UpdateProjectConflict::StaleProjectRevision)
    );

    let row = admin
        .query_one(
            "SELECT title, revision::text FROM storyos.projects WHERE project_id = $1::text::uuid",
            &[&PROJECT],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            row.get::<_, String>(/*idx*/ 0),
            row.get::<_, String>(/*idx*/ 1)
        ),
        (TITLE.to_owned(), "2".to_owned())
    );

    let later_issue = issue_named_request("0505", LATER_COMMAND_DIGEST);
    issue_project_command_challenge(&store, &later_issue)
        .await
        .unwrap();
    let later = update_project(
        &store,
        &command(
            &later_issue,
            "0506",
            /*expected_revision*/ 2,
            LATER_TITLE,
            LATER_COMMAND_BYTES,
        ),
    )
    .await;
    assert_eq!(
        applied_effect(&later),
        &UpdateProjectApplied {
            title: LATER_TITLE.to_owned(),
            revision: 3,
        }
    );
    let frozen = update_project(
        &store,
        &command(
            &first_issue,
            "0598",
            /*expected_revision*/ 1,
            TITLE,
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
            "SELECT count(*) FROM storyos.projects WHERE title = $1",
            &[&LATER_TITLE],
        )
        .await
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(hidden, 0);
}
