//! The Conversation list and the Conversation read with real PostgreSQL, and the Provider
//! continuation of each Conversation (ADR 0094 section 10, ADR 0053).

use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    ConversationSelection as Selection, NoContractFaults, ProjectConversationListItem,
    ProjectConversationRead, ProjectConversationReader, ProjectConversationRunItem, ProjectId,
    ProjectScope, UserId, complete_agent_run, list_project_conversations,
    read_project_conversation,
};
use storyos_contracts::ProjectConversationCompleteness::Complete;
use storyos_core::{AgentRunStatus, DeploymentDestination::HostFakeToolLoop};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_steering::cancellation;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::model_gateway::{
    RuntimeClock, admit_run_in, claim_run, queued_run_offering,
};

const MESSAGES: [&str; 4] = [
    "Help with this passage.",
    "Write about the storm.",
    "Name the river.",
    "Keep the voice of the narrator.",
];

async fn complete(store: &PostgresProjectReader, admin: &Client, scope: &ProjectScope, run: &str) {
    let claim = claim_run(admin, scope.clone(), run.to_owned()).await;
    let destination = (&FakeDestination, &NoContractFaults, &RuntimeClock);
    complete_agent_run(store, destination.0, destination.1, destination.2, &claim)
        .await
        .unwrap();
}

async fn conversation_of(admin: &Client, run_id: &str) -> String {
    let query =
        "SELECT conversation_id::text FROM storyos.agent_runs WHERE run_id = $1::text::uuid";
    let row = admin.query_one(query, &[&run_id]).await.unwrap();
    row.get(/*idx*/ 0)
}

/// Admits a Run with `message` in `conversation` on the Chapter of the tool loop profile.
async fn admit(
    store: &PostgresProjectReader,
    (scope, chapter_id): (&ProjectScope, &str),
    prefix: &str,
    message: &str,
    conversation: Selection,
) -> String {
    let destination = HostFakeToolLoop;
    admit_run_in(
        store,
        scope,
        chapter_id,
        prefix,
        destination,
        message,
        conversation,
        None,
    )
    .await
}

/// The completed Runs A1, B1, C1, and A2, in this order, of the Conversations A, B, and C of a
/// new Project. `prefixes` give the identities. The first message of A is `MESSAGES[0]`.
async fn three_conversations(
    store: &PostgresProjectReader,
    admin: &Client,
    prefixes: [&str; 4],
) -> (ProjectScope, String, Vec<String>, Vec<String>) {
    let (scope, chapter_id, first) =
        queued_run_offering(store, prefixes[0], HostFakeToolLoop, None).await;
    complete(store, admin, &scope, &first).await;
    let a = conversation_of(admin, &first).await;
    let mut runs = vec![first];
    for (index, prefix) in prefixes.into_iter().enumerate().skip(1) {
        let conversation = match index {
            3 => Selection::Existing {
                conversation_id: a.clone(),
            },
            _ => Selection::New,
        };
        let place = (&scope, chapter_id.as_str());
        let run = admit(store, place, prefix, MESSAGES[index], conversation).await;
        complete(store, admin, &scope, &run).await;
        runs.push(run);
    }
    let conversations = vec![
        a,
        conversation_of(admin, &runs[1]).await,
        conversation_of(admin, &runs[2]).await,
    ];
    (scope, chapter_id, runs, conversations)
}

