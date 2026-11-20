//! The Host Tool `read_memory_entry` (ADR 0094 sections 4 and 8): the read of the current version
//! after a write of the same Run, the record of the read and the request after it, each refusal,
//! the Project Scope, the time of the last read, the grants and the archive of the read rows, a
//! Run that lost its Lease, a compaction, and a Run with no Memory use.

use serde_json::{Value, json};
use storyos_application::{
    CompleteAgentRunError, ContractFaultPoint, ConversationSelection, ProjectScope,
    complete_agent_run_with_tools,
};
use storyos_core::{
    ACTIVE_COMPACTION_POSITIONS_PREFIX, ACTIVE_COMPACTION_REQUEST_PREFIX, RESEARCH_TOOL_OFFER,
};
use tokio_postgres::Client;

use crate::shared_tests::agent_run_finish_rule::complete;
use crate::shared_tests::agent_run_load_skill::calls;
use crate::shared_tests::agent_run_read_chapter::{
    boundary, finish, no_public_call, outputs_record, sent,
};
use crate::shared_tests::agent_run_read_memory_entry::refused;
use crate::shared_tests::agent_run_retrieval::destination;
use crate::shared_tests::agent_run_steering::{cancellation, queued};
use crate::shared_tests::agent_run_tool_dispatch::{client, exa_tools, mcp_replies, point_mcp};
use crate::shared_tests::agent_run_tool_recovery::{Fault, FaultAt};
use crate::shared_tests::agent_run_write_memory::first_index;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::chapter_list_context::admitted;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::memory_entries::{archived_rows, as_runtime, write_versions};
use crate::shared_tests::model_gateway::{Admission, RuntimeClock, claim_run};

fn settled(text: &str) -> Value {
    json!(["read_memory_entry", "settled", null, text, true])
}

