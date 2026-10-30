use super::*;
use storyos_application::{
    ProjectCommandChallengeBinding, ProjectCommandError, ProjectId, ProjectScope,
    UpdateProjectAssistanceInput, UpdateProjectAssistanceSettlement, UserId,
    issue_project_command_challenge, open_project_assistance,
};
use storyos_core::HOST_FAKE_REGISTRATION;
use storyos_core::{
    AssistanceAvailability, TransitionOutcome, UpdateProjectAssistanceApplied,
    UpdateProjectAssistanceConflict, UpdateProjectAssistanceNoEffect,
};

use crate::command_sequence::tests::{
    CommandCall, UPDATE_PROJECT_ASSISTANCE, command_call, command_issue,
};
use tokio_postgres::NoTls;

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const PROJECT: &str = "018f0000-0000-7001-8000-0000000006a2";
const OTHER_PROJECT: &str = "018f0000-0000-7001-8000-0000000006a3";
const AVAILABLE_BYTES: &[u8] = br#"{"availability":"available"}"#;
const UNAVAILABLE_BYTES: &[u8] = br#"{"availability":"unavailable"}"#;

fn project_scope() -> ProjectScope {
    ProjectScope::new(UserId::new(USER_A), ProjectId::new(PROJECT))
}

fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    let value = Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut value, byte| {
            use std::fmt::Write as _;
            write!(value, "{byte:02x}").expect("writing to String cannot fail");
            value
        });
    format!("sha256:storyos.command.updateProjectAssistance.jcs.v1:{value}")
}

fn command(
    binding: ProjectCommandChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
    expected_revision: u64,
    availability: AssistanceAvailability,
    bytes: &[u8],
) -> CommandCall<UpdateProjectAssistanceInput> {
    command_call(
        binding,
        nonce_digest,
        ids_suffix,
        bytes,
        UpdateProjectAssistanceInput {
            availability,
            expected_revision,
            destination: Some(storyos_core::DeploymentDestination::HostFake),
        },
    )
}

async fn update_project_assistance(
    store: &PostgresProjectReader,
    call: &CommandCall<UpdateProjectAssistanceInput>,
) -> Result<UpdateProjectAssistanceSettlement, ProjectCommandError> {
    store
        .update_project_assistance(&call.envelope, &call.input)
        .await
}

