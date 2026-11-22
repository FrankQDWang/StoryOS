//! The current intent of a character in the Runs of the Project (ADR 0101 section 5): the Memory
//! Index and the reads of a Run of the author, the Index and the reads of a capture, the end of a
//! result, the items of each kind of Run, and the search. The leads of these tests are `71`, `72`,
//! and `73`. Each test completes each Run that it queued.

use serde_json::{Value, json};
use tokio_postgres::Client;

use super::tests::{change, character, consolidate, run_intent};
use crate::shared_tests::agent_run_finish_rule::{calls, complete};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::chapter_list_context::admitted;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::memory_capture::{project, run_capture, start};
use crate::shared_tests::memory_capture_loop::{record, script};
use crate::shared_tests::memory_conflict_flag_show::{run, write};

/// The intent of the fake reply of an intent pass, and its question.
const INTENT: &str = "我明天一早去渡口，看看那条船还在不在。";
const QUESTION: &str = "我以前去过河对岸吗？";
/// The result of a search with no match.
const NO_MATCH: &str =
    "No current entry of the Memory has this exact text, and this is not a failure.";

/// The `memory_index` text of each decision RunStep of the Run `run_id`, in order.
async fn indexes(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
        "SELECT jsonb_agg((SELECT item ->> 'content'
                             FROM jsonb_array_elements(manifest.payload -> 'selected') AS item
                            WHERE item ->> 'source_class' = 'memory_index')
                          ORDER BY manifest.decision_position)::text
           FROM storyos.context_assembly_manifests AS manifest
          WHERE manifest.run_id = $1::text::uuid AND manifest.manifest_role = 'decision'",
        &[&run_id],
    )
    .await
}

/// The field `"intent"` of the line of the entry 1 in the Index of the first RunStep of the Run
/// `run_id`, or null.
async fn intent_of_entry_1(admin: &Client, run_id: &str) -> Value {
    let index = indexes(admin, run_id).await;
    let lines = index[0].as_str().unwrap().lines();
    let line = (lines.map(|line| serde_json::from_str::<Value>(line).unwrap()))
        .find(|line| line["id"] == 1)
        .unwrap();
    line["intent"].clone()
}

/// The result text of each call of `function` of the Run `run_id`, in order.
async fn results_of(admin: &Client, run_id: &str, function: &str) -> Vec<Value> {
    (calls(admin, run_id).await.into_iter())
        .filter(|call| call[1] == function)
        .map(|call| call[4].clone())
        .collect()
}

/// The complete text of the result of a read, with the time of the line `changed_at` replaced.
fn timeless(result: &Value) -> String {
    (result.as_str().unwrap().split('\n'))
        .map(|line| match line.starts_with("changed_at: ") {
            true => "changed_at: <time>",
            false => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// For each selected item of each decision RunStep of the Run `run_id`: its source class, and
/// whether its content has the intent and the question. Each distinct row once.
async fn items(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
        "SELECT jsonb_agg(DISTINCT jsonb_build_array(item ->> 'source_class',
                                                     strpos(item ->> 'content', $2) > 0,
                                                     strpos(item ->> 'content', $3) > 0))::text
           FROM storyos.context_assembly_manifests AS manifest,
                jsonb_array_elements(manifest.payload -> 'selected') AS item
          WHERE manifest.run_id = $1::text::uuid AND manifest.manifest_role = 'decision'",
        &[&run_id, &INTENT, &QUESTION],
    )
    .await
}

/// Moves the Run `run_id` of the author between `queued` and `paused`, so that a claim does not
/// take it while an intent pass runs.
async fn set_status(admin: &Client, run_id: &str, status: &str) {
    admin
        .execute(
            "UPDATE storyos.agent_runs SET status = $2::text, lease_expires_at = NULL
              WHERE run_id = $1::text::uuid",
            &[&run_id, &status],
        )
        .await
        .unwrap();
}

/// The `write_memory` create of the part of the author of 林舟 with `text`.
fn author_part(action: &str, text: &str) -> String {
    let entry = match action {
        "create" => Value::Null,
        _ => json!(2),
    };
    let arguments = json!({
        "entry": entry, "type": "character", "title": "林舟", "description": "船夫。",
        "text": text
    });
    write(action, arguments)
}

