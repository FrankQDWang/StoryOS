//! The Conversation list and the Conversation read with real PostgreSQL, and the Provider
//! continuation of each Conversation (ADR 0094 section 10, ADR 0053).

use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    ConversationSelection, NoContractFaults, ProjectConversationReader, ProjectId, ProjectScope,
    UserId, complete_agent_run, list_project_conversations, read_project_conversation,
};
use storyos_contracts::ProjectConversationCompleteness;
use storyos_core::{AgentRunStatus, DeploymentDestination};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{
    RuntimeClock, admit_run_in, claim_run, queued_run_offering, stores,
};

async fn complete(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    run_id: &str,
) {
    let claim = claim_run(admin, scope.clone(), run_id.to_owned()).await;
    complete_agent_run(
        store,
        &FakeDestination,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await
    .unwrap();
}

/// A completed Run in a new Project of `prefix`, and the Project Scope and the Chapter.
async fn first_run(
    store: &PostgresProjectReader,
    admin: &Client,
    prefix: &str,
) -> (ProjectScope, String, String) {
    let (scope, chapter_id, run_id) =
        queued_run_offering(store, prefix, DeploymentDestination::HostFakeToolLoop, None).await;
    complete(store, admin, &scope, &run_id).await;
    (scope, chapter_id, run_id)
}

/// A completed Run with `message` in `conversation` of the Project.
async fn next_run(
    store: &PostgresProjectReader,
    admin: &Client,
    (scope, chapter_id): (&ProjectScope, &str),
    prefix: &str,
    message: &str,
    conversation: ConversationSelection,
) -> String {
    let run_id = admit_run_in(
        store,
        scope,
        chapter_id,
        prefix,
        DeploymentDestination::HostFakeToolLoop,
        message,
        conversation,
        None,
    )
    .await;
    complete(store, admin, scope, &run_id).await;
    run_id
}

async fn conversation_of(admin: &Client, run_id: &str) -> String {
    admin
        .query_one(
            "SELECT conversation_id::text FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// The Conversations A, B, and C of one Project, with the Runs A1, B1, C1, and A2 in this order.
struct ThreeConversations {
    scope: ProjectScope,
    chapter_id: String,
    runs: [String; 4],
    conversations: [String; 3],
}

const MESSAGES: [&str; 4] = [
    "Help with this passage.",
    "Write about the storm.",
    "Name the river.",
    "Keep the voice of the narrator.",
];

async fn three_conversations(
    store: &PostgresProjectReader,
    admin: &Client,
    prefixes: [&str; 4],
) -> ThreeConversations {
    let (scope, chapter_id, first) = first_run(store, admin, prefixes[0]).await;
    let a = conversation_of(admin, &first).await;
    let place = (&scope, chapter_id.as_str());
    let b_run = next_run(
        store,
        admin,
        place,
        prefixes[1],
        MESSAGES[1],
        ConversationSelection::New,
    )
    .await;
    let c_run = next_run(
        store,
        admin,
        place,
        prefixes[2],
        MESSAGES[2],
        ConversationSelection::New,
    )
    .await;
    let second = next_run(
        store,
        admin,
        place,
        prefixes[3],
        MESSAGES[3],
        ConversationSelection::Existing {
            conversation_id: a.clone(),
        },
    )
    .await;
    let conversations = [
        a,
        conversation_of(admin, &b_run).await,
        conversation_of(admin, &c_run).await,
    ];
    ThreeConversations {
        scope,
        chapter_id,
        runs: [first, b_run, c_run, second],
        conversations,
    }
}

async fn run_ids(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    conversation_id: &str,
    limit: usize,
) -> Option<Vec<String>> {
    store
        .read_project_conversation(scope, conversation_id, limit)
        .await
        .unwrap()
        .map(|read| {
            read.newest_first
                .into_iter()
                .map(|run| run.run_id)
                .collect()
        })
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_list_and_the_read_give_the_conversations_of_one_scope_in_order() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let fixture = three_conversations(&store, &admin, ["da1", "da2", "da3", "da4"]).await;
    let (scope, [a, b, c]) = (&fixture.scope, &fixture.conversations);
    let (other_scope, _, other_run) = first_run(&store, &admin, "db1").await;
    let other_conversation = conversation_of(&admin, &other_run).await;
    let stranger = ProjectScope::new(
        UserId::new("018f0000-0000-7001-8000-0000000db999"),
        ProjectId::new(scope.project_id.as_ref()),
    );

    let list = list_project_conversations(&store, scope)
        .await
        .unwrap()
        .unwrap();
    let entries: Vec<Value> = list
        .items
        .iter()
        .map(|item| {
            json!([
                item.conversation_id,
                item.label,
                item.newest_run_status == AgentRunStatus::Completed,
                item.first_run_created_at <= item.newest_run_created_at,
            ])
        })
        .collect();
    let a_entry = &list.items[0];
    let read = read_project_conversation(&store, scope, a)
        .await
        .unwrap()
        .unwrap();
    let a_runs: Vec<Value> = read
        .runs
        .iter()
        .map(|run| {
            json!([
                run.run_id,
                run.author_message,
                run.chapter_id,
                run.status == AgentRunStatus::Completed
            ])
        })
        .collect();

    // The second Run of A makes A the newest. Each Conversation read gives only its own Runs.
    assert_eq!(
        (entries, list.completeness),
        (
            vec![
                json!([a, MESSAGES[0], true, true]),
                json!([c, MESSAGES[2], true, true]),
                json!([b, MESSAGES[1], true, true]),
            ],
            ProjectConversationCompleteness::Complete,
        )
    );
    assert_eq!(
        (
            a_entry.first_run_created_at < a_entry.newest_run_created_at,
            a_entry.newest_run_created_at.clone(),
            a_runs,
            read.completeness,
            read.earlier_turns_expired,
        ),
        (
            true,
            read.runs[1].created_at.clone(),
            vec![
                json!([fixture.runs[0], MESSAGES[0], fixture.chapter_id, true]),
                json!([fixture.runs[3], MESSAGES[3], fixture.chapter_id, true]),
            ],
            ProjectConversationCompleteness::Complete,
            false,
        )
    );
    // A smaller store limit keeps the newest entries and the newest Runs.
    let newest_two: Vec<String> = store
        .read_project_conversations(scope, 2)
        .await
        .unwrap()
        .unwrap()
        .into_iter()
        .map(|entry| entry.conversation_id)
        .collect();
    assert_eq!(
        (
            newest_two,
            run_ids(&store, scope, a, 1).await,
            run_ids(&store, scope, b, 64).await,
            run_ids(&store, scope, c, 64).await,
        ),
        (
            vec![a.clone(), c.clone()],
            Some(vec![fixture.runs[3].clone()]),
            Some(vec![fixture.runs[1].clone()]),
            Some(vec![fixture.runs[2].clone()]),
        )
    );
    // The list of Project A has no Conversation of Project B, and no read crosses a Scope.
    let other_list: Vec<String> = list_project_conversations(&store, &other_scope)
        .await
        .unwrap()
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.conversation_id)
        .collect();
    assert_eq!(
        (
            other_list,
            run_ids(&store, scope, &other_conversation, 64).await,
            run_ids(&store, &other_scope, a, 64).await,
            list_project_conversations(&store, &stranger).await.unwrap(),
            run_ids(&store, &stranger, a, 64).await,
            run_ids(&store, scope, "018f0000-0000-7001-8000-0000000dafff", 64).await,
        ),
        (
            vec![other_conversation.clone()],
            None,
            None,
            None,
            None,
            None
        )
    );
}

/// For each decision request of the Run in RunStep order: the prior response that it named, its
/// own response, and its serialized payload.
async fn requests(admin: &Client, run_id: &str) -> Vec<(Option<String>, String, String)> {
    admin
        .query(
            "SELECT (payload #>> '{wire,serialized_payload}')::jsonb ->> 'previous_response_reference',
                    payload ->> 'response_reference', payload #>> '{wire,serialized_payload}'
               FROM storyos.model_attempts
              WHERE run_id = $1::text::uuid AND attempt_role = 'decision'
              ORDER BY decision_position",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| (row.get(0), row.get(1), row.get(2)))
        .collect()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_conversation_keeps_its_own_provider_continuation() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = first_run(&store, &admin, "da5").await;
    let a = conversation_of(&admin, &first).await;
    let place = (&scope, chapter_id.as_str());
    let other = next_run(
        &store,
        &admin,
        place,
        "da6",
        MESSAGES[1],
        ConversationSelection::New,
    )
    .await;
    let second = next_run(
        &store,
        &admin,
        place,
        "da7",
        MESSAGES[3],
        ConversationSelection::Existing { conversation_id: a },
    )
    .await;
    let (first, other, second) = (
        requests(&admin, &first).await,
        requests(&admin, &other).await,
        requests(&admin, &second).await,
    );
    let responses_of = |requests: &[(Option<String>, String, String)]| -> Vec<String> {
        requests.iter().map(|request| request.1.clone()).collect()
    };
    let named_in_other = other.iter().filter_map(|request| request.0.clone());
    let named_in_second: Vec<String> = second
        .iter()
        .filter_map(|request| request.0.clone())
        .collect();

    // The first request of the second Run of A names the last response of the first Run of A.
    // The Run of B starts its own continuation, and no request of A or B has an item of the
    // other Conversation.
    assert_eq!(
        (
            second[0].0.clone(),
            other[0].0.clone(),
            named_in_second
                .iter()
                .any(|named| responses_of(&other).contains(named)),
            named_in_other
                .into_iter()
                .any(|named| responses_of(&first).contains(&named)),
            second.iter().any(|request| request.2.contains(MESSAGES[1])),
            other.iter().any(|request| request.2.contains(MESSAGES[0])),
        ),
        (
            Some(first.last().unwrap().1.clone()),
            None,
            false,
            false,
            false,
            false,
        )
    );
}

/// Moves the Receipt time and each Run event time of the Run back by `receipt` and `events` hours.
async fn move_back(admin: &Client, run_id: &str, receipt: i32, events: i32) {
    admin
        .execute(
            "UPDATE storyos.domain_receipts AS receipt
                SET created_at = created_at - make_interval(hours => $2)
               FROM storyos.agent_runs AS run
              WHERE run.run_id = $1::text::uuid
                AND (receipt.owner_user_id, receipt.project_id, receipt.receipt_id) =
                    (run.owner_user_id, run.project_id, run.receipt_id)",
            &[&run_id, &receipt],
        )
        .await
        .unwrap();
    admin
        .execute(
            "UPDATE storyos.agent_run_events
                SET recorded_at = clock_timestamp() - make_interval(hours => $2)
              WHERE run_id = $1::text::uuid",
            &[&run_id, &events],
        )
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_earlier_turns_expire_after_the_retention_from_the_last_run_event() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, run_id) = first_run(&store, &admin, "da8").await;
    let conversation = conversation_of(&admin, &run_id).await;
    let events: i64 = admin
        .query_one(
            "SELECT count(*) FROM storyos.agent_run_events WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let mut expired = Vec::new();
    // The Receipt is older than the retention each time, so the last Run event decides.
    for hours in [71, 73] {
        move_back(&admin, &run_id, 100, hours).await;
        expired.push(
            read_project_conversation(&store, &scope, &conversation)
                .await
                .unwrap()
                .unwrap()
                .earlier_turns_expired,
        );
    }

    assert_eq!((events > 0, expired), (true, vec![false, true]));
}
