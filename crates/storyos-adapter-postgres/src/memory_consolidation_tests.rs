//! A consolidation pass in the store (ADR 0094 sections 6 and 8): the start with a direct call, the
//! request, a response with no call, each operation, a list with refused and valid operations, an
//! empty list, and each refusal. Each pass Run completes, and no pass writes a record of AP-22.

use serde_json::{Value, json};
use storyos_application::MemoryCaptureSource;

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::memory_capture::project;
use crate::shared_tests::memory_capture_loop::requests;
use crate::shared_tests::memory_consolidation::{
    capture, create, delete, guarded, run_pass, scripted, settled, update, versions,
};
use crate::shared_tests::memory_entries::{archived_rows, write_versions};

/// A started pass pins the Memory revision and has one RunStep whose Context Assembly holds the
/// current entries and the waiting observations. Its one request requires the one output Tool, and
/// the empty list of the fake consumes each observation and changes no entry.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_pass_sends_one_request_and_an_empty_list_consumes_the_observations() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "601").await;
    let observations = json!([
        { "type": "character", "title": "林舟", "text": "沉默的船夫。" },
        { "type": "other", "title": "渡口", "text": "渡口在河边。" }
    ]);
    capture(&store, &admin, &scope, &source, observations).await;
    write_versions(
        &admin,
        &scope,
        &[(1, 1, 1, "current", "other", "阿青", "船夫")],
    )
    .await;
    let before = guarded(&admin, &scope).await;
    let started = run_pass(&store, &admin, &scope).await;
    let mut rows: Value = json_of(
        &admin,
        "SELECT jsonb_build_array(pass.memory_revision, jsonb_array_length(pass.observations),
                      run.run_kind,
                      (SELECT jsonb_agg(jsonb_build_array(item ->> 'source_class',
                                item ->> 'projection_mode', item ->> 'content') ORDER BY number)
                         FROM storyos.context_assembly_manifests AS manifest,
                              jsonb_array_elements(manifest.payload -> 'selected')
                                WITH ORDINALITY AS selected(item, number)
                        WHERE manifest.run_id = run.run_id AND manifest.manifest_role = 'decision'),
                      (SELECT count(*) FROM storyos.model_attempts
                        WHERE run_id = run.run_id AND attempt_role = 'decision'))::text
               FROM storyos.memory_consolidation_passes AS pass
               JOIN storyos.agent_runs AS run USING (run_id)
              WHERE pass.pass_id = $1::text::uuid",
        &[&started.pass_id],
    )
    .await;
    let MemoryCaptureSource::Conversation {
        conversation_id, ..
    } = source
    else {
        panic!("the source is a Conversation")
    };
    let lines = |item: &Value| -> Vec<Value> {
        let content = item[2].as_str().unwrap();
        content
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    };
    let items = rows[3].take();
    let [entries, observations] = [&items[0], &items[1]].map(lines);

    assert_eq!(
        (
            rows,
            items
                .as_array()
                .unwrap()
                .iter()
                .map(|item| [&item[0], &item[1]])
                .collect::<Vec<_>>(),
            entries[0]["changed_at"].is_string(),
        ),
        (
            json!([1, 2, "memory_consolidation", null, 1]),
            vec![
                [&json!("memory_entries"), &json!("exact_required")],
                [&json!("memory_observations"), &json!("exact_required")]
            ],
            true
        )
    );
    assert_eq!(
        (&entries[0]["origin"], &entries[0]["id"], observations),
        (
            &json!("author_request"),
            &json!(1),
            vec![
                json!({ "observation": 1, "type": "character", "title": "林舟",
                        "text": "沉默的船夫。",
                        "source": { "kind": "conversation", "conversation_id": conversation_id } }),
                json!({ "observation": 2, "type": "other", "title": "渡口", "text": "渡口在河边。",
                        "source": { "kind": "conversation", "conversation_id": conversation_id } }),
            ]
        )
    );
    assert_eq!(
        (
            requests(&admin, &started.run_id).await,
            settled(&admin, &started.run_id).await,
            versions(&admin, &scope).await,
            guarded(&admin, &scope).await,
        ),
        (
            json!([[
                "memory_consolidation",
                "required",
                ["apply_memory_operations"]
            ]]),
            json!([
                "completed",
                "no_operation",
                null,
                [],
                ["consumed", "consumed"]
            ]),
            json!([]),
            before
        )
    );
}