async fn run_ids(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    conversation_id: &str,
    limit: usize,
) -> Option<Vec<String>> {
    let read = store.read_project_conversation(scope, conversation_id, limit);
    let runs = read.await.unwrap()?.newest_first.into_iter();
    Some(runs.map(|run| run.run_id).collect())
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_list_and_the_read_give_the_conversations_of_one_scope_in_order() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, runs, conversations) =
        three_conversations(&store, &admin, ["da1", "da2", "da3", "da4"]).await;
    let [a, b, c] = [&conversations[0], &conversations[1], &conversations[2]];
    let (other_scope, _, other_run) =
        queued_run_offering(&store, "db1", HostFakeToolLoop, None).await;
    complete(&store, &admin, &other_scope, &other_run).await;
    let other = conversation_of(&admin, &other_run).await;
    let stranger = ProjectScope::new(
        UserId::new("018f0000-0000-7001-8000-0000000db999"),
        ProjectId::new(scope.project_id.as_ref()),
    );

    let list = list_project_conversations(&store, &scope).await;
    let list = list.unwrap().unwrap();
    let read = async |id: &str| {
        let read = read_project_conversation(&store, &scope, id).await;
        read.unwrap().unwrap()
    };
    let (read_a, read_b, read_c) = (read(a).await, read(b).await, read(c).await);
    let entry = |item: &ProjectConversationListItem| {
        let completed = item.newest_run_status == AgentRunStatus::Completed;
        let times = [&item.first_run_created_at, &item.newest_run_created_at];
        json!([item.conversation_id, item.label, completed, times])
    };
    let run = |run: &ProjectConversationRunItem| {
        let completed = run.status == AgentRunStatus::Completed;
        json!([run.run_id, run.author_message, run.chapter_id, completed])
    };
    let time = |read: &ProjectConversationRead, index: usize| read.runs[index].created_at.clone();

    // The second Run of A makes A the newest. Each Conversation read gives only its own Runs.
    assert_eq!(
        (
            list.items.iter().map(entry).collect::<Vec<_>>(),
            list.completeness
        ),
        (
            vec![
                json!([a, MESSAGES[0], true, [time(&read_a, 0), time(&read_a, 1)]]),
                json!([c, MESSAGES[2], true, [time(&read_c, 0), time(&read_c, 0)]]),
                json!([b, MESSAGES[1], true, [time(&read_b, 0), time(&read_b, 0)]]),
            ],
            Complete,
        )
    );
    assert_eq!(
        (
            read_a.runs.iter().map(run).collect::<Vec<_>>(),
            read_a.completeness
        ),
        (
            vec![
                json!([runs[0], MESSAGES[0], chapter_id, true]),
                json!([runs[3], MESSAGES[3], chapter_id, true]),
            ],
            Complete,
        )
    );
    assert!(time(&read_a, 0) < time(&read_a, 1) && !read_a.earlier_turns_expired);
    // A smaller store limit keeps the newest entries and the newest Runs.
    let newest_two = store
        .read_project_conversations(&scope, 2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            newest_two
                .into_iter()
                .map(|entry| entry.conversation_id)
                .collect::<Vec<_>>(),
            run_ids(&store, &scope, a, 1).await,
            run_ids(&store, &scope, b, 64).await,
        ),
        (
            vec![a.clone(), c.clone()],
            Some(vec![runs[3].clone()]),
            Some(vec![runs[1].clone()]),
        )
    );
    // The list of Project A has no Conversation of Project B, and no read crosses a Scope.
    let other_list = list_project_conversations(&store, &other_scope)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            other_list
                .items
                .into_iter()
                .map(|item| item.conversation_id)
                .collect::<Vec<_>>(),
            run_ids(&store, &scope, &other, 64).await,
            run_ids(&store, &other_scope, a, 64).await,
            list_project_conversations(&store, &stranger).await.unwrap(),
            run_ids(&store, &stranger, a, 64).await,
            run_ids(&store, &scope, "018f0000-0000-7001-8000-0000000dafff", 64).await,
        ),
        (vec![other.clone()], None, None, None, None, None)
    );
}

