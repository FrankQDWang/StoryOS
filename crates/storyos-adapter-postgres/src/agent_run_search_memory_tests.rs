//! The Host Tool `search_memory` (ADR 0094 section 4): the search of the current entries after
//! the pin of the Index, the record of the call and the request after it, no read row, the
//! Project Scope, a search with no match, a refused query, and a compaction.

use serde_json::{Value, json};
use storyos_application::ConversationSelection;
use storyos_core::{
    ACTIVE_COMPACTION_POSITIONS_PREFIX, ACTIVE_COMPACTION_REQUEST_PREFIX, CallValidationRefusal,
    IntakeRefusal,
};
use tokio_postgres::Client;

use crate::agent_run_dispatch::tests::stores;
use crate::agent_run_finish_rule::tests::complete;
use crate::agent_run_load_skill::tests::calls;
use crate::agent_run_read_chapter::tests::{
    boundary, finish, no_public_call, outputs_record, sent,
};
use crate::agent_run_steering::tests::queued;
use crate::agent_run_write_memory::tests::first_index;
use crate::create_agent_run::context::chapter_list_tests::admitted;
use crate::memory_entries::tests::write_versions;

fn settled(text: &str) -> Value {
    json!(["search_memory", "settled", null, text, true])
}

/// The count of the read rows of the Project of the Run `run_id`.
async fn read_rows(admin: &Client, run_id: &str) -> i64 {
    admin
        .query_one(
            "SELECT count(*) FROM storyos.memory_entry_reads
              WHERE project_id = (SELECT project_id FROM storyos.agent_runs
                                   WHERE run_id = $1::text::uuid)",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// The author adds the entry 2 after the Run pinned an Index with only the entry 1. The search
/// reads the current entries at the time of the call, so it finds the entry 2. The call has its
/// record and no Destination Attempt, it is not in the public ToolCall list, it writes no read
/// row, and the next request sends its result as the output of the call, in a
/// `memory_search_result` item.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_search_finds_the_current_entries_and_the_request_sends_its_result() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "d41").await;
    complete(&store, &admin, &scope, &first).await;
    let versions = [(1, 1, 1, "current", "character", "林雪", "渡口船工。")];
    write_versions(&admin, &scope, &versions).await;
    let message = "SCRIPT:search_memory 七结";
    let new = ConversationSelection::New;
    let run_id = admitted(&store, &scope, &chapter_id, "d42", message, new).await;
    let added = [(
        2,
        1,
        2,
        "current",
        "foreshadowing",
        "船头绳",
        "船头系七结绳。",
    )];
    write_versions(&admin, &scope, &added).await;
    complete(&store, &admin, &scope, &run_id).await;

    let text = [
        r#"{"type":"foreshadowing"}"#,
        r#"{"description":"船头系七结绳。","id":2,"matched":["description","text"],"title":"船头绳"}"#,
    ]
    .join("\n");
    let search: String = admin
        .query_one(
            "SELECT tool_call_id::text FROM storyos.tool_calls
              WHERE run_id = $1::text::uuid AND call_id = 'call_search_memory'",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    assert_eq!(
        (
            first_index(&admin, &run_id).await,
            calls(&admin, &run_id).await,
            read_rows(&admin, &run_id).await,
            sent(&admin, &run_id).await[1].clone(),
            outputs_record(&admin, &run_id, 1).await,
            no_public_call(&store, &admin, &scope, &run_id, &search).await,
        ),
        (
            json!([r#"{"type":"character"}
{"description":"渡口船工。","id":1,"title":"林雪"}"#]),
            json!(["completed", [settled(&text), finish()]]),
            0,
            json!([
                "decision",
                1,
                [[
                    "call_search_memory",
                    boundary("call_search_memory", "ok", &text)
                ]],
                false
            ]),
            json!([
                [["data_only", "ok"]],
                [["memory_search_result", search, "exact_required"]]
            ]),
            true,
        )
    );
}

/// Two Projects each have an entry with the term, and each Run gets the entry of its own
/// Project. A search with no match settles with one sentence. An empty query is refused with the
/// reason in one sentence, and its output stays a `tool_result` item.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_run_searches_its_own_project_and_no_match_and_a_refused_query_have_one_sentence() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "d43").await;
    complete(&store, &admin, &scope, &first).await;
    let (other, other_chapter, other_first) = queued(&store, "d45").await;
    complete(&store, &admin, &other, &other_first).await;
    let versions = [(1, 1, 1, "current", "foreshadowing", "铜铃", "铃在渡口响。")];
    write_versions(&admin, &scope, &versions).await;
    let versions = [(1, 1, 1, "current", "other", "铜铃", "铃挂在船头。")];
    write_versions(&admin, &other, &versions).await;
    let new = || ConversationSelection::New;
    let mut runs = Vec::new();
    for (scope, chapter, prefix, message) in [
        (&scope, &chapter_id, "d44", "SCRIPT:search_memory 铜铃"),
        (&other, &other_chapter, "d46", "SCRIPT:search_memory 铜铃"),
        (&scope, &chapter_id, "d47", "SCRIPT:search_memory 灯塔"),
        (
            &other,
            &other_chapter,
            "d48",
            r#"SCRIPT:refuse search_memory {"query":""}"#,
        ),
    ] {
        let run_id = admitted(&store, scope, chapter, prefix, message, new()).await;
        complete(&store, &admin, scope, &run_id).await;
        runs.push(run_id);
    }

    let refusal = CallValidationRefusal::Intake(IntakeRefusal::Query);
    let refused = calls(&admin, &runs[3]).await;
    let refused_classes: Vec<Value> = outputs_record(&admin, &runs[3], 1).await[1]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item[0].clone())
        .collect();
    assert_eq!(
        (
            calls(&admin, &runs[0]).await,
            calls(&admin, &runs[1]).await,
            calls(&admin, &runs[2]).await,
            json!([refused[0], refused[1][0]]),
            refused_classes,
        ),
        (
            json!([
                "completed",
                [
                    settled(
                        "{\"type\":\"foreshadowing\"}\n{\"description\":\"铃在渡口响。\",\"id\":1,\
                         \"matched\":[\"title\"],\"title\":\"铜铃\"}"
                    ),
                    finish()
                ]
            ]),
            json!([
                "completed",
                [
                    settled(
                        "{\"type\":\"other\"}\n{\"description\":\"铃挂在船头。\",\"id\":1,\
                         \"matched\":[\"title\"],\"title\":\"铜铃\"}"
                    ),
                    finish()
                ]
            ]),
            json!([
                "completed",
                [
                    settled(
                        "No current entry of the Memory has this exact text, and this is not a \
                         failure."
                    ),
                    finish()
                ]
            ]),
            json!([
                "completed",
                [
                    "search_memory",
                    "refused",
                    refusal.label(),
                    refusal.result_text(),
                    true
                ]
            ]),
            vec![json!("tool_result"), json!("tool_result")],
        )
    );
}

/// The message has the compaction prefix of the fake before the search script. The summary
/// request continues the response of the search, and the later request is a full input: both
/// send the exact result of the search, because only the result of a research Tool can be a
/// stub.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_compaction_after_a_search_keeps_the_exact_result() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "d49").await;
    complete(&store, &admin, &scope, &first).await;
    let versions = [(1, 1, 1, "current", "relationship", "林舟与林雪", "姐妹。")];
    write_versions(&admin, &scope, &versions).await;
    let message = format!(
        "{ACTIVE_COMPACTION_REQUEST_PREFIX} {ACTIVE_COMPACTION_POSITIONS_PREFIX} 1. \
         SCRIPT:search_memory 姐妹"
    );
    let new = ConversationSelection::New;
    let run_id = admitted(&store, &scope, &chapter_id, "d4a", &message, new).await;
    complete(&store, &admin, &scope, &run_id).await;

    let text = "{\"type\":\"relationship\"}\n{\"description\":\"姐妹。\",\"id\":1,\
                \"matched\":[\"description\",\"text\"],\"title\":\"林舟与林雪\"}";
    let output = json!([[
        "call_search_memory",
        boundary("call_search_memory", "ok", text)
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
