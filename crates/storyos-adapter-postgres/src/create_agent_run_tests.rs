use super::*;
use storyos_application::{
    ConversationSelection, CreateAgentRunApplied, CreateAgentRunCommandError, CreateAgentRunInput,
    CreateAgentRunSettlement, IssueProjectCommandChallenge, ProjectCommandChallengeBinding,
    ProjectId, ProjectScope, RefusableCommandError, UpdateProjectAssistanceInput, UserId,
    issue_project_command_challenge, open_agent_run,
};
use storyos_core::{AssistanceAvailability, CreateAgentRunRefusal, TransitionOutcome};

use crate::command_sequence::tests::{
    CREATE_AGENT_RUN, CommandCall, UPDATE_PROJECT_ASSISTANCE, command_call, command_issue,
};
use tokio_postgres::{Client, NoTls};

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const PROJECT: &str = "018f0000-0000-7001-8000-0000000007a2";
const OTHER_PROJECT: &str = "018f0000-0000-7001-8000-0000000007a3";
const CHAPTER: &str = "018f0000-0000-7001-8000-0000000007c1";
const AVAILABLE_BYTES: &[u8] = br#"{"availability":"available"}"#;
const RUN_BYTES: &[u8] = br#"{"conversation":{"kind":"new"}}"#;

fn digest(profile: &str, bytes: &[u8]) -> String {
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

fn issue_assistance(project: &str, idempotency_suffix: &str) -> IssueProjectCommandChallenge {
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

fn issue_run(project: &str, idempotency_suffix: &str) -> IssueProjectCommandChallenge {
    command_issue(
        &ProjectScope::new(UserId::new(USER_A), ProjectId::new(project)),
        idempotency_suffix,
        &CREATE_AGENT_RUN,
        &digest("storyos.command.createAgentRun.jcs.v1", RUN_BYTES),
    )
}

fn run_command(
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
            destination: Some(storyos_core::DeploymentDestination::HostFake),
        },
    )
}

async fn create(
    store: &PostgresProjectReader,
    call: CommandCall<CreateAgentRunInput>,
) -> Result<CreateAgentRunSettlement, CreateAgentRunCommandError> {
    store.create_agent_run(&call.envelope, &call.input).await
}

fn admitted(settlement: &CreateAgentRunSettlement) -> CreateAgentRunApplied {
    match &settlement.outcome {
        TransitionOutcome::Applied(applied) => applied.effect.clone(),
        TransitionOutcome::NoEffect(reason) => match *reason {},
        TransitionOutcome::Conflicted(reason) => match *reason {},
        TransitionOutcome::Refused(reason) => match *reason {},
    }
}

fn refused(
    result: Result<CreateAgentRunSettlement, CreateAgentRunCommandError>,
) -> CreateAgentRunRefusal {
    match result {
        Err(RefusableCommandError::RefusedBeforeAdmission(refusal)) => refusal,
        other => panic!("the createAgentRun must refuse before its Admission, got {other:?}"),
    }
}

async fn connect_admin() -> (String, Client) {
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
async fn prepare_project(
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
            destination: Some(storyos_core::DeploymentDestination::HostFake),
        },
    );
    store
        .update_project_assistance(&assistance_call.envelope, &assistance_call.input)
        .await
        .unwrap();
}

