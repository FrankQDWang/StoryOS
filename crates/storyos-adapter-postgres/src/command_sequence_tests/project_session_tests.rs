use storyos_application::EditorSessionId;
use storyos_application::{
    ArchiveProjectInput, ProjectCommandError, UpdateProjectAssistanceInput, UpdateProjectInput,
};
use storyos_application::{TakeOverProjectWriterInput, TakeOverProjectWriterSettlement};
use storyos_core::AssistanceAvailability;

use crate::PostgresProjectReader;
use crate::set_current_chapter_authority_tests::open_session;
use crate::update_volume_tests::seed_project;

use super::damaged_evidence::ReplayError;
use super::support::{
    CommandCall, Route, issued, stores, two_chapter_writer, with_new_request_ids,
};

pub(super) async fn take_over_project_writer(
    store: &PostgresProjectReader,
    call: &CommandCall<TakeOverProjectWriterInput>,
) -> Result<TakeOverProjectWriterSettlement, ProjectCommandError> {
    store
        .take_over_project_writer(&call.envelope, &call.input)
        .await
}

pub(super) const UPDATE_PROJECT_ASSISTANCE: Route = Route {
    kind: "updateProjectAssistance",
    method: storyos_contracts::UPDATE_PROJECT_ASSISTANCE_METHOD,
    path: storyos_contracts::UPDATE_PROJECT_ASSISTANCE_PATH,
    schema: storyos_contracts::UPDATE_PROJECT_ASSISTANCE_REQUEST_SCHEMA_ID,
};

pub(super) const TAKE_OVER_PROJECT_WRITER: Route = Route {
    kind: "takeOverProjectWriter",
    method: storyos_contracts::TAKE_OVER_PROJECT_WRITER_METHOD,
    path: storyos_contracts::TAKE_OVER_PROJECT_WRITER_PATH,
    schema: storyos_contracts::TAKE_OVER_PROJECT_WRITER_REQUEST_SCHEMA_ID,
};

pub(super) const UPDATE_PROJECT: Route = Route {
    kind: "updateProject",
    method: storyos_contracts::UPDATE_PROJECT_METHOD,
    path: storyos_contracts::UPDATE_PROJECT_PATH,
    schema: storyos_contracts::UPDATE_PROJECT_REQUEST_SCHEMA_ID,
};

pub(super) const ARCHIVE_PROJECT: Route = Route {
    kind: "archiveProject",
    method: storyos_contracts::ARCHIVE_PROJECT_METHOD,
    path: storyos_contracts::ARCHIVE_PROJECT_PATH,
    schema: storyos_contracts::ARCHIVE_PROJECT_REQUEST_SCHEMA_ID,
};

/// One applicable writer takeover by a second Editor Session in a new Project.
pub(super) async fn take_over_project_writer_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<TakeOverProjectWriterInput> {
    let (scope, ..) = two_chapter_writer(store, base).await;
    let observer = open_session(store, &scope, &format!("{:04x}", base + 5)).await;
    let input = TakeOverProjectWriterInput {
        editor_session_id: EditorSessionId::new(observer),
        observed_writer_generation: 1,
        editor_contract_revision: storyos_contracts::EDITOR_CONTRACT_REVISION.to_owned(),
    };
    issued(store, &scope, base + 9, &TAKE_OVER_PROJECT_WRITER, input).await
}

/// One applicable first Update Project Assistance in a new Project.
pub(super) async fn update_project_assistance_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<UpdateProjectAssistanceInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let input = UpdateProjectAssistanceInput {
        availability: AssistanceAvailability::Available,
        expected_revision: 0,
        destination: storyos_core::DeploymentDestination::HostFake,
    };
    issued(store, &scope, base + 9, &UPDATE_PROJECT_ASSISTANCE, input).await
}

/// One applicable Update Project in a new Project.
pub(super) async fn update_project_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<UpdateProjectInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let input = UpdateProjectInput {
        title: "Renamed".to_owned(),
        expected_revision: 1,
    };
    issued(store, &scope, base + 9, &UPDATE_PROJECT, input).await
}

/// One applicable Archive Project in a new Project.
pub(super) async fn archive_project_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<ArchiveProjectInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let input = ArchiveProjectInput {
        expected_revision: 1,
    };
    issued(store, &scope, base + 9, &ARCHIVE_PROJECT, input).await
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_command_without_a_response_record_replays_without_it_and_damaged_activity_is_a_fault() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let call = take_over_project_writer_call(&store, /*base*/ 0x6aa0).await;
    let first = take_over_project_writer(&store, &call).await.unwrap();
    let key = &call.envelope.challenge_binding.idempotency_key;
    admin
        .batch_execute(&format!(
            "UPDATE storyos.command_idempotency
                SET acknowledgement_format = NULL, response_project = NULL
              WHERE idempotency_key = '{key}'"
        ))
        .await
        .unwrap();
    let pre_capture = take_over_project_writer(&store, &with_new_request_ids(&call)).await;
    admin
        .batch_execute(&format!(
            "UPDATE storyos.project_activity_event_payloads
                SET payload = jsonb_set(payload, '{{prior_writer_generation}}', '\"damaged\"')
              WHERE receipt_id = '{}'",
            first.ids.receipt_id
        ))
        .await
        .unwrap();
    let damaged = take_over_project_writer(&store, &with_new_request_ids(&call)).await;
    admin
        .batch_execute(&format!(
            "UPDATE storyos.command_idempotency
                SET canonical_command_digest = 'sha256:takeOverProjectWriter:damaged'
              WHERE idempotency_key = '{key}'"
        ))
        .await
        .unwrap();
    let other_digest = take_over_project_writer(&store, &with_new_request_ids(&call)).await;
    assert_eq!(pre_capture.unwrap(), first);
    assert!(matches!(damaged, Err(ProjectCommandError::Unavailable(_))));
    assert!(matches!(
        other_digest,
        Err(ProjectCommandError::BindingConflict)
    ));
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_assistance_replay_separates_a_project_only_record_from_damaged_assistance() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let call = update_project_assistance_call(&store, /*base*/ 0x6ab0).await;
    store
        .update_project_assistance(&call.envelope, &call.input)
        .await
        .unwrap();
    let key = &call.envelope.challenge_binding.idempotency_key;
    let mut errors = Vec::new();
    for evidence in [
        "acknowledgement_format = 'command_response_project.v1', response_assistance = NULL",
        "acknowledgement_format = 'command_response_project_assistance.v1',
         response_assistance = '{}'::jsonb",
    ] {
        admin
            .batch_execute(&format!(
                "UPDATE storyos.command_idempotency SET {evidence}
                  WHERE idempotency_key = '{key}'"
            ))
            .await
            .unwrap();
        let retry = with_new_request_ids(&call);
        errors.push(
            match store
                .update_project_assistance(&retry.envelope, &retry.input)
                .await
            {
                Err(ProjectCommandError::HistoricalAcknowledgementUnavailable) => {
                    ReplayError::HistoricalAcknowledgementUnavailable
                }
                Err(ProjectCommandError::Unavailable(_)) => ReplayError::Unavailable,
                other => panic!("the replay must fail on assistance evidence, got {other:?}"),
            },
        );
    }
    assert_eq!(
        errors,
        vec![
            ReplayError::HistoricalAcknowledgementUnavailable,
            ReplayError::Unavailable,
        ]
    );
}
