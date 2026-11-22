//! The Host Tool `write_memory` (ADR 0094 section 5, writer 1): the settled writes of one
//! response in call order, the record and the next request, each refusal, the Project Scope, a
//! cancel after a settled write, a Run that lost its Lease, the Index of the writing Run and of
//! the next Run, and the Project Export Archive.

use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore, CompleteAgentRunError,
    ContractFaultPoint, ConversationSelection, NoContractFaults, complete_agent_run,
    complete_agent_run_with_tools,
};
use storyos_core::{MEMORY_TITLE_MAX_CHARS, MemoryWriteRefusal, RESEARCH_TOOL_OFFER};

use crate::shared_tests::agent_run_finish_rule::complete;
use crate::shared_tests::agent_run_load_skill::calls;
use crate::shared_tests::agent_run_retrieval::destination;
use crate::shared_tests::agent_run_steering::{
    AuthorAction, AuthorAt, Moment, cancellation, claim_duties, queued, settle,
};
use crate::shared_tests::agent_run_tool_dispatch::{client, exa_tools, mcp_replies, point_mcp};
use crate::shared_tests::agent_run_tool_recovery::{Fault, FaultAt};
use crate::shared_tests::agent_run_write_memory::{first_index, memory_head, refused, versions};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::chapter_list_context::admitted;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::memory_entries::{archived, write_versions};
use crate::shared_tests::model_gateway::{Admission, RuntimeClock, claim_run};

/// The fake script line of one `write_memory` call.
fn script(action: &str, arguments: &Value) -> String {
    format!("SCRIPT:write_memory {action} {arguments}")
}

fn settled(text: &str) -> Value {
    json!(["write_memory", "settled", null, text, true])
}

fn finish() -> Value {
    json!(["finish", "settled", null, "", true])
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_create_and_a_change_of_one_response_settle_in_order_and_the_next_run_gets_the_entry() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "9f1").await;
    complete(&store, &admin, &scope, &first).await;
    // The change names the entry 1 that the create of the same response writes.
    let create = json!({"type": "character", "title": " 林舟 ", "description": "船夫。",
                        "text": "他不说话。"});
    let change = json!({"entry": 1, "description": "沉默的船夫。"});
    let message = format!(
        "{}\n{}",
        script("create", &create),
        script("change", &change)
    );
    let new = ConversationSelection::New;
    let run_id = admitted(&store, &scope, &chapter_id, "9f2", &message, new.clone()).await;
    complete(&store, &admin, &scope, &run_id).await;
    let next = admitted(&store, &scope, &chapter_id, "9f3", "Help.", new).await;
    complete(&store, &admin, &scope, &next).await;

    let created = "The Host created the Memory entry 1 with the title \"林舟\".";
    let changed = "The Host changed the Memory entry 1 with the title \"林舟\".";
    assert_eq!(
        calls(&admin, &run_id).await,
        json!(["completed", [settled(created), settled(changed), finish()]])
    );
    // Each version has the origin and the cause `author_request`, and the Memory revision moves
    // by one for each write.
    assert_eq!(
        (
            versions(&admin, &scope).await,
            memory_head(&admin, &scope).await
        ),
        (
            vec![
                json!(
                    "1 1 1 current character author_request 林舟 船夫。 他不说话。 author_request"
                ),
                json!(
                    "1 2 2 current character author_request 林舟 沉默的船夫。 他不说话。 author_request"
                ),
            ],
            Some((2, 2))
        )
    );
    // The entry identity is the ToolCall of the create, and the source reference of each version
    // names the Conversation, the Run, and the ToolCall of its write.
    let sources: Value = json_of(
        &admin,
        "SELECT jsonb_agg(jsonb_build_array(version.memory_entry_id = first.tool_call_id,
                      version.source_references = jsonb_build_array(jsonb_build_object(
                        'kind', 'conversation', 'conversation_id', run.conversation_id::text,
                        'run_id', run.run_id::text, 'tool_call_id', call.tool_call_id::text)))
                      ORDER BY version.version)::text
               FROM storyos.memory_entry_versions AS version
               JOIN storyos.agent_runs AS run ON run.run_id = $1::text::uuid
               JOIN storyos.tool_calls AS first
                 ON first.run_id = run.run_id AND first.call_id = 'call_write_memory_0'
               JOIN storyos.tool_calls AS call
                 ON call.run_id = run.run_id
                AND call.call_id = 'call_write_memory_' || (version.version - 1)
              WHERE version.project_id = run.project_id",
        &[&run_id],
    )
    .await;
    assert_eq!(sources, json!([[true, true], [true, true]]));
    // The request after the writes sends each result as the output of its call in the
    // `tool_result` boundary (ADR 0085).
    let outputs: String = admin
        .query_one(
            "SELECT (payload #>> '{wire,serialized_payload}')::jsonb -> 'tool_outputs'
                      #>> '{1,output}'
               FROM storyos.model_attempts
              WHERE run_id = $1::text::uuid AND decision_position = 1",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let boundary = format!(
        "<tool_result source=\"call_write_memory_1\" state=\"ok\">{}</tool_result>",
        changed.replace('"', "&quot;")
    );
    assert_eq!(outputs, boundary);
    // The public ToolCall list has no Host ToolCall.
    let attempt_id: String = admin
        .query_one(
            "SELECT model_attempt_id::text FROM storyos.tool_calls
              WHERE run_id = $1::text::uuid AND call_id = 'call_write_memory_0'",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let public = store
        .read_agent_run(
            &scope,
            &run_id,
            &AgentRunReadSelection::ModelAttempt(attempt_id),
        )
        .await
        .unwrap()
        .unwrap()
        .decision;
    assert!(matches!(
        public,
        AgentRunDecisionInspect::ToolRequest { ref calls, .. } if calls.is_empty()
    ));
    // The writing Run keeps the Index of its start, and the next Run gets the entry.
    let index = "{\"type\":\"character\"}\n\
                 {\"author_part\":true,\"description\":\"沉默的船夫。\",\"id\":1,\"title\":\"林舟\"}";
    assert_eq!(
        (
            first_index(&admin, &run_id).await,
            first_index(&admin, &next).await
        ),
        (json!([]), json!([index]))
    );
    // The Project Export Archive has the versions that the Tool wrote.
    assert_eq!(
        archived(&store, &scope, "9f").await,
        (
            vec![json!([1, 1, 1, "current"]), json!([1, 2, 2, "current"])],
            vec![json!([2, 2])]
        )
    );
}

