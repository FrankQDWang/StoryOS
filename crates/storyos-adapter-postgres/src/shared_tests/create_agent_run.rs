//! The shared test support from `create_agent_run_tests.rs`.

use crate::shared_tests::command_sequence_agent_run::CREATE_AGENT_RUN;
use crate::shared_tests::command_sequence_project_session::UPDATE_PROJECT_ASSISTANCE;
use crate::shared_tests::command_sequence_support::{CommandCall, command_call, command_issue};
use crate::*;
use storyos_application::{
    ConversationSelection, CreateAgentRunApplied, CreateAgentRunCommandError, CreateAgentRunInput,
    CreateAgentRunSettlement, IssueProjectCommandChallenge, ProjectCommandChallengeBinding,
    ProjectId, ProjectScope, RefusableCommandError, UpdateProjectAssistanceInput, UserId,
    issue_project_command_challenge,
};
use storyos_core::{AssistanceAvailability, CreateAgentRunRefusal, TransitionOutcome};
use tokio_postgres::{Client, NoTls};

pub(crate) const USER_A: &str = "018f0000-0000-7001-8000-000000000001";

pub(crate) const CHAPTER: &str = "018f0000-0000-7001-8000-0000000007c1";

pub(crate) const AVAILABLE_BYTES: &[u8] = br#"{"availability":"available"}"#;

pub(crate) const RUN_BYTES: &[u8] = br#"{"conversation":{"kind":"new"}}"#;

pub(crate) fn digest(profile: &str, bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    let value = Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut value, byte| {
            use std::fmt::Write as _;
            write!(value, "{byte:02x}").expect("writing to String cannot fail");
            value
        });
    format!("sha256:{profile}:{value}")
}

pub(crate) fn issue_assistance(
    project: &str,
    idempotency_suffix: &str,
) -> IssueProjectCommandChallenge {
    command_issue(
        &ProjectScope::new(UserId::new(USER_A), ProjectId::new(project)),
        idempotency_suffix,
        &UPDATE_PROJECT_ASSISTANCE,
        &digest(
            "storyos.command.updateProjectAssistance.jcs.v1",
            AVAILABLE_BYTES,
        ),
    )
}

pub(crate) fn issue_run(project: &str, idempotency_suffix: &str) -> IssueProjectCommandChallenge {
    command_issue(
        &ProjectScope::new(UserId::new(USER_A), ProjectId::new(project)),
        idempotency_suffix,
        &CREATE_AGENT_RUN,
        &digest("storyos.command.createAgentRun.jcs.v1", RUN_BYTES),
    )
}

pub(crate) fn run_command(
    binding: ProjectCommandChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
    conversation: ConversationSelection,
    conversation_id: &str,
    chapter_id: &str,
) -> CommandCall<CreateAgentRunInput> {
    command_call(
        binding,
        nonce_digest,
        ids_suffix,
        RUN_BYTES,
        CreateAgentRunInput {
            passage_targets: None,
            selection_references: None,
            conversation,
            author_message: "Help with this passage.".to_owned(),
            chapter_id: chapter_id.to_owned(),
            run_id: format!("018f0000-0000-7001-8000-00000004{ids_suffix}"),
            conversation_id: conversation_id.to_owned(),
            project_agent_id: format!("018f0000-0000-7001-8000-00000005{ids_suffix}"),
            destination: Some(storyos_core::DeploymentDestination::HostFakeToolLoop),
            research: None,
            embedding: None,
            skills: Default::default(),
            skill_selections: Vec::new(),
        },
    )
}

pub(crate) fn admitted(settlement: &CreateAgentRunSettlement) -> CreateAgentRunApplied {
    match &settlement.outcome {
        TransitionOutcome::Applied(applied) => applied.effect.clone(),
        TransitionOutcome::NoEffect(reason) => match *reason {},
        TransitionOutcome::Conflicted(reason) => match *reason {},
        TransitionOutcome::Refused(reason) => match *reason {},
    }
}

pub(crate) fn refused(
    result: Result<CreateAgentRunSettlement, CreateAgentRunCommandError>,
) -> CreateAgentRunRefusal {
    match result {
        Err(RefusableCommandError::RefusedBeforeAdmission(refusal)) => refusal,
        other => panic!("the createAgentRun must refuse before its Admission, got {other:?}"),
    }
}

pub(crate) async fn connect_admin() -> (String, Client) {
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let (admin, admin_connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        admin_connection.await.unwrap();
    });
    (runtime_url, admin)
}

/// Seeds one Project with its current Chapter and makes its assistance available.
pub(crate) async fn prepare_project(
    admin: &mut Client,
    store: &PostgresProjectReader,
    project: &str,
    chapter: &str,
    suffix_lead: &str,
) {
    let setup = admin.transaction().await.unwrap();
    setup
        .batch_execute("SET CONSTRAINTS ALL DEFERRED")
        .await
        .unwrap();
    setup
        .execute(
            "INSERT INTO storyos.projects
               (owner_user_id, project_id, title, current_chapter_id)
             VALUES ($1::text::uuid, $2::text::uuid, 'Assistance Novel', $3::text::uuid)",
            &[&USER_A, &project, &chapter],
        )
        .await
        .unwrap();
    setup
        .execute(
            "INSERT INTO storyos.manuscript_objects
               (owner_user_id, project_id, manuscript_object_id, object_kind, title)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid,
                     'chapter', 'Assistance Chapter')",
            &[&USER_A, &project, &chapter],
        )
        .await
        .unwrap();
    setup.commit().await.unwrap();
    let assistance_issue = issue_assistance(project, &format!("{suffix_lead}1"));
    issue_project_command_challenge(store, &assistance_issue)
        .await
        .unwrap();
    let assistance_call = command_call(
        assistance_issue.binding.clone(),
        &assistance_issue.nonce_digest,
        &format!("{suffix_lead}2"),
        AVAILABLE_BYTES,
        UpdateProjectAssistanceInput {
            availability: AssistanceAvailability::Available,
            expected_revision: 0,
            destination: Some(storyos_core::DeploymentDestination::HostFakeToolLoop),
        },
    );
    store
        .update_project_assistance(&assistance_call.envelope, &assistance_call.input)
        .await
        .unwrap();
}