/// The Index of each RunStep of a Run of the author has the field `"intent"` in the line of the
/// entry of the background work with a current result, and no such field in the line of the entry
/// of the author. A read of that entry has the line `intent`. The Index and the read of a capture
/// of the same Project have no intent. No complete read result has the question.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_run_of_the_author_gets_the_current_intent_and_a_capture_does_not() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "720").await;
    let project = (&scope, chapter_id.as_str());
    consolidate(
        &store,
        &admin,
        &scope,
        &source,
        &[character("林舟", "林舟是船夫。")],
    )
    .await;
    run(
        &store,
        &admin,
        project,
        "721",
        &author_part("create", "他怕水。"),
    )
    .await;
    run_intent(&store, &admin, &scope).await;
    let author = run(
        &store,
        &admin,
        project,
        "722",
        "Read.\nSCRIPT:read_memory_entry 1,2",
    )
    .await;
    let reads = [
        ("read_memory_entry", json!({"entry": 1})),
        record(json!([])),
    ];
    let capture = start(&store, &scope, &source, &script(&reads))
        .await
        .unwrap();
    run_capture(&store, &admin, &scope, &capture.run_id).await;

    let index = |intent: Option<&str>| {
        let intent = intent.map_or(String::new(), |intent| format!(",\"intent\":\"{intent}\""));
        json!(format!(
            "{{\"type\":\"character\"}}\n\
             {{\"description\":\"船夫。\",\"id\":1{intent},\"title\":\"林舟\"}}\n\
             {{\"author_part\":true,\"description\":\"船夫。\",\"id\":2,\"title\":\"林舟\"}}"
        ))
    };
    let read = |origin: &str, intent: Option<&str>, text: &str| {
        let intent = intent.map_or(String::new(), |intent| format!("\nintent: {intent}"));
        format!(
            "type: character\ntitle: 林舟\ndescription: 船夫。\norigin: {origin}\n\
             changed_at: <time>{intent}\n\n{text}"
        )
    };
    let author_reads = results_of(&admin, &author, "read_memory_entry").await;
    let capture_reads = results_of(&admin, &capture.run_id, "read_memory_entry").await;
    assert_eq!(
        (
            indexes(&admin, &author).await,
            author_reads.iter().map(timeless).collect::<Vec<_>>(),
            indexes(&admin, &capture.run_id).await[0].clone(),
            capture_reads.iter().map(timeless).collect::<Vec<_>>(),
        ),
        (
            json!(vec![index(Some(INTENT)); 3]),
            vec![
                read("conversation", Some(INTENT), "林舟是船夫。"),
                read("author_request", None, "他怕水。")
            ],
            index(None),
            vec![read("conversation", None, "林舟是船夫。")],
        )
    );
}

/// The Index of a Run that pinned its Memory revision before the result has no intent in each
/// RunStep, and the Index of a Run that pinned it after the result has the intent in each RunStep.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_runstep_gets_the_intents_of_the_pinned_revision() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "723").await;
    let create = [character("林舟", "林舟是船夫。")];
    consolidate(&store, &admin, &scope, &source, &create).await;
    let new = storyos_application::ConversationSelection::New;
    let before = admitted(&store, &scope, &chapter_id, "724", "Help.", new).await;
    set_status(&admin, &before, "paused").await;
    run_intent(&store, &admin, &scope).await;
    set_status(&admin, &before, "queued").await;
    complete(&store, &admin, &scope, &before).await;
    let after = run(&store, &admin, (&scope, &chapter_id), "725", "Help.").await;

    let line = |intent: &str| {
        json!(format!(
            "{{\"type\":\"character\"}}\n{{\"description\":\"船夫。\",\"id\":1{intent},\"title\":\"林舟\"}}"
        ))
    };
    let with = line(&format!(",\"intent\":\"{INTENT}\""));
    assert_eq!(
        (
            indexes(&admin, &before).await,
            indexes(&admin, &after).await
        ),
        (json!([line(""), line("")]), json!([with, with])),
    );
}

