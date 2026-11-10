use serde_json::{Value, json};
use storyos_application::{
    ExportProjectArchiveInput, IssueProjectCommandChallenge, NoContractFaults, ProjectScope,
    ResearchSourceReadStore, VerifiedExportArchive, claim_next_archive_export,
    complete_agent_run_with_tools, complete_archive_export, get_verified_export_archive,
    issue_project_command_challenge,
};
use storyos_core::ToolEnablement;
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{RuntimeClock, claim_run, stores};
use crate::agent_run_retrieval::tests::{adapter, destination, real_run};
use crate::agent_run_tool_call::tests::set_research_tool;
use crate::agent_run_tool_dispatch::tests::{Hooked, exa_tools, mcp_replies, point_mcp, reply};
use crate::agent_run_tool_exposure::tests::{advisory_stream, sse};
use crate::command_sequence::tests::seed_project;
use crate::project_archive_build::tests::{export_command, zip_store_files};

/// A search result in the text format of the research Tool: two entries with a URL each.
const SEARCH_TEXT: &str = "Title: Liverpool tide table 1890\nURL: https://example.org/tides\n\
Published: 1890-01-01T00:00:00.000Z\nAuthor: N/A\nHighlights:\nHigh water at 10.42.\n\n---\n\n\
Title: Port log\nURL: https://example.org/port\nPublished: N/A\nAuthor: N/A\nHighlights:\n\
The port log.";

/// One response that requests each `(call_id, name, arguments)` call in order.
fn calls_stream(reference: &str, calls: &[(&str, &str, &str)]) -> String {
    let items: Vec<Value> = calls
        .iter()
        .enumerate()
        .map(|(index, (call_id, name, arguments))| {
            json!({"id": format!("fc_{index}"), "type": "function_call", "status": "completed",
                   "call_id": call_id, "name": name, "arguments": arguments})
        })
        .collect();
    let mut events = vec![json!({"type": "response.created",
        "response": {"id": reference, "status": "in_progress", "output": []}})];
    events.extend(items.iter().enumerate().map(|(index, item)| {
        json!({"type": "response.output_item.done", "output_index": index, "item": item})
    }));
    events.push(json!({"type": "response.completed", "response": {
        "id": reference, "status": "completed", "output": items,
        "usage": {"input_tokens": 90, "output_tokens": 30}}}));
    sse(&events)
}

