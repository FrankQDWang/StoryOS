use super::*;
use storyos_application::{
    AgentRunReadSelection, ConversationSelection, CreateAgentRunCommandError, CreateAgentRunInput,
    CreateAgentRunSettlement, IssueProjectCommandChallenge, ProjectId, ProjectScope,
    RefusableCommandError, UserId, inspect_agent_run, issue_project_command_challenge,
};
use storyos_core::CreateAgentRunRefusal;

use std::time::Duration;

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::CommandCall;
use crate::shared_tests::create_agent_run::{
    CHAPTER, USER_A, admitted, connect_admin, issue_run, prepare_project, refused, run_command,
};
use tokio_postgres::NoTls;

use crate::statement_cache::Client;

#[path = "create_agent_run_skill_tests.rs"]
pub(crate) mod skill_tests;

const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const PROJECT: &str = "018f0000-0000-7001-8000-0000000007a2";
const OTHER_PROJECT: &str = "018f0000-0000-7001-8000-0000000007a3";
const RACE_PROJECT: &str = "018f0000-0000-7001-8000-000000000b82";
const RACE_CHAPTER: &str = "018f0000-0000-7001-8000-000000000b8c";

async fn create(
    store: &PostgresProjectReader,
    call: CommandCall<CreateAgentRunInput>,
) -> Result<CreateAgentRunSettlement, CreateAgentRunCommandError> {
    store.create_agent_run(&call.envelope, &call.input).await
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

fn named_store(database_url: &str, application_name: &str) -> PostgresProjectReader {
    let separator = if database_url.contains('?') { '&' } else { '?' };
    PostgresProjectReader::new(format!(
        "{database_url}{separator}application_name={application_name}"
    ))
}

/// Waits until the backend of `application_name` waits for a row lock of `holder` only on a row of
/// `relation`, and returns the backend process ID. A serializable predicate lock does not count.
async fn wait_for_row_lock_of(
    monitor: &Client,
    application_name: &str,
    holder: i32,
    relation: &str,
) -> i32 {
    let waited = tokio::time::timeout(Duration::from_secs(/*secs*/ 10), async {
        loop {
            let waiting = monitor
                .query_opt(
                    "SELECT activity.pid FROM pg_catalog.pg_stat_activity AS activity
                      WHERE activity.application_name = $1 AND activity.wait_event_type = 'Lock'
                        AND pg_catalog.pg_blocking_pids(activity.pid) = ARRAY[$2::int]
                        AND EXISTS (
                          SELECT 1 FROM pg_catalog.pg_locks AS lock
                           WHERE lock.pid = activity.pid AND lock.locktype = 'tuple'
                             AND lock.mode <> 'SIReadLock'
                             AND lock.relation = $3::text::regclass)",
                    &[&application_name, &holder, &relation],
                )
                .await
                .unwrap();
            if let Some(row) = waiting {
                return row.get::<_, i32>(/*idx*/ 0);
            }
            tokio::task::yield_now().await;
        }
    })
    .await;
    if let Ok(pid) = waited {
        return pid;
    }
    let observations = monitor
        .query(
            "SELECT state, wait_event_type, wait_event, left(query, 160),
                    pg_catalog.pg_blocking_pids(pid)
               FROM pg_catalog.pg_stat_activity WHERE application_name = $1",
            &[&application_name],
        )
        .await
        .unwrap()
        .into_iter()
        .map(|row| {
            (
                row.get::<_, String>(/*idx*/ 0),
                row.get::<_, Option<String>>(/*idx*/ 1),
                row.get::<_, Option<String>>(/*idx*/ 2),
                row.get::<_, String>(/*idx*/ 3),
                row.get::<_, Vec<i32>>(/*idx*/ 4),
            )
        })
        .collect::<Vec<_>>();
    panic!("{application_name} did not wait for backend {holder} on {relation}: {observations:?}");
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn create_agent_run_admits_one_conversation_and_stays_scope_safe() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
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
    let opened = inspect_agent_run(
        &store,
        &scope,
        &first_run.run_id,
        &AgentRunReadSelection::Current,
    )
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

    let hidden = inspect_agent_run(
        &store,
        &ProjectScope::new(UserId::new(USER_B), ProjectId::new(PROJECT)),
        &first_run.run_id,
        &AgentRunReadSelection::Current,
    )
    .await
    .unwrap();
    assert_eq!(hidden, None);
}

/// Two admissions to one idle Conversation race. The second waits for the Project lock of the
/// first, so its snapshot does not show the first Run. The contention refusal refuses it.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn concurrent_existing_conversation_admissions_take_the_contention_refusal() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (runtime_url, mut admin) = connect_admin().await;
    let store = PostgresProjectReader::new(runtime_url.clone());
    prepare_project(&mut admin, &store, RACE_PROJECT, RACE_CHAPTER, "b80").await;
    let opening_issue = issue_run(RACE_PROJECT, "b811");
    issue_project_command_challenge(&store, &opening_issue)
        .await
        .unwrap();
    let opening = create(
        &store,
        run_command(
            opening_issue.binding.clone(),
            &opening_issue.nonce_digest,
            "b812",
            ConversationSelection::New,
            "018f0000-0000-7001-8000-00000006b812",
            RACE_CHAPTER,
        ),
    )
    .await
    .unwrap();
    let conversation_id = admitted(&opening).conversation_id;
    delete_run(&admin, RACE_PROJECT, &admitted(&opening).run_id).await;
    let mut calls = Vec::new();
    for (issue_suffix, ids_suffix) in [("b821", "b822"), ("b831", "b832")] {
        let issue = issue_run(RACE_PROJECT, issue_suffix);
        issue_project_command_challenge(&store, &issue)
            .await
            .unwrap();
        calls.push(run_command(
            issue.binding.clone(),
            &issue.nonce_digest,
            ids_suffix,
            ConversationSelection::Existing {
                conversation_id: conversation_id.clone(),
            },
            &conversation_id,
            RACE_CHAPTER,
        ));
    }
    let second_call = calls.pop().expect("two admissions");
    let first_call = calls.pop().expect("two admissions");

    // The gate holds the first admission at its Run insert, after its classification and
    // while it keeps the Project lock.
    let (_, gate) = connect_admin().await;
    gate.batch_execute("BEGIN").await.unwrap();
    gate.execute(
        "SELECT 1 FROM storyos.conversation_memory_settings
          WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
            AND conversation_id = $3::text::uuid
            FOR UPDATE",
        &[&USER_A, &RACE_PROJECT, &conversation_id],
    )
    .await
    .unwrap();
    let gate_pid: i32 = gate
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let first_store = named_store(&runtime_url, "storyos_race_first_admission");
    let second_store = named_store(&runtime_url, "storyos_race_second_admission");
    let second_after_first_waits = async {
        let first_pid = wait_for_row_lock_of(
            &admin,
            "storyos_race_first_admission",
            gate_pid,
            "storyos.conversation_memory_settings",
        )
        .await;
        let open_gate = async {
            wait_for_row_lock_of(
                &admin,
                "storyos_race_second_admission",
                first_pid,
                "storyos.projects",
            )
            .await;
            gate.batch_execute("ROLLBACK").await.unwrap();
        };
        tokio::join!(create(&second_store, second_call), open_gate).0
    };
    let (first, second) = tokio::join!(create(&first_store, first_call), second_after_first_waits);

    let first = admitted(&first.unwrap());
    assert_eq!(refused(second), CreateAgentRunRefusal::ConversationBusy);
    let conversations = admin
        .query_one(
            "SELECT count(*) FROM storyos.project_conversations
              WHERE project_id = $1::text::uuid",
            &[&RACE_PROJECT],
        )
        .await
        .unwrap()
        .get::<_, i64>(/*idx*/ 0);
    let runs = admin
        .query(
            "SELECT conversation_id::text, run_id::text, status FROM storyos.agent_runs
              WHERE project_id = $1::text::uuid",
            &[&RACE_PROJECT],
        )
        .await
        .unwrap()
        .into_iter()
        .map(|row| (row.get(/*idx*/ 0), row.get(/*idx*/ 1), row.get(/*idx*/ 2)))
        .collect::<Vec<(String, String, String)>>();
    assert_eq!(
        (conversations, runs),
        (
            1,
            vec![(conversation_id, first.run_id, "queued".to_owned())]
        )
    );
}