/// After a `write_memory` call created, changed, or deleted the entry of the author of the
/// character, the Index of a new Run has no intent for the character until a new pass records a
/// result.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_write_of_the_author_part_ends_the_intent_until_a_new_pass() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "730").await;
    let project = (&scope, chapter_id.as_str());
    let create = [character("林舟", "林舟是船夫。")];
    consolidate(&store, &admin, &scope, &source, &create).await;
    let mut intents = Vec::new();
    for (prefixes, message) in [
        (["731", "732"], author_part("create", "他怕水。")),
        (["733", "734"], author_part("change", "他会游泳。")),
        (["735", "736"], write("delete", json!({"entry": 2}))),
    ] {
        run_intent(&store, &admin, &scope).await;
        // The Run of the write pinned the Index before its write.
        let writer = run(&store, &admin, project, prefixes[0], &message).await;
        let later = run(&store, &admin, project, prefixes[1], "Help.").await;
        intents.push([
            intent_of_entry_1(&admin, &writer).await,
            intent_of_entry_1(&admin, &later).await,
        ]);
    }

    let intent = json!(INTENT);
    assert_eq!(intents, vec![[intent.clone(), Value::Null]; 3],);
}

/// After a consolidation pass changed the entry of the background work, the Index of a new Run
/// has no intent for the character until a new pass records a result.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_consolidation_change_ends_the_intent_until_a_new_pass() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "73a").await;
    let project = (&scope, chapter_id.as_str());
    let create = [character("林舟", "林舟是船夫。")];
    consolidate(&store, &admin, &scope, &source, &create).await;
    run_intent(&store, &admin, &scope).await;
    let before = run(&store, &admin, project, "73b", "Help.").await;
    let update = [change(1, "林舟", "林舟换了船。")];
    consolidate(&store, &admin, &scope, &source, &update).await;
    let changed = run(&store, &admin, project, "73c", "Help.").await;
    run_intent(&store, &admin, &scope).await;
    let again = run(&store, &admin, project, "73d", "Help.").await;

    assert_eq!(
        [
            intent_of_entry_1(&admin, &before).await,
            intent_of_entry_1(&admin, &changed).await,
            intent_of_entry_1(&admin, &again).await,
        ],
        [json!(INTENT), Value::Null, json!(INTENT)]
    );
}

/// For a Project with a current result, only the Memory Index of a Run of the author has the
/// intent, and no item of a Run of each kind has the question: the items of a Run of the author,
/// of a capture, of a consolidation pass, and of an intent pass. A search for a term that only the
/// intent or the question holds has no match.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn no_run_gets_the_question_and_only_a_run_of_the_author_gets_the_intent() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "710").await;
    let project = (&scope, chapter_id.as_str());
    let create = [character("林舟", "林舟是船夫。")];
    consolidate(&store, &admin, &scope, &source, &create).await;
    run_intent(&store, &admin, &scope).await;
    let question = run(
        &store,
        &admin,
        project,
        "711",
        "Find.\nSCRIPT:search_memory 河对岸",
    )
    .await;
    let intent = run(
        &store,
        &admin,
        project,
        "712",
        "Find.\nSCRIPT:search_memory 那条船",
    )
    .await;
    let capture = start(&store, &scope, &source, &script(&[record(json!([]))]))
        .await
        .unwrap();
    run_capture(&store, &admin, &scope, &capture.run_id).await;
    let update = [change(1, "林舟", "林舟换了船。")];
    let pass = consolidate(&store, &admin, &scope, &source, &update).await;
    let second = run_intent(&store, &admin, &scope).await;

    let mut searches = results_of(&admin, &question, "search_memory").await;
    searches.extend(results_of(&admin, &intent, "search_memory").await);
    assert_eq!(
        (
            items(&admin, &question).await,
            items(&admin, &capture.run_id).await,
            items(&admin, &pass).await,
            items(&admin, &second).await,
            searches,
        ),
        (
            json!([
                ["author_instruction", false, false],
                ["chapter_list", false, false],
                ["memory_index", true, false],
                ["memory_search_result", false, false],
                ["tool_result", false, false],
                ["working_target", false, false]
            ]),
            json!([
                ["chapter_list", false, false],
                ["memory_capture_input", false, false],
                ["memory_index", false, false]
            ]),
            json!([
                ["memory_entries", false, false],
                ["memory_observations", false, false]
            ]),
            json!([["character_file", false, false]]),
            vec![json!(NO_MATCH); 2],
        )
    );
}