/// Removes one Run and its rows, so that its Conversation is idle again.
async fn delete_run(admin: &Client, project: &str, run_id: &str) {
    for table in [
        "model_attempts",
        "context_assembly_manifests",
        "operation_requirements",
        "agent_runs",
    ] {
        admin
            .execute(
                &format!(
                    "DELETE FROM storyos.{table}
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                        AND run_id = $3::text::uuid"
                ),
                &[&USER_A, &project, &run_id],
            )
            .await
            .unwrap();
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn create_agent_run_admits_one_conversation_and_stays_scope_safe() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (runtime_url, mut admin) = connect_admin().await;
    let store = PostgresProjectReader::new(runtime_url.clone());
    prepare_project(&mut admin, &store, PROJECT, CHAPTER, "7a0").await;
    admin
        .execute(
            "INSERT INTO storyos.projects (owner_user_id, project_id, title, current_chapter_id)
             VALUES ($1::text::uuid, $2::text::uuid, 'Other Novel', NULL)",
            &[&USER_A, &OTHER_PROJECT],
        )
        .await
        .unwrap();
    let scope = ProjectScope::new(UserId::new(USER_A), ProjectId::new(PROJECT));

    let first_issue = issue_run(PROJECT, "7a11");
    issue_project_command_challenge(&store, &first_issue)
        .await
        .unwrap();
    let first = create(
        &store,
        run_command(
            first_issue.binding.clone(),
            &first_issue.nonce_digest,
            "7a12",
            ConversationSelection::New,
            "018f0000-0000-7001-8000-000000067a12",
            CHAPTER,
        ),
    )
    .await
    .unwrap();
    let first_run = admitted(&first);
    assert_eq!(
        uuid::Uuid::parse_str(&first_run.memory_settings_revision)
            .expect("settings revision is a UUID")
            .get_version_num(),
        7
    );
    assert_eq!(
        first_run.conversation_id,
        "018f0000-0000-7001-8000-000000067a12"
    );
    let opened = open_agent_run(&store, &scope, &first_run.run_id)
        .await
        .unwrap()
        .expect("admitted run");
    assert_eq!(opened.conversation_id, first_run.conversation_id);
    assert_eq!(
        opened.memory_settings_revision,
        first_run.memory_settings_revision
    );
    assert_eq!(
        opened.context.record.sufficiency,
        storyos_core::ContextSufficiency::Blocked {
            reasons: vec![storyos_core::ContextBlockReason::WorkingTargetRevisionUnavailable],
        }
    );
    assert_eq!(opened.context.destination_context_manifest_id, None);
    assert_eq!(opened.context.outbound_disclosure_manifest_id, None);
    assert_eq!(
        opened.context.working_target_availability,
        storyos_application::WorkingTargetAvailability::Unavailable
    );
    assert_eq!(
        opened.context.record.destination_io,
        storyos_core::DestinationIo::None
    );
    let grant_id = admin
        .query_one(
            "SELECT run.grant_id::text
               FROM storyos.agent_runs AS run
               JOIN storyos.project_destination_grants AS destination_grant
                 ON (destination_grant.owner_user_id, destination_grant.project_id,
                     destination_grant.grant_id) =
                    (run.owner_user_id, run.project_id, run.grant_id)
              WHERE run.owner_user_id = $1::text::uuid
                AND run.project_id = $2::text::uuid
                AND run.run_id = $3::text::uuid",
            &[&USER_A, &PROJECT, &first_run.run_id],
        )
        .await
        .unwrap()
        .get::<_, String>(0);
    assert_eq!(
        uuid::Uuid::parse_str(&grant_id)
            .expect("grant identity is a UUID")
            .get_version_num(),
        7
    );
    let settings = admin
        .query_one(
            "SELECT use_enabled, contribution_enabled
               FROM storyos.conversation_memory_settings
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND conversation_id = $3::text::uuid",
            &[&USER_A, &PROJECT, &first_run.conversation_id],
        )
        .await
        .unwrap();
    assert_eq!(
        (settings.get::<_, bool>(0), settings.get::<_, bool>(1)),
        (true, true)
    );

    let busy_issue = issue_run(PROJECT, "7a21");
    issue_project_command_challenge(&store, &busy_issue)
        .await
        .unwrap();
    let busy = create(
        &store,
        run_command(
            busy_issue.binding.clone(),
            &busy_issue.nonce_digest,
            "7a22",
            ConversationSelection::Existing {
                conversation_id: first_run.conversation_id.clone(),
            },
            &first_run.conversation_id,
            CHAPTER,
        ),
    )
    .await;
    // The ordinary busy classification refuses an admission that reads the queued Run.
    assert_eq!(refused(busy), CreateAgentRunRefusal::ConversationBusy);

    delete_run(&admin, PROJECT, &first_run.run_id).await;
    let reopen_issue = issue_run(PROJECT, "7a25");
    issue_project_command_challenge(&store, &reopen_issue)
        .await
        .unwrap();
    let reopened = create(
        &store,
        run_command(
            reopen_issue.binding.clone(),
            &reopen_issue.nonce_digest,
            "7a26",
            ConversationSelection::Existing {
                conversation_id: first_run.conversation_id.clone(),
            },
            &first_run.conversation_id,
            CHAPTER,
        ),
    )
    .await
    .unwrap();
    let reopened = admitted(&reopened);
    assert_eq!(reopened.conversation_id, first_run.conversation_id);
    assert_eq!(
        reopened.memory_settings_revision,
        first_run.memory_settings_revision
    );
    assert_ne!(reopened.run_id, first_run.run_id);

    let missing_issue = issue_run(PROJECT, "7a31");
    issue_project_command_challenge(&store, &missing_issue)
        .await
        .unwrap();
    let missing = create(
        &store,
        run_command(
            missing_issue.binding.clone(),
            &missing_issue.nonce_digest,
            "7a32",
            ConversationSelection::Existing {
                conversation_id: "018f0000-0000-7001-8000-00000000dead".to_owned(),
            },
            "018f0000-0000-7001-8000-00000000dead",
            CHAPTER,
        ),
    )
    .await;
    assert_eq!(
        refused(missing),
        CreateAgentRunRefusal::InaccessibleConversation
    );

    let chapter_issue = issue_run(PROJECT, "7a41");
    issue_project_command_challenge(&store, &chapter_issue)
        .await
        .unwrap();
    let chapter = create(
        &store,
        run_command(
            chapter_issue.binding.clone(),
            &chapter_issue.nonce_digest,
            "7a42",
            ConversationSelection::New,
            "018f0000-0000-7001-8000-000000067a42",
            "018f0000-0000-7001-8000-00000000bad1",
        ),
    )
    .await;
    assert_eq!(refused(chapter), CreateAgentRunRefusal::InvalidChapterJoin);

    let conversations = admin
        .query_one(
            "SELECT count(*) FROM storyos.project_conversations
              WHERE project_id = $1::text::uuid",
            &[&PROJECT],
        )
        .await
        .unwrap()
        .get::<_, i64>(0);
    let runs = admin
        .query_one(
            "SELECT count(*) FROM storyos.agent_runs
              WHERE project_id = $1::text::uuid",
            &[&PROJECT],
        )
        .await
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!((conversations, runs), (1, 1));

    let hidden = open_agent_run(
        &store,
        &ProjectScope::new(UserId::new(USER_B), ProjectId::new(PROJECT)),
        &first_run.run_id,
    )
    .await
    .unwrap();
    assert_eq!(hidden, None);
}