/// For each decision request of the Run in RunStep order: the prior response that it named, its
/// own response, and its serialized payload.
async fn requests(admin: &Client, run_id: &str) -> Vec<(Option<String>, String, String)> {
    let query = "SELECT payload ->> 'response_reference', payload #>> '{wire,serialized_payload}'
                   FROM storyos.model_attempts
                  WHERE run_id = $1::text::uuid AND attempt_role = 'decision'
                  ORDER BY decision_position";
    let rows = admin.query(query, &[&run_id]).await.unwrap();
    rows.iter()
        .map(|row| {
            let wire: String = row.get(/*idx*/ 1);
            let body: Value = serde_json::from_str(&wire).unwrap();
            let named = body["previous_response_reference"]
                .as_str()
                .map(str::to_owned);
            (named, row.get(/*idx*/ 0), wire)
        })
        .collect()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_conversation_keeps_its_own_provider_continuation() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (_, _, runs, _) = three_conversations(&store, &admin, ["da5", "da6", "da7", "da8"]).await;
    let (first, other, second) = (
        requests(&admin, &runs[0]).await,
        requests(&admin, &runs[1]).await,
        requests(&admin, &runs[3]).await,
    );
    let named = |requests: &[(Option<String>, String, String)]| -> Vec<String> {
        requests
            .iter()
            .filter_map(|request| request.0.clone())
            .collect()
    };
    let answers = |requests: &[(Option<String>, String, String)], named: &[String]| {
        requests.iter().any(|request| named.contains(&request.1))
    };

    // The first request of the second Run of A names the last response of the first Run of A.
    // The Run of B starts its own continuation, and no request of A or B has an item of the
    // other Conversation.
    assert_eq!(
        (
            second[0].0.clone(),
            other[0].0.clone(),
            answers(&other, &named(&second)),
            answers(&first, &named(&other)),
            second.iter().any(|request| request.2.contains(MESSAGES[1])),
            other.iter().any(|request| request.2.contains(MESSAGES[0])),
        ),
        (
            Some(first.last().unwrap().1.clone()),
            None,
            false,
            false,
            false,
            false
        )
    );
}

/// Sets each Run event time of the Run to `hours` before now.
async fn move_events(admin: &Client, run_id: &str, hours: i32) {
    let query = "UPDATE storyos.agent_run_events
                    SET recorded_at = clock_timestamp() - make_interval(hours => $2)
                  WHERE run_id = $1::text::uuid";
    admin.execute(query, &[&run_id, &hours]).await.unwrap();
}

/// Admits a Run of `conversation` and cancels it before its first Tool call, so it has no Run
/// event. Its Receipt moves back 100 hours, so a read that used the Receipt time would expire.
async fn cancelled_run(
    (store, admin): (&PostgresProjectReader, &Client),
    place: (&ProjectScope, &str),
    (prefix, suffix): (&str, u16),
    conversation: Selection,
) -> String {
    let (scope, run) = (
        place.0,
        admit(store, place, prefix, MESSAGES[3], conversation).await,
    );
    let cancel = cancellation(store, scope, suffix, &run).await;
    store
        .cancel_agent_run(&cancel.envelope, &cancel.input)
        .await
        .unwrap();
    let receipts = "UPDATE storyos.domain_receipts AS receipt
                       SET created_at = created_at - interval '100 hours'
                      FROM storyos.agent_runs AS run
                     WHERE run.run_id = $1::text::uuid AND run.status = 'cancelled'
                       AND (receipt.owner_user_id, receipt.project_id, receipt.receipt_id) =
                           (run.owner_user_id, run.project_id, run.receipt_id)";
    assert_eq!(admin.execute(receipts, &[&run]).await.unwrap(), 1);
    run
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_earlier_turns_expire_after_the_retention_from_the_newest_run_event() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, run_id) =
        queued_run_offering(&store, "da9", HostFakeToolLoop, None).await;
    complete(&store, &admin, &scope, &run_id).await;
    let conversation = conversation_of(&admin, &run_id).await;
    let query = "SELECT count(*) FROM storyos.agent_run_events WHERE run_id = $1::text::uuid";
    let events: i64 = admin
        .query_one(query, &[&run_id])
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let read = async |id: &str| {
        let read = read_project_conversation(&store, &scope, id).await;
        read.unwrap().unwrap().earlier_turns_expired
    };
    let mut expired = Vec::new();
    for hours in [71, 73] {
        move_events(&admin, &run_id, hours).await;
        expired.push(read(&conversation).await);
    }
    // A newest Run with no Run event: the old event of the earlier Run decides.
    let existing = Selection::Existing {
        conversation_id: conversation.clone(),
    };
    let place = (&scope, chapter_id.as_str());
    cancelled_run((&store, &admin), place, ("dab", 0xdab0), existing).await;
    expired.push(read(&conversation).await);
    // A Conversation with no Run event gives false, though its Receipt is old.
    let lone = cancelled_run((&store, &admin), place, ("dac", 0xdac0), Selection::New).await;
    expired.push(read(&conversation_of(&admin, &lone).await).await);

    assert_eq!(
        (events > 0, expired),
        (true, vec![false, true, true, false])
    );
}