/// One call of each refusal of the table in a Project with 300 current entries. A refused call
/// writes no row, its result gives the reason, and the Run continues.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_refused_write_writes_no_row_and_the_run_continues() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "9f4").await;
    complete(&store, &admin, &scope, &first).await;
    let titles: Vec<String> = (1..=300).map(|number| format!("事件{number}")).collect();
    let full: Vec<_> = (1..=300_u64)
        .map(|n| {
            let title = titles[usize::try_from(n - 1).unwrap()].as_str();
            (n, 1, n, "current", "major_event", title, "d")
        })
        .collect();
    write_versions(&admin, &scope, &full).await;
    let entry = |title: &str, text: Value| json!({"type": "major_event", "title": title, "description": "d", "text": text});
    let message = [
        script("create", &entry("事件301", json!("x"))),
        script("change", &json!({"entry": 301, "text": "x"})),
        script("create", &entry(" 事件7 ", json!("x"))),
        script(
            "create",
            &entry(&"长".repeat(MEMORY_TITLE_MAX_CHARS + 1), json!("x")),
        ),
        script("create", &entry("事件302", Value::Null)),
    ]
    .join("\n");
    let new = ConversationSelection::New;
    let run_id = admitted(&store, &scope, &chapter_id, "9f5", &message, new).await;
    complete(&store, &admin, &scope, &run_id).await;

    assert_eq!(
        (
            calls(&admin, &run_id).await,
            versions(&admin, &scope).await.len(),
            memory_head(&admin, &scope).await
        ),
        (
            json!([
                "completed",
                [
                    refused(MemoryWriteRefusal::IndexFull),
                    refused(MemoryWriteRefusal::NotFound),
                    refused(MemoryWriteRefusal::Exists(7)),
                    refused(MemoryWriteRefusal::TooLong),
                    refused(MemoryWriteRefusal::Invalid),
                    finish(),
                ]
            ]),
            300,
            Some((300, 301))
        )
    );
}