/// A response with no `apply_memory_operations` call fails the pass. It applies nothing, and each
/// observation waits.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_response_with_no_call_fails_the_pass() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "602").await;
    let observation = json!([{ "type": "other", "title": "空", "text": "SCRIPT:no-call" }]);
    capture(&store, &admin, &scope, &source, observation).await;
    let started = run_pass(&store, &admin, &scope).await;

    assert_eq!(
        (
            settled(&admin, &started.run_id).await,
            versions(&admin, &scope).await
        ),
        (
            json!(["completed", "failed", "no_operation_call", [], ["waiting"]]),
            json!([])
        )
    );
}

/// A create writes version 1 with the next number and the origin and the source of its first
/// observation. An update and a delete write the next version with the cause `consolidation`, and
/// a refused operation between them does not stop the list. Each pass consumes its observations.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_operation_writes_its_version_and_a_refused_operation_does_not_stop_the_list() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "603").await;
    let revision_id: String = admin
        .query_one(
            "SELECT revision_id::text FROM storyos.authoritative_revisions
              WHERE manuscript_object_id = $1::text::uuid LIMIT 1",
            &[&chapter_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let first = [
        create("character", "林舟", json!([1])),
        create("foreshadowing", "灯", json!([2, 1])),
    ];
    let observations = json!([scripted("林舟", &first)]);
    capture(&store, &admin, &scope, &source, observations).await;
    let chapter = MemoryCaptureSource::Chapter {
        chapter_id,
        revision_id,
    };
    let lamp = json!([{ "type": "foreshadowing", "title": "灯", "text": "灯每夜亮一次。" }]);
    capture(&store, &admin, &scope, &chapter, lamp).await;
    let before = guarded(&admin, &scope).await;
    let created = run_pass(&store, &admin, &scope).await;
    let created = settled(&admin, &created.run_id).await;
    let second = [
        update(1, "沉默的船夫。"),
        create("other", "桥", json!([])),
        delete(2),
    ];
    let observations = json!([scripted("林舟", &second)]);
    capture(&store, &admin, &scope, &source, observations).await;
    let changed = run_pass(&store, &admin, &scope).await;

    assert_eq!(
        (
            created,
            settled(&admin, &changed.run_id).await,
            versions(&admin, &scope).await,
            guarded(&admin, &scope).await,
        ),
        (
            json!([
                "completed",
                "applied",
                null,
                [["applied", null, 1], ["applied", null, 2]],
                ["consumed", "consumed"]
            ]),
            json!([
                "completed",
                "applied",
                null,
                [
                    ["applied", null, 1],
                    ["refused", "create_without_source", null],
                    ["applied", null, 2]
                ],
                ["consumed", "consumed", "consumed"]
            ]),
            json!([
                [
                    1,
                    1,
                    1,
                    "current",
                    "character",
                    "conversation",
                    "林舟",
                    "文。",
                    "consolidation",
                    ["conversation"]
                ],
                [
                    2,
                    1,
                    2,
                    "current",
                    "foreshadowing",
                    "chapter",
                    "灯",
                    "文。",
                    "consolidation",
                    ["chapter", "conversation"]
                ],
                [
                    1,
                    2,
                    3,
                    "current",
                    "character",
                    "conversation",
                    "林舟",
                    "沉默的船夫。",
                    "consolidation",
                    ["conversation"]
                ],
                [
                    2,
                    2,
                    4,
                    "deleted",
                    "foreshadowing",
                    "chapter",
                    "灯",
                    "",
                    "consolidation",
                    ["chapter", "conversation"]
                ]
            ]),
            before
        )
    );
}

/// A null title of an update keeps the title of the entry, and the update writes the new
/// description and text. A null title of a merge keeps the title of its first entry (#1685).
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_null_title_of_an_update_or_a_merge_keeps_the_title() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "4e1").await;
    let first = [
        create("character", "林舟", json!([1])),
        create("character", "林雪", json!([1])),
        create("character", "林岚", json!([1])),
    ];
    capture(
        &store,
        &admin,
        &scope,
        &source,
        json!([scripted("林舟", &first)]),
    )
    .await;
    run_pass(&store, &admin, &scope).await;
    let mut untitled = update(1, "新文。");
    untitled["title"] = Value::Null;
    untitled["description"] = json!("沉默的船夫。");
    let merged = json!({
        "action": "merge", "entry": null, "entries": [2, 3], "type": null, "title": null,
        "description": "姐妹。", "text": "合文。", "observations": [1], "reason": null
    });
    let second = json!([scripted("林舟", &[untitled, merged])]);
    capture(&store, &admin, &scope, &source, second).await;
    let changed = run_pass(&store, &admin, &scope).await;
    let latest: Value = json_of(
        &admin,
        "SELECT jsonb_agg(jsonb_build_array(entry_number, state, title, description, text)
                      ORDER BY entry_number)::text
               FROM (SELECT DISTINCT ON (entry_number) entry_number, state, title, description, text
                       FROM storyos.memory_entry_versions WHERE project_id = $1::text::uuid
                      ORDER BY entry_number, version DESC) AS latest",
        &[&scope.project_id.as_ref()],
    )
    .await;

    assert_eq!(
        (settled(&admin, &changed.run_id).await[3].clone(), latest),
        (
            json!([["applied", null, 1], ["applied", null, 2]]),
            json!([
                [1, "current", "林舟", "沉默的船夫。", "新文。"],
                [2, "current", "林雪", "姐妹。", "合文。"],
                [3, "deleted", "林岚", "述。", ""]
            ])
        )
    );
}