/// The applied assistance change of one settlement. Any other outcome panics.
fn applied(settlement: &UpdateProjectAssistanceSettlement) -> UpdateProjectAssistanceApplied {
    match &settlement.outcome {
        TransitionOutcome::Applied(applied) => applied.effect,
        other => panic!("the assistance change must apply, got {other:?}"),
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn update_project_assistance_initializes_toggles_and_stays_scope_safe() {
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
    admin
        .execute(
            "INSERT INTO storyos.projects (owner_user_id, project_id, title, current_chapter_id)
             VALUES ($1::text::uuid, $2::text::uuid, 'Other Novel', NULL)",
            &[&USER_A, &OTHER_PROJECT],
        )
        .await
        .unwrap();
    let store = PostgresProjectReader::new(runtime_url.clone());
    let scope = ProjectScope::new(UserId::new(USER_A), ProjectId::new(PROJECT));
    assert_eq!(open_project_assistance(&store, &scope).await.unwrap(), None);

    let absent_issue = command_issue(
        &project_scope(),
        "0a0b",
        &UPDATE_PROJECT_ASSISTANCE,
        &digest(UNAVAILABLE_BYTES),
    );
    issue_project_command_challenge(&store, &absent_issue)
        .await
        .unwrap();
    let absent = update_project_assistance(
        &store,
        &command(
            absent_issue.binding.clone(),
            &absent_issue.nonce_digest,
            "0a0c",
            1,
            AssistanceAvailability::Unavailable,
            UNAVAILABLE_BYTES,
        ),
    )
    .await
    .unwrap();
    assert_eq!(absent.response.assistance, None);

    let first_issue = command_issue(
        &project_scope(),
        "0a01",
        &UPDATE_PROJECT_ASSISTANCE,
        &digest(AVAILABLE_BYTES),
    );
    issue_project_command_challenge(&store, &first_issue)
        .await
        .unwrap();
    let first = update_project_assistance(
        &store,
        &command(
            first_issue.binding.clone(),
            &first_issue.nonce_digest,
            "0a02",
            0,
            AssistanceAvailability::Available,
            AVAILABLE_BYTES,
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        applied(&first),
        UpdateProjectAssistanceApplied::Initialized {
            availability: AssistanceAvailability::Available,
            revision: 1,
        }
    );
    let first_binding = first
        .response
        .assistance
        .clone()
        .expect("initialized binding");
    assert_eq!(
        first_binding.model_registration_revision,
        HOST_FAKE_REGISTRATION.revision
    );
    assert_eq!(
        open_project_assistance(&store, &scope).await.unwrap(),
        Some(first_binding.clone())
    );

    let same_issue = command_issue(
        &project_scope(),
        "0a03",
        &UPDATE_PROJECT_ASSISTANCE,
        &digest(AVAILABLE_BYTES),
    );
    issue_project_command_challenge(&store, &same_issue)
        .await
        .unwrap();
    let same = update_project_assistance(
        &store,
        &command(
            same_issue.binding.clone(),
            &same_issue.nonce_digest,
            "0a04",
            1,
            AssistanceAvailability::Available,
            AVAILABLE_BYTES,
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        same.outcome,
        TransitionOutcome::NoEffect(UpdateProjectAssistanceNoEffect::AvailabilityUnchanged)
    );
    assert_eq!(same.response.assistance.as_ref(), Some(&first_binding));

    let stale_issue = command_issue(
        &project_scope(),
        "0a05",
        &UPDATE_PROJECT_ASSISTANCE,
        &digest(UNAVAILABLE_BYTES),
    );
    issue_project_command_challenge(&store, &stale_issue)
        .await
        .unwrap();
    let stale = update_project_assistance(
        &store,
        &command(
            stale_issue.binding.clone(),
            &stale_issue.nonce_digest,
            "0a06",
            0,
            AssistanceAvailability::Unavailable,
            UNAVAILABLE_BYTES,
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        stale.outcome,
        TransitionOutcome::Conflicted(UpdateProjectAssistanceConflict::StaleAssistanceRevision)
    );
    assert_eq!(stale.response.assistance.as_ref(), Some(&first_binding));

    let toggle_issue = command_issue(
        &project_scope(),
        "0a07",
        &UPDATE_PROJECT_ASSISTANCE,
        &digest(UNAVAILABLE_BYTES),
    );
    issue_project_command_challenge(&store, &toggle_issue)
        .await
        .unwrap();
    let toggle = update_project_assistance(
        &store,
        &command(
            toggle_issue.binding.clone(),
            &toggle_issue.nonce_digest,
            "0a08",
            1,
            AssistanceAvailability::Unavailable,
            UNAVAILABLE_BYTES,
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        applied(&toggle),
        UpdateProjectAssistanceApplied::Changed {
            availability: AssistanceAvailability::Unavailable,
            revision: 2,
        }
    );
    let toggled = toggle.response.assistance.expect("toggled binding");
    assert_eq!(
        (
            toggled.availability,
            toggled.revision,
            toggled.model_registration_revision.as_str(),
            toggled.processing_destination_identity.as_str(),
            toggled.processing_destination_identity_evidence_revision,
            toggled.project_model_use_binding_revision.as_str(),
            toggled.external_compatibility_decision.as_str(),
        ),
        (
            AssistanceAvailability::Unavailable,
            2,
            first_binding.model_registration_revision.as_str(),
            first_binding.processing_destination_identity.as_str(),
            first_binding.processing_destination_identity_evidence_revision,
            first_binding.project_model_use_binding_revision.as_str(),
            first_binding.external_compatibility_decision.as_str(),
        )
    );

    let other_scope = ProjectScope::new(UserId::new(USER_A), ProjectId::new(OTHER_PROJECT));
    assert_eq!(
        open_project_assistance(&store, &other_scope).await.unwrap(),
        None
    );

    let counts = admin
        .query_one(
            "SELECT
                (SELECT count(*) FROM storyos.model_registration_revisions
                  WHERE model_registration_revision = $2::text::uuid),
                (SELECT count(*) FROM storyos.processing_destination_identities
                  WHERE project_id = $1::text::uuid),
                (SELECT count(*) FROM storyos.project_destination_grants
                  WHERE project_id = $1::text::uuid),
                (SELECT count(*) FROM storyos.project_policy_revisions
                  WHERE project_id = $1::text::uuid)",
            &[&PROJECT, &HOST_FAKE_REGISTRATION.revision],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            counts.get::<_, i64>(0),
            counts.get::<_, i64>(1),
            counts.get::<_, i64>(2),
            counts.get::<_, i64>(3)
        ),
        (1, 1, 1, 2)
    );

    let (mut runtime, connection) = tokio_postgres::connect(&runtime_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        connection.await.unwrap();
    });
    let foreign = runtime.transaction().await.unwrap();
    foreign
        .execute(
            "SELECT set_config('storyos.user_id', $1, true),
                    set_config('storyos.owner_user_id', $1, true),
                    set_config('storyos.project_id', $2, true)",
            &[&USER_B, &PROJECT],
        )
        .await
        .unwrap();
    let hidden = foreign
        .query_one("SELECT count(*) FROM storyos.project_policy_revisions", &[])
        .await
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(hidden, 0);
}