/// The result of a read of the current version of the entry `number` of the Project of `scope`,
/// with the time of the write of that version.
async fn read_result(admin: &Client, scope: &ProjectScope, number: i64) -> String {
    let row = admin
        .query_one(
            "SELECT entry_type, title, description, origin, text,
                    to_char(recorded_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
               FROM storyos.memory_entry_versions
              WHERE project_id = $1::text::uuid AND entry_number = $2::text::numeric
              ORDER BY memory_revision DESC LIMIT 1",
            &[&scope.project_id.as_ref(), &number.to_string()],
        )
        .await
        .unwrap();
    let value = |index| row.get::<_, String>(index);
    format!(
        "type: {}\ntitle: {}\ndescription: {}\norigin: {}\nchanged_at: {}\n\n{}",
        value(0),
        value(1),
        value(2),
        value(3),
        value(5),
        value(4)
    )
}

/// Each read row of the Run, in call order: whether it names the ToolCall of the call and the
/// Run, and the number and the version of the entry.
async fn reads(admin: &Client, run_id: &str) -> Vec<Value> {
    admin
        .query(
            "SELECT concat_ws(' ', read.run_id = call.run_id, version.entry_number, read.version)
               FROM storyos.memory_entry_reads AS read
               JOIN storyos.tool_calls AS call ON call.tool_call_id = read.tool_call_id
               JOIN storyos.memory_entry_versions AS version
                 ON (version.memory_entry_id, version.version) =
                    (read.memory_entry_id, read.version)
              WHERE call.run_id = $1::text::uuid
              ORDER BY call.decision_position, call.call_position",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| json!(row.get::<_, String>(/*idx*/ 0)))
        .collect()
}

/// The identity of the call `call_id` of the Run.
async fn call_id(admin: &Client, run_id: &str, call_id: &str) -> String {
    admin
        .query_one(
            "SELECT tool_call_id::text FROM storyos.tool_calls
              WHERE run_id = $1::text::uuid AND call_id = $2",
            &[&run_id, &call_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// The Run writes an entry with `write_memory` and reads it in the next RunStep. The pinned Index
/// of the Run has no entry, so the read gives the current version, not the version of the Index.
/// The read has its record and no Destination Attempt, it is not in the public ToolCall list,
/// and the next request sends its result as the output of the call.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_read_after_a_write_of_the_same_run_gives_the_current_version_and_the_request_sends_it() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "c21").await;
    complete(&store, &admin, &scope, &first).await;
    let create = json!({"type": "character", "title": "林舟", "description": "船夫。",
                        "text": "他不会游泳。\n\t他从不上船。"});
    let message = format!("SCRIPT:write_memory create {create}\nSCRIPT:read_memory_entry 1");
    let new = ConversationSelection::New;
    let run_id = admitted(&store, &scope, &chapter_id, "c22", &message, new).await;
    complete(&store, &admin, &scope, &run_id).await;

    let text = read_result(&admin, &scope, 1).await;
    let created = "The Host created the Memory entry 1 with the title \"林舟\".";
    let write = call_id(&admin, &run_id, "call_write_memory_0").await;
    let read = call_id(&admin, &run_id, "call_read_memory_entry_0").await;
    assert_eq!(
        (
            calls(&admin, &run_id).await,
            reads(&admin, &run_id).await,
            sent(&admin, &run_id).await[2].clone(),
            outputs_record(&admin, &run_id, 2).await,
            no_public_call(&store, &admin, &scope, &run_id, &read).await,
        ),
        (
            json!([
                "completed",
                [
                    ["write_memory", "settled", null, created, true],
                    settled(&text),
                    finish()
                ]
            ]),
            vec![json!("t 1 1")],
            json!([
                "decision",
                2,
                [
                    [
                        "call_write_memory_0",
                        boundary("call_write_memory_0", "ok", created)
                    ],
                    [
                        "call_read_memory_entry_0",
                        boundary("call_read_memory_entry_0", "ok", &text)
                    ]
                ],
                false
            ]),
            json!([
                [["data_only", "ok"], ["data_only", "ok"]],
                [
                    ["tool_result", write, "exact_required"],
                    ["memory_read_result", read, "exact_required"]
                ]
            ]),
            true,
        )
    );
}

/// The author deletes the entry 2 after the Run pinned an Index with it. A read of that entry and
/// a read of a number of no entry are refused with one sentence, write no row, and the Run
/// continues; their outputs stay `tool_result` items. Two Projects each have an entry 1, and each
/// Run reads the entry of its own Project.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_read_of_a_deleted_or_missing_entry_is_refused_and_each_run_reads_its_own_project() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "c23").await;
    complete(&store, &admin, &scope, &first).await;
    let (other, other_chapter, other_first) = queued(&store, "c25").await;
    complete(&store, &admin, &other, &other_first).await;
    let versions = [
        (1, 1, 1, "current", "character", "林舟", "船夫。"),
        (2, 1, 2, "current", "other", "渡口", "旧渡口。"),
    ];
    write_versions(&admin, &scope, &versions).await;
    let versions = [(1, 1, 1, "current", "foreshadowing", "铜铃", "铃声。")];
    write_versions(&admin, &other, &versions).await;
    let new = ConversationSelection::New;
    let message = "SCRIPT:read_memory_entry 2,3,1";
    let run_id = admitted(&store, &scope, &chapter_id, "c24", message, new.clone()).await;
    let deleted = [(2, 2, 3, "deleted", "other", "渡口", "旧渡口。")];
    write_versions(&admin, &scope, &deleted).await;
    complete(&store, &admin, &scope, &run_id).await;
    let message = "SCRIPT:read_memory_entry 1";
    let other_run = admitted(&store, &other, &other_chapter, "c26", message, new).await;
    complete(&store, &admin, &other, &other_run).await;

    let lin = read_result(&admin, &scope, 1).await;
    let bell = read_result(&admin, &other, 1).await;
    let classes: Vec<Value> = outputs_record(&admin, &run_id, 1).await[1]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item[0].clone())
        .collect();
    let pinned = first_index(&admin, &run_id).await[0]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        (
            pinned.contains(r#""id":2,"title":"渡口""#),
            calls(&admin, &run_id).await,
            reads(&admin, &run_id).await,
            classes,
            calls(&admin, &other_run).await,
            reads(&admin, &other_run).await,
        ),
        (
            true,
            json!(["completed", [refused(), refused(), settled(&lin), finish()]]),
            vec![json!("t 1 1")],
            vec![
                json!("tool_result"),
                json!("tool_result"),
                json!("memory_read_result")
            ],
            json!(["completed", [settled(&bell), finish()]]),
            vec![json!("t 1 1")],
        )
    );
}

/// Two Runs read the entry 1 and no Run reads the entry 2: the time of the last read of the entry
/// 1 is the newest read, and the entry 2 has none. The runtime role cannot change or delete a
/// read row, and the Project Export Archive has each read row.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_last_read_is_the_newest_read_row_and_the_archive_has_each_read() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "c27").await;
    complete(&store, &admin, &scope, &first).await;
    let versions = [
        (1, 1, 1, "current", "character", "林舟", "船夫。"),
        (2, 1, 2, "current", "other", "渡口", "旧渡口。"),
    ];
    write_versions(&admin, &scope, &versions).await;
    for prefix in ["c28", "c29"] {
        let new = ConversationSelection::New;
        let message = "SCRIPT:read_memory_entry 1";
        let run_id = admitted(&store, &scope, &chapter_id, prefix, message, new).await;
        complete(&store, &admin, &scope, &run_id).await;
    }

    let newest: String = admin
        .query_one(
            "SELECT to_char(max(read_at) AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
               FROM storyos.memory_entry_reads WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let last = crate::memory_entries::last_memory_reads(&admin, &scope)
        .await
        .unwrap();
    let statements = [
        "UPDATE storyos.memory_entry_reads SET version = 1",
        "DELETE FROM storyos.memory_entry_reads",
    ];
    let [archived] = archived_rows(&store, &scope, "c2", ["memory_entry_reads"]).await;
    let archived: Vec<_> = archived.iter().map(|row| row["version"].clone()).collect();
    assert_eq!(
        (
            last.iter()
                .map(|(_, number, read)| (*number, read.clone()))
                .collect::<Vec<_>>(),
            as_runtime(&scope, &statements).await,
            archived,
        ),
        (
            vec![(1, Some(newest)), (2, None)],
            vec![Err("42501".to_owned()), Err("42501".to_owned())],
            vec![json!(1), json!(1)],
        )
    );
}

/// The response has a `web_search` call before a read. The author cancels the Run after the
/// dispatch claim of the search, so the Worker loses the Run Lease before the Tool Gateway
/// reaches the read: the Host refuses the read and writes no read row.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_read_of_a_run_that_lost_its_lease_writes_no_read_row() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "c2a").await;
    complete(&store, &admin, &scope, &first).await;
    let versions = [(1, 1, 1, "current", "other", "潮汐", "潮汐表。")];
    write_versions(&admin, &scope, &versions).await;
    let run_id = Admission {
        message: "SCRIPT:read_memory_entry 1\nSearch the web: tide tables",
        research: Some(&RESEARCH_TOOL_OFFER),
        ..Default::default()
    }
    .admit(&store, &scope, &chapter_id, "c2b")
    .await;
    let (mcp_base, _mcp) = destination(mcp_replies(exa_tools(|_| {}), None)).await;
    point_mcp(&admin, &scope, &mcp_base).await;
    let cancel = Fault::Cancel {
        store: &store,
        call: Box::new(cancellation(&store, &scope, 0xc210, &run_id).await),
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
    let read = &status[1][1];
    assert_eq!(
        (
            matches!(fenced, Err(CompleteAgentRunError::StaleFence)),
            json!([status[0], read[0], read[1], read[2]]),
            reads(&admin, &run_id).await,
        ),
        (
            true,
            json!(["cancelled", "read_memory_entry", "refused", "run_cancelled"]),
            Vec::<Value>::new(),
        )
    );
}

/// The message has the compaction prefix of the fake before the read script. The summary request
/// continues the response of the read, and the later request is a full input: both send the
/// exact result of the read, because only the result of a research Tool can be a stub.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_compaction_after_a_read_keeps_the_exact_result() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "c2c").await;
    complete(&store, &admin, &scope, &first).await;
    let versions = [(1, 1, 1, "current", "relationship", "林舟与林雪", "姐妹。")];
    write_versions(&admin, &scope, &versions).await;
    let message = format!(
        "{ACTIVE_COMPACTION_REQUEST_PREFIX} {ACTIVE_COMPACTION_POSITIONS_PREFIX} 1. \
         SCRIPT:read_memory_entry 1"
    );
    let new = ConversationSelection::New;
    let run_id = admitted(&store, &scope, &chapter_id, "c2d", &message, new).await;
    complete(&store, &admin, &scope, &run_id).await;

    let text = read_result(&admin, &scope, 1).await;
    let output = json!([[
        "call_read_memory_entry_0",
        boundary("call_read_memory_entry_0", "ok", &text)
    ]]);
    let sent = sent(&admin, &run_id).await;
    assert_eq!(
        (
            calls(&admin, &run_id).await[0].clone(),
            json!([sent[1][0], sent[1][2]]),
            json!([sent[2][0], sent[2][2], sent[2][3]]),
        ),
        (
            json!("completed"),
            json!(["compaction", output]),
            json!(["decision", output, true]),
        )
    );
}

/// A Run whose captured Conversation Memory Settings have `use_enabled = false` does not declare
/// `read_memory_entry`. The test writes the settings row with SQL.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_run_with_no_memory_use_declares_no_read() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "c2e").await;
    complete(&store, &admin, &scope, &first).await;
    admin
        .batch_execute(&format!(
            "UPDATE storyos.conversation_memory_settings SET is_current = FALSE
              WHERE project_id = '{project}'::uuid;
             INSERT INTO storyos.conversation_memory_settings
             SELECT (jsonb_populate_record(NULL::storyos.conversation_memory_settings,
               (to_jsonb(settings) - 'is_current') || jsonb_build_object(
                 'memory_settings_revision', gen_random_uuid(), 'use_enabled', false))).*
               FROM storyos.conversation_memory_settings AS settings
              WHERE project_id = '{project}'::uuid",
            project = scope.project_id.as_ref()
        ))
        .await
        .unwrap();
    let conversation = ConversationSelection::Existing {
        conversation_id: "018f0000-0000-7001-8000-00000006c2e6".to_owned(),
    };
    let message = "SCRIPT:read_memory_entry 1";
    let run_id = admitted(&store, &scope, &chapter_id, "c2f", message, conversation).await;
    complete(&store, &admin, &scope, &run_id).await;

    let declared: Value = json_of(
        &admin,
        "SELECT ((payload #>> '{wire,serialized_payload}')::jsonb -> 'tools')::text
               FROM storyos.model_attempts
              WHERE run_id = $1::text::uuid AND decision_position = 0",
        &[&run_id],
    )
    .await;
    assert_eq!(
        (
            declared,
            calls(&admin, &run_id).await[1][0].as_array().unwrap()[0..3].to_vec()
        ),
        (
            json!([
                "propose_edit",
                "submit_research",
                "read_tool_result",
                "read_chapter",
                "search_manuscript",
                "write_memory",
                "finish"
            ]),
            vec![
                json!("read_memory_entry"),
                json!("refused"),
                json!("unexposed_name")
            ]
        )
    );
}