/// One list with each refusal of the table, in list order, after one valid create. The Project
/// has an entry of the author and 299 current entries, so the second create fills the Index.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_refusal_of_the_table_refuses_only_its_operation() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "604").await;
    admin
        .execute(
            "INSERT INTO storyos.memory_entry_versions
                 (owner_user_id, project_id, memory_entry_id, entry_number, version,
                  memory_revision, state, entry_type, origin, title, description, text, cause,
                  source_references)
             SELECT $1::text::uuid, $2::text::uuid, md5($2 || ':' || number)::uuid, number, 1,
                    number, 'current', 'other', 'author_request', '条' || number, '述', '文',
                    jsonb_build_object('kind', 'author_request'), '[]'
               FROM generate_series(1, 299) AS number",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    admin
        .execute(
            "INSERT INTO storyos.project_memory_heads
             VALUES ($1::text::uuid, $2::text::uuid, 299, 300)",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    let mut long = create("other", "长", json!([1]));
    long["text"] = json!("长".repeat(601));
    let operations = [
        create("foreshadowing", "灯", json!([1])),
        update(1, "文。"),
        update(999, "文。"),
        delete(300),
        create("other", "桥", json!([7])),
        create("other", "桥", json!([])),
        create("unknown", "桥", json!([1])),
        create("foreshadowing", "灯", json!([1])),
        create("other", "桥", json!([1])),
    ];
    let observations = json!([
        scripted("规则一", &operations[..3]),
        scripted("规则二", &operations[3..6]),
        scripted("规则三", &operations[6..]),
        scripted("长", &[long])
    ]);
    capture(&store, &admin, &scope, &source, observations).await;
    let before = guarded(&admin, &scope).await;
    let started = run_pass(&store, &admin, &scope).await;
    let refused = |label: &str| json!(["refused", label, null]);

    assert_eq!(
        (
            settled(&admin, &started.run_id).await[3].clone(),
            versions(&admin, &scope).await.as_array().map(Vec::len),
            guarded(&admin, &scope).await,
        ),
        (
            json!([
                ["applied", null, 300],
                refused("author_entry"),
                refused("memory_entry_not_found"),
                refused("entry_named_twice"),
                refused("observation_unavailable"),
                refused("create_without_source"),
                refused("memory_entry_invalid"),
                refused("memory_entry_exists"),
                refused("memory_index_full"),
                refused("memory_entry_too_long"),
            ]),
            Some(1),
            before
        )
    );
}

/// The values of `keys` of each row of `rows`, sorted. The rows come from an archive file.
fn archived_values(rows: &[Value], keys: &[&str]) -> Value {
    let mut values: Vec<Value> = rows
        .iter()
        .map(|row| Value::Array(keys.iter().map(|key| row[*key].clone()).collect()))
        .collect();
    values.sort_by_key(Value::to_string);
    Value::Array(values)
}

/// S7-E1b: the Project Export Archive holds each consolidation pass and each Conversation Memory
/// Settings revision of the Project. No other archive test names these two tables. The lead of
/// this test is `a2`.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_archive_holds_the_passes_and_the_conversation_memory_settings() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "a24").await;
    let fact = json!([{"type": "character", "title": "林舟", "text": "船夫。"}]);
    capture(&store, &admin, &scope, &source, fact).await;
    let pass = run_pass(&store, &admin, &scope).await;
    let tables = [
        "memory_consolidation_passes",
        "conversation_memory_settings",
    ];
    let [passes, settings] = archived_rows(&store, &scope, "a2", tables).await;
    let stored = |sql: &'static str| {
        let admin = &admin;
        let project = scope.project_id.as_ref().to_owned();
        async move { json_of(admin, sql, &[&project]).await }
    };
    let stored_passes = stored(
        "SELECT jsonb_agg(jsonb_build_array(pass_id, run_id, outcome) ORDER BY pass_id)::text
           FROM storyos.memory_consolidation_passes WHERE project_id = $1::text::uuid",
    )
    .await;
    let stored_settings = stored(
        "SELECT jsonb_agg(jsonb_build_array(conversation_id, memory_settings_revision,
                  use_enabled, contribution_enabled) ORDER BY conversation_id, memory_settings_revision)::text
           FROM storyos.conversation_memory_settings WHERE project_id = $1::text::uuid",
    )
    .await;

    assert_eq!(
        (
            archived_values(&passes, &["pass_id", "run_id", "outcome"]),
            archived_values(
                &settings,
                &[
                    "conversation_id",
                    "memory_settings_revision",
                    "use_enabled",
                    "contribution_enabled"
                ]
            ),
            stored_passes[0][1].clone(),
        ),
        (stored_passes, stored_settings, json!(pass.run_id))
    );
}

/// As text, entry 10 sorts before entry 2 (#1861).
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_pass_gives_eleven_entries_in_the_order_of_their_numbers() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let scope = crate::shared_tests::command_sequence_support::seed_project(&store, "d910").await;
    let titles: Vec<String> = (1..=11).map(|number| format!("条目{number}")).collect();
    let versions: Vec<_> = titles
        .iter()
        .zip(1..=11_u64)
        .map(|(title, number)| {
            (
                number,
                1,
                number,
                "current",
                "other",
                title.as_str(),
                "说明",
            )
        })
        .collect();
    write_versions(&admin, &scope, &versions).await;

    let numbers: Vec<u64> = crate::memory_consolidation_versions::pass_entries(&admin, &scope)
        .await
        .unwrap()
        .iter()
        .map(|stored| stored.entry.entry.number)
        .collect();

    assert_eq!(numbers, (1..=11).collect::<Vec<u64>>());
}
