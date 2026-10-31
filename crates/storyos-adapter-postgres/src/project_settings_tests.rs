use std::convert::Infallible;

use storyos_application::{
    ActivityApplied, ProjectAssistanceRecord, ProjectCommandError, ProjectCommandSettlement,
    ProjectScope, UpdateProjectAssistanceInput, UpdateProjectInput, open_project_assistance,
};
use storyos_core::{
    AssistanceAvailability, HOST_FAKE_REGISTRATION, ReasonCode, TransitionOutcome,
    UpdateProjectApplied, UpdateProjectAssistanceApplied, UpdateProjectAssistanceConflict,
    UpdateProjectAssistanceNoEffect, UpdateProjectConflict,
};
use tokio_postgres::NoTls;

use crate::PostgresProjectReader;
use crate::command_sequence::tests::{
    CommandCall, Route, UPDATE_PROJECT, UPDATE_PROJECT_ASSISTANCE, issued, seed_project, stores,
};

const OTHER_USER: &str = "018f0000-0000-7001-8000-000000000101";

/// Settles each input in order in the Project of `scope`. The Command Challenge of input `n` has
/// the suffix `first_suffix + n`.
async fn settle_each<I, S>(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    route: &Route,
    first_suffix: u16,
    inputs: Vec<I>,
    settle: impl AsyncFn(&PostgresProjectReader, &CommandCall<I>) -> Result<S, ProjectCommandError>,
) -> Vec<S> {
    let mut settled = Vec::new();
    for (suffix, input) in (first_suffix..).zip(inputs) {
        let call = issued(store, scope, suffix, route, input).await;
        settled.push(settle(store, &call).await.unwrap());
    }
    settled
}

/// The outcome of an `ActivityOnly` settlement with its applied effect.
fn outcome<A: Clone, N: ReasonCode + Clone, C: ReasonCode + Clone, P, Z>(
    settlement: &ProjectCommandSettlement<ActivityApplied<A>, N, C, Infallible, P, Z>,
) -> TransitionOutcome<A, N, C, Infallible> {
    settlement
        .outcome
        .clone()
        .map_applied(|applied| applied.effect)
}

/// Counts the rows of `table` in the Project of `scope` that another User can read.
async fn rows_for_other_user(scope: &ProjectScope, table: &str) -> i64 {
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let (mut runtime, connection) = tokio_postgres::connect(&runtime_url, NoTls).await.unwrap();
    tokio::spawn(connection);
    let foreign = runtime.transaction().await.unwrap();
    foreign
        .execute(
            "SELECT set_config('storyos.user_id', $1, true),
                    set_config('storyos.owner_user_id', $1, true),
                    set_config('storyos.project_id', $2, true)",
            &[&OTHER_USER, &scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    foreign
        .query_one(
            &format!("SELECT count(*) FROM storyos.{table} WHERE project_id = $1::text::uuid"),
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn update_project_renames_only_its_own_project() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let scope = seed_project(&store, "9800").await;
    let settled = settle_each(
        &store,
        &scope,
        &UPDATE_PROJECT,
        /*first_suffix*/ 0x9801,
        [
            ("Renamed Novel", 1),
            ("Renamed Novel", 1),
            ("Later Novel", 2),
        ]
        .map(|(title, expected_revision)| UpdateProjectInput {
            title: title.to_owned(),
            expected_revision,
        })
        .to_vec(),
        async |store: &PostgresProjectReader, call: &CommandCall<UpdateProjectInput>| {
            store.update_project(&call.envelope, &call.input).await
        },
    )
    .await;
    assert_eq!(
        settled.iter().map(outcome).collect::<Vec<_>>(),
        vec![
            TransitionOutcome::Applied(UpdateProjectApplied {
                title: "Renamed Novel".to_owned(),
                revision: 2,
            }),
            TransitionOutcome::Conflicted(UpdateProjectConflict::StaleProjectRevision),
            TransitionOutcome::Applied(UpdateProjectApplied {
                title: "Later Novel".to_owned(),
                revision: 3,
            }),
        ]
    );
    let row = admin
        .query_one(
            "SELECT title, revision::text FROM storyos.projects WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    assert_eq!(
        (row.get::<_, String>(0), row.get::<_, String>(1)),
        ("Later Novel".to_owned(), "3".to_owned())
    );
    assert_eq!(rows_for_other_user(&scope, "projects").await, 0);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn update_project_assistance_initializes_and_toggles_only_its_own_binding() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let scope = seed_project(&store, "9820").await;
    let other_scope = seed_project(&store, "9830").await;
    assert_eq!(open_project_assistance(&store, &scope).await.unwrap(), None);
    let settled = settle_each(
        &store,
        &scope,
        &UPDATE_PROJECT_ASSISTANCE,
        /*first_suffix*/ 0x9821,
        [
            (AssistanceAvailability::Unavailable, 1),
            (AssistanceAvailability::Available, 0),
            (AssistanceAvailability::Available, 1),
            (AssistanceAvailability::Unavailable, 0),
            (AssistanceAvailability::Unavailable, 1),
        ]
        .map(
            |(availability, expected_revision)| UpdateProjectAssistanceInput {
                availability,
                expected_revision,
                destination: Some(storyos_core::DeploymentDestination::HostFake),
            },
        )
        .to_vec(),
        async |store: &PostgresProjectReader, call: &CommandCall<UpdateProjectAssistanceInput>| {
            store
                .update_project_assistance(&call.envelope, &call.input)
                .await
        },
    )
    .await;
    assert_eq!(
        settled.iter().map(outcome).collect::<Vec<_>>(),
        vec![
            TransitionOutcome::Conflicted(UpdateProjectAssistanceConflict::StaleAssistanceRevision),
            TransitionOutcome::Applied(UpdateProjectAssistanceApplied::Initialized {
                availability: AssistanceAvailability::Available,
                revision: 1,
            }),
            TransitionOutcome::NoEffect(UpdateProjectAssistanceNoEffect::AvailabilityUnchanged),
            TransitionOutcome::Conflicted(UpdateProjectAssistanceConflict::StaleAssistanceRevision),
            TransitionOutcome::Applied(UpdateProjectAssistanceApplied::Changed {
                availability: AssistanceAvailability::Unavailable,
                revision: 2,
            }),
        ]
    );
    let initialized = settled[1]
        .response
        .assistance
        .clone()
        .expect("the initialized binding");
    assert_eq!(
        initialized.model_registration_revision,
        HOST_FAKE_REGISTRATION.revision
    );
    let toggled = settled[4]
        .response
        .assistance
        .clone()
        .expect("the toggled binding");
    assert_eq!(
        settled
            .iter()
            .map(|settlement| settlement.response.assistance.clone())
            .collect::<Vec<_>>(),
        vec![
            None,
            Some(initialized.clone()),
            Some(initialized.clone()),
            Some(initialized.clone()),
            Some(ProjectAssistanceRecord {
                availability: AssistanceAvailability::Unavailable,
                revision: 2,
                grant_id: toggled.grant_id.clone(),
                ..initialized
            }),
        ]
    );
    assert_eq!(
        open_project_assistance(&store, &scope).await.unwrap(),
        Some(toggled)
    );
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
            &[&scope.project_id.as_ref(), &HOST_FAKE_REGISTRATION.revision],
        )
        .await
        .unwrap();
    assert_eq!(
        [0, 1, 2, 3].map(|index| counts.get::<_, i64>(index)),
        [1, 1, 1, 2]
    );
    assert_eq!(
        rows_for_other_user(&scope, "project_policy_revisions").await,
        0
    );
}