/// Settles one Run whose first response requests `calls`. The fake MCP server answers the calls
/// in order with `texts`, and the continuation gets an Advisory.
async fn research_run(
    store: &PostgresProjectReader,
    admin: &Client,
    prefix: &str,
    suffix: u16,
    calls: &[(&str, &str, &str)],
    texts: &[&str],
) -> (ProjectScope, String) {
    let (model_base, _model) = destination(vec![
        calls_stream(&format!("resp_{prefix}"), calls),
        advisory_stream(&format!("resp_{prefix}_2")),
    ])
    .await;
    let (scope, run_id) = real_run(store, admin, prefix, &model_base).await;
    set_research_tool(store, &scope, suffix, ToolEnablement::Enabled).await;
    let replies = texts
        .iter()
        .flat_map(|text| {
            let call = reply(
                /*id*/ 0,
                json!({"content": [{"type": "text", "text": text}], "isError": false}),
            );
            mcp_replies(exa_tools(|_| {}), Some(call))
        })
        .collect();
    let (mcp_base, _mcp) = destination(replies).await;
    point_mcp(admin, &scope, &mcp_base).await;
    let claim = claim_run(admin, scope.clone(), run_id.clone()).await;
    let tools = Hooked {
        admin,
        after_prepare: String::new(),
        after_exchange: String::new(),
    };
    complete_agent_run_with_tools(
        store,
        &adapter(),
        &tools,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await
    .unwrap();
    (scope, run_id)
}

/// The native call identifier of each ToolCall of the Run, by its ToolCall identity.
async fn call_ids(admin: &Client, run_id: &str) -> std::collections::BTreeMap<String, String> {
    admin
        .query(
            "SELECT tool_call_id::text, call_id FROM storyos.tool_calls
              WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| (row.get(/*idx*/ 0), row.get(/*idx*/ 1)))
        .collect()
}

/// The exact result text of each ToolCall of the Run, by its ToolCall identity.
async fn result_texts(
    admin: &Client,
    run_id: &str,
) -> std::collections::BTreeMap<String, Option<String>> {
    admin
        .query(
            "SELECT tool_call_id::text, result_text FROM storyos.tool_calls
              WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| (row.get(/*idx*/ 0), row.get(/*idx*/ 1)))
        .collect()
}

/// The listed sources of the Run, with each ToolCall identity as its native call identifier and
/// with no volatile identity or time. A capture time must be an RFC 3339 UTC time. The span of
/// each Snapshot must hold exactly its text in the stored result text of its ToolCall, and the span
/// of a Gap reads as the result line that it names.
async fn listed(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Value {
    let calls = call_ids(admin, run_id).await;
    let results = result_texts(admin, run_id).await;
    let span = |tool_call_id: &str, offset: &Value, bytes: usize| {
        let offset: usize = offset.as_str().unwrap().parse().unwrap();
        let result = results[tool_call_id]
            .as_deref()
            .expect("a span has a result text");
        String::from_utf8(result.as_bytes()[offset..offset + bytes].to_vec()).unwrap()
    };
    let sources = store
        .list_research_sources(scope, run_id)
        .await
        .unwrap()
        .expect("the Run is in Scope");
    let stable = |value: Value, time: &str| {
        let mut value = value;
        let object = value.as_object_mut().unwrap();
        let captured = object.remove(time).unwrap();
        assert!(captured.as_str().unwrap().ends_with('Z'), "{captured}");
        object.retain(|key, _| key != "source_snapshot_id" && key != "source_gap_id");
        let call = calls[object["tool_call_id"].as_str().unwrap()].clone();
        object.insert("tool_call_id".to_owned(), json!(call));
        value
    };
    let mut snapshots = Vec::new();
    for snapshot in &sources.snapshots {
        let text = store
            .get_research_source(scope, &snapshot.source_snapshot_id)
            .await
            .unwrap()
            .expect("the Snapshot is in Scope")
            .text;
        let mut value = stable(serde_json::to_value(snapshot).unwrap(), "captured_at");
        let offset = value
            .as_object_mut()
            .unwrap()
            .remove("result_offset")
            .unwrap();
        assert_eq!(span(&snapshot.tool_call_id, &offset, text.len()), text);
        snapshots.push(value);
    }
    json!({
        "snapshots": snapshots,
        "gaps": sources.gaps.iter()
            .map(|gap| {
                let mut value = stable(serde_json::to_value(gap).unwrap(), "recorded_at");
                let object = value.as_object_mut().unwrap();
                let (offset, bytes) = (object.remove("result_offset").unwrap(),
                                       object.remove("result_bytes").unwrap());
                let line = (!offset.is_null()).then(|| {
                    span(&gap.tool_call_id, &offset, bytes.as_str().unwrap().parse().unwrap())
                });
                object.insert("result_line".to_owned(), json!(line));
                value
            })
            .collect::<Vec<_>>(),
    })
}

fn digest(text: &str) -> String {
    format!("sha256:{}", storyos_core::hex_sha256(text.as_bytes()))
}

fn snapshot(
    call: &str,
    kind: &str,
    locator: &str,
    title: Value,
    published: Value,
    text: &str,
) -> Value {
    json!({"tool_call_id": call, "kind": kind, "locator": locator, "title": title,
           "published": published,
           "content_digest": digest(text), "text_bytes": text.len().to_string()})
}

/// The Source Snapshot identities of the Run, in list order.
async fn snapshot_ids(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    run_id: &str,
) -> Vec<String> {
    store
        .list_research_sources(scope, run_id)
        .await
        .unwrap()
        .unwrap()
        .snapshots
        .into_iter()
        .map(|snapshot| snapshot.source_snapshot_id)
        .collect()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_settled_search_captures_one_snapshot_per_entry_that_its_continuation_names() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let search = r#"{"query":"tide tables of 1890","result_count":null}"#;
    let (scope, run_id) = research_run(
        &store,
        &admin,
        "5a1",
        0x5a1a,
        &[("call_01", "web_search", search)],
        &[SEARCH_TEXT],
    )
    .await;
    let ids = snapshot_ids(&store, &scope, &run_id).await;
    let read = store
        .get_research_source(&scope, &ids[0])
        .await
        .unwrap()
        .unwrap();
    // The continuation records the Snapshot identities, and its selected tool_result item
    // names them as its Context Source Version.
    let continuation: Value = admin
        .query_one(
            "SELECT jsonb_build_object(
                      'recorded', requirement.payload
                                  #> '{operation_requirement,tool_results,0,source_snapshot_ids}',
                      'versions', (SELECT jsonb_agg(DISTINCT item -> 'source_version')
                                     FROM jsonb_path_query(manifest.payload,
                                          'strict $.** ? (@.source_class == \"tool_result\")')
                                          AS item))::text
               FROM storyos.operation_requirements AS requirement
               JOIN storyos.context_assembly_manifests AS manifest
                 ON (manifest.run_id, manifest.decision_position, manifest.manifest_role) =
                    (requirement.run_id, requirement.decision_position, 'decision')
              WHERE requirement.run_id = $1::text::uuid AND requirement.decision_position = 1
                AND requirement.requirement_role = 'primary'",
            &[&run_id],
        )
        .await
        .map(|row| serde_json::from_str(row.get::<_, &str>(0)).unwrap())
        .unwrap();

    assert_eq!(
        (
            listed(&store, &admin, &scope, &run_id).await,
            read.text,
            continuation
        ),
        (
            json!({
                "snapshots": [
                    snapshot("call_01", "search_result", "https://example.org/tides",
                             json!("Liverpool tide table 1890"),
                             json!("1890-01-01T00:00:00.000Z"), "High water at 10.42."),
                    snapshot("call_01", "search_result", "https://example.org/port",
                             json!("Port log"), Value::Null, "The port log."),
                ],
                "gaps": [],
            }),
            "High water at 10.42.".to_owned(),
            json!({"recorded": ids, "versions": [ids.join(",")]}),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn metadata_above_the_store_bounds_keeps_the_settlement_and_reads_as_absent() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let search = r#"{"query":"tide tables of 1890","result_count":null}"#;
    let text = format!(
        "Title: {}\nURL: https://example.org/tides\nPublished: {}\nAuthor: N/A\nHighlights:\n\
         High water at 10.42.",
        "T".repeat(storyos_core::SOURCE_TITLE_MAX_CHARS + 1),
        "1".repeat(storyos_core::SOURCE_PUBLISHED_MAX_CHARS + 1),
    );
    let (scope, run_id) = research_run(
        &store,
        &admin,
        "5a6",
        0x5a6a,
        &[("call_01", "web_search", search)],
        &[&text],
    )
    .await;
    let state: String = admin
        .query_one(
            "SELECT state FROM storyos.tool_calls WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);

    assert_eq!(
        (state, listed(&store, &admin, &scope, &run_id).await),
        (
            "settled".to_owned(),
            json!({
                "snapshots": [snapshot("call_01", "search_result", "https://example.org/tides",
                                       Value::Null, Value::Null, "High water at 10.42.")],
                "gaps": [],
            })
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_fetch_records_a_failed_a_missing_and_an_empty_url_as_gaps_and_never_as_snapshots() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let urls = r#"{"urls":["https://example.org/a","https://example.org/c","https://example.org/d","https://example.org/e"]}"#;
    let page = "# Tide table\nURL: https://example.org/a\n\nHigh water at 10.42.\n\n\
                # Empty page\nURL: https://example.org/e\n\n\n\
                Error fetching https://example.org/c: CRAWL_NOT_FOUND";
    let (scope, run_id) = research_run(
        &store,
        &admin,
        "5a2",
        0x5a2a,
        &[("call_01", "web_fetch", urls)],
        &[page],
    )
    .await;
    let gap = |locator: &str, reason: &str, line: Option<&str>| json!({"tool_call_id": "call_01", "locator": locator, "reason": reason, "result_line": line});

    assert_eq!(
        listed(&store, &admin, &scope, &run_id).await,
        json!({
            "snapshots": [snapshot("call_01", "page", "https://example.org/a",
                                   json!("Tide table"), Value::Null, "High water at 10.42.")],
            "gaps": [
                gap("https://example.org/c", "failed",
                    Some("Error fetching https://example.org/c: CRAWL_NOT_FOUND")),
                gap("https://example.org/d", "missing", None),
                gap("https://example.org/e", "empty",
                    Some("# Empty page\nURL: https://example.org/e")),
            ],
        })
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_page_read_above_the_result_bound_fails_with_one_gap_for_each_url_and_cuts_nothing() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let urls = r#"{"urls":["https://example.org/a","https://example.org/b"]}"#;
    // One byte above the bound of the whole Tool result (S5-03), which is the bound of a source.
    let page = format!(
        "# Tide table\nURL: https://example.org/a\n\n{}",
        "x".repeat(storyos_core::TOOL_RESULT_TEXT_MAX_BYTES)
    );
    let (scope, run_id) = research_run(
        &store,
        &admin,
        "5a7",
        0x5a7a,
        &[("call_01", "web_fetch", urls)],
        &[&page],
    )
    .await;
    let state: String = admin
        .query_one(
            "SELECT state FROM storyos.tool_calls WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let gap = |locator: &str| {
        json!({"tool_call_id": "call_01", "locator": locator, "reason": "failed",
                               "result_line": null})
    };

    assert_eq!(
        (state, listed(&store, &admin, &scope, &run_id).await),
        (
            "failed".to_owned(),
            json!({
                "snapshots": [],
                "gaps": [gap("https://example.org/a"), gap("https://example.org/b")],
            })
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_page_that_quotes_a_header_is_unattributed_and_the_call_stays_settled() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let urls = r#"{"urls":["https://example.org/a"]}"#;
    let page = "# Page\nURL: https://example.org/a\n\nBefore.\n\n\
                # Quoted\nURL: https://example.org/a\n\nAfter.";
    let (scope, run_id) = research_run(
        &store,
        &admin,
        "5a8",
        0x5a8a,
        &[("call_01", "web_fetch", urls)],
        &[page],
    )
    .await;
    let state: String = admin
        .query_one(
            "SELECT state FROM storyos.tool_calls WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);

    assert_eq!(
        (state, listed(&store, &admin, &scope, &run_id).await),
        (
            "settled".to_owned(),
            json!({
                "snapshots": [],
                "gaps": [{"tool_call_id": "call_01", "locator": "https://example.org/a",
                          "reason": "unattributed", "result_line": null}],
            })
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_second_fetch_of_one_url_is_a_new_revision_and_no_snapshot_changes() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let url = r#"{"urls":["https://example.org/tides"]}"#;
    let (scope, run_id) = research_run(
        &store,
        &admin,
        "5a3",
        0x5a3a,
        &[("call_01", "web_fetch", url), ("call_02", "web_fetch", url)],
        &[
            "# Tide table\nURL: https://example.org/tides\n\nHigh water at 10.42.",
            "# Tide table\nURL: https://example.org/tides\n\nHigh water at 11.07.",
        ],
    )
    .await;
    let ids = snapshot_ids(&store, &scope, &run_id).await;
    let runtime = store.connect().await.unwrap();
    let mut refused = Vec::new();
    for statement in [
        "UPDATE storyos.source_snapshots SET text = 'changed' WHERE source_snapshot_id = $1::text::uuid",
        "DELETE FROM storyos.source_snapshots WHERE source_snapshot_id = $1::text::uuid",
        "UPDATE storyos.source_gaps SET reason = 'failed' WHERE source_gap_id = $1::text::uuid",
        "DELETE FROM storyos.source_gaps WHERE source_gap_id = $1::text::uuid",
    ] {
        // Each statement runs in the exact Project Scope, so that only the grant can refuse it.
        runtime.batch_execute("BEGIN").await.unwrap();
        crate::set_challenge_scope_on_client(&runtime, &scope)
            .await
            .unwrap();
        refused.push(
            runtime
                .execute(statement, &[&ids[0]])
                .await
                .map_err(|error| crate::sql_state(&error)),
        );
        runtime.batch_execute("ROLLBACK").await.unwrap();
    }
    let first = store
        .get_research_source(&scope, &ids[0])
        .await
        .unwrap()
        .unwrap();
    let page = |call: &str, text: &str| {
        snapshot(
            call,
            "page",
            "https://example.org/tides",
            json!("Tide table"),
            Value::Null,
            text,
        )
    };
    // The two revisions of the lineage of the URL: each with its own identity and capture time.
    let captured: Vec<String> = store
        .list_research_sources(&scope, &run_id)
        .await
        .unwrap()
        .unwrap()
        .snapshots
        .into_iter()
        .map(|snapshot| snapshot.captured_at)
        .collect();

    assert_eq!(
        (
            listed(&store, &admin, &scope, &run_id).await,
            (ids[0] != ids[1], captured[0] < captured[1]),
            first.text,
            refused
        ),
        (
            json!({
                "snapshots": [page("call_01", "High water at 10.42."),
                              page("call_02", "High water at 11.07.")],
                "gaps": [],
            }),
            (true, true),
            "High water at 10.42.".to_owned(),
            vec![Err(Some("42501".to_owned())); 4],
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_read_in_another_project_scope_finds_no_run_and_no_snapshot() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let search = r#"{"query":"tide tables of 1890","result_count":null}"#;
    let (scope, run_id) = research_run(
        &store,
        &admin,
        "5a4",
        0x5a4a,
        &[("call_01", "web_search", search)],
        &[SEARCH_TEXT],
    )
    .await;
    let ids = snapshot_ids(&store, &scope, &run_id).await;
    let other = seed_project(&store, "5af0").await;

    assert_eq!(
        (
            store.list_research_sources(&other, &run_id).await.unwrap(),
            store.get_research_source(&other, &ids[0]).await.unwrap(),
            ids.len(),
        ),
        (None, None, 2)
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_project_export_packs_the_sources_and_a_reload_reads_them_unchanged() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let urls = r#"{"urls":["https://example.org/a","https://example.org/d"]}"#;
    let (scope, run_id) = research_run(
        &store,
        &admin,
        "5a5",
        0x5a5a,
        &[("call_01", "web_fetch", urls)],
        &["# Tide table\nURL: https://example.org/a\n\nHigh water at 10.42."],
    )
    .await;
    let before = (
        store.list_research_sources(&scope, &run_id).await.unwrap(),
        store
            .get_research_source(&scope, &snapshot_ids(&store, &scope, &run_id).await[0])
            .await
            .unwrap(),
    );
    let (export, input): (_, ExportProjectArchiveInput) = export_command(&scope, "5a");
    issue_project_command_challenge(
        &store,
        &IssueProjectCommandChallenge {
            binding: export.challenge_binding.clone(),
            nonce: "nonce-export-5a".to_owned(),
            nonce_digest: export.nonce_digest.clone(),
            earlier_revision_nonces: Vec::new(),
        },
    )
    .await
    .unwrap();
    store.export_project_archive(&export, &input).await.unwrap();
    let claimed = claim_next_archive_export(&store).await.unwrap().unwrap();
    complete_archive_export(&store, &claimed).await.unwrap();
    let VerifiedExportArchive::Ready(zip) =
        get_verified_export_archive(&store, &scope, &input.export_id)
            .await
            .unwrap()
    else {
        panic!("the Project Export must return verified ZIP bytes");
    };
    let files = zip_store_files(&zip);
    let mut packed = Vec::new();
    for table in ["source_snapshots", "source_gaps"] {
        let path = format!("canonical/{table}.json");
        let bytes = &files.iter().find(|(name, _)| *name == path).unwrap().1;
        let rows: Vec<Value> = serde_json::from_slice(bytes).unwrap();
        packed.push((
            table,
            rows.len(),
            std::str::from_utf8(bytes).unwrap().to_owned(),
        ));
    }
    // A restore loads the packed rows again in place of the stored rows.
    for (table, _, rows) in &packed {
        admin
            .execute(
                &format!("DELETE FROM storyos.{table} WHERE project_id = $1::text::uuid"),
                &[&scope.project_id.as_ref()],
            )
            .await
            .unwrap();
        admin
            .execute(
                &format!(
                    "INSERT INTO storyos.{table}
                     SELECT * FROM jsonb_populate_recordset(NULL::storyos.{table}, $1::text::jsonb)"
                ),
                &[rows],
            )
            .await
            .unwrap();
    }
    let after = (
        store.list_research_sources(&scope, &run_id).await.unwrap(),
        store
            .get_research_source(&scope, &snapshot_ids(&store, &scope, &run_id).await[0])
            .await
            .unwrap(),
    );

    assert_eq!(
        (
            packed
                .iter()
                .map(|(table, count, _)| (*table, *count))
                .collect::<Vec<_>>(),
            after,
        ),
        (vec![("source_snapshots", 1), ("source_gaps", 1)], before)
    );
}