/// Two Projects each have an entry 1. A change in one Project writes only its own entry.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_change_writes_only_the_entry_of_the_project_of_its_run() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "9f6").await;
    complete(&store, &admin, &scope, &first).await;
    let (other, _, other_first) = queued(&store, "9f7").await;
    complete(&store, &admin, &other, &other_first).await;
    let entry = [(1, 1, 1, "current", "character", "林舟", "船夫。")];
    write_versions(&admin, &scope, &entry).await;
    write_versions(&admin, &other, &entry).await;
    let message = script("change", &json!({"entry": 1, "text": "他会游泳。"}));
    let new = ConversationSelection::New;
    let run_id = admitted(&store, &scope, &chapter_id, "9f8", &message, new).await;
    complete(&store, &admin, &scope, &run_id).await;

    // The row that `write_versions` writes has no cause kind.
    let kept = json!("1 1 1 current character author_request 林舟 船夫。 船夫。");
    let changed =
        json!("1 2 2 current character author_request 林舟 船夫。 他会游泳。 author_request");
    assert_eq!(
        (
            versions(&admin, &scope).await,
            versions(&admin, &other).await,
            memory_head(&admin, &other).await
        ),
        (vec![kept.clone(), changed], vec![kept], Some((1, 2)))
    );
}

/// The author cancels the Run after its `create` settled: the write stays.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_settled_create_stays_when_the_author_cancels_the_run_later() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "9f9").await;
    complete(&store, &admin, &scope, &first).await;
    let create = json!({"type": "foreshadowing", "title": "铜铃", "description": "铃声。",
                        "text": "铜铃会再响。"});
    let new = ConversationSelection::New;
    let message = script("create", &create);
    let run_id = admitted(&store, &scope, &chapter_id, "9fa", &message, new).await;
    let cancel = cancellation(&store, &scope, 0x8d10, &run_id).await;
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(2),
        AuthorAction::Cancel(Box::new(cancel)),
    );
    let _cancelled = settle(&store, &admin, &scope, &run_id, &author).await;
    // A new claim does the cancellation duties of the request that was out, so the Run leaves no
    // work for the drain of the HTTP tests.
    let claim = claim_duties(&admin, &scope, &run_id).await.unwrap();
    complete_agent_run(
        &store,
        &FakeDestination,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await
    .unwrap();

    let created = "The Host created the Memory entry 1 with the title \"铜铃\".";
    let status = calls(&admin, &run_id).await;
    assert_eq!(
        (
            &status[0],
            &status[1][0],
            versions(&admin, &scope).await.len(),
            memory_head(&admin, &scope).await
        ),
        (&json!("cancelled"), &settled(created), 1, Some((1, 2)))
    );
}

/// The response has a `web_search` call before a `create`. The author cancels the Run after the
/// dispatch claim of the search, so the Worker loses the Run Lease before the Tool Gateway
/// reaches the `create`: the Host refuses the `create` and writes nothing.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_create_of_a_run_that_lost_its_lease_writes_nothing() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "9fb").await;
    complete(&store, &admin, &scope, &first).await;
    let create = json!({"type": "other", "title": "潮汐", "description": "潮汐表。",
                        "text": "1890 年的潮汐表。"});
    let message = format!("{}\nSearch the web: tide tables", script("create", &create));
    let run_id = Admission {
        message: &message,
        research: Some(&RESEARCH_TOOL_OFFER),
        ..Default::default()
    }
    .admit(&store, &scope, &chapter_id, "9fc")
    .await;
    let (mcp_base, _mcp) = destination(mcp_replies(exa_tools(|_| {}), None)).await;
    point_mcp(&admin, &scope, &mcp_base).await;
    let cancel = Fault::Cancel {
        store: &store,
        call: Box::new(cancellation(&store, &scope, 0x8d11, &run_id).await),
    };
    let observer = FaultAt::new(ContractFaultPoint::ToolDispatchClaimed, 1, cancel);
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    let tools = client();
    let fake = storyos_adapter_fake_destination::FakeDestination;
    let gateway =
        complete_agent_run_with_tools(&store, &fake, &tools, &observer, &RuntimeClock, &claim);
    let fenced = tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 120), gateway)
        .await
        .expect("the Run settles");

    let status = calls(&admin, &run_id).await;
    let create = &status[1][1];
    assert_eq!(
        (
            matches!(fenced, Err(CompleteAgentRunError::StaleFence)),
            json!([status[0], create[0], create[1], create[2]]),
            versions(&admin, &scope).await.len(),
            memory_head(&admin, &scope).await
        ),
        (
            true,
            json!(["cancelled", "write_memory", "refused", "run_cancelled"]),
            0,
            None
        )
    );
}
