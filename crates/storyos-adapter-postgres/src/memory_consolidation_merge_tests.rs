//! The merge, the delete marks, the entries of the author, the mark `unused`, and the conflict
//! flag in a consolidation pass (ADR 0094 sections 6, 7, and 8). Each test starts a pass with a
//! direct call, and each pass Run and each Run of the author completes. The lead of the tests of
//! S7-D4b is `62`, and the lead of the tests of the conflict flag (S7-D4b2) is `80`.

use serde_json::{Value, json};
use storyos_application::{
    ConversationSelection, MemoryCaptureSource, MemoryConsolidationStore, ProjectScope,
    StartedMemoryConsolidation, list_memory_entries,
};
use tokio_postgres::Client;

use super::tests::{
    capture, create, delete, guarded, run_pass, scripted, settled, update, versions,
};
use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::stores;
use crate::agent_run_finish_rule::tests::complete;
use crate::agent_run_load_skill::tests::calls;
use crate::create_agent_run::context::chapter_list_tests::admitted;
use crate::memory_capture::tests::{project, run_capture};
use crate::memory_entries::tests::archived_rows;

/// One entry for `seed`: the number, the type, the origin, and the title.
type Seed<'a> = (u64, &'a str, &'a str, &'a str);

/// Writes version 1 of each entry of `rows` with SQL, at the revision of its number, and moves
/// the head of the Project to the last row. A background entry has one source of its origin. The
/// origin `author_delete` gives a deleted entry of the author, which is a delete mark. The cause
/// kind `seed` is a test value.
async fn seed(admin: &Client, scope: &ProjectScope, rows: &[Seed<'_>]) {
    let rows: Vec<Value> = rows
        .iter()
        .map(|(number, entry_type, origin, title)| json!([number, entry_type, origin, title]))
        .collect();
    admin
        .batch_execute(&format!(
            "WITH rows AS (
                 SELECT (row ->> 0)::numeric AS number, row ->> 1 AS entry_type,
                        row ->> 2 AS origin, row ->> 3 AS title, md5('{project}:' || (row ->> 0))
                        AS id
                   FROM jsonb_array_elements('{rows}'::jsonb) AS row),
             entries AS (
                 INSERT INTO storyos.memory_entry_versions
                     (owner_user_id, project_id, memory_entry_id, entry_number, version,
                      memory_revision, state, entry_type, origin, title, description, text,
                      cause, source_references)
                 SELECT '{owner}', '{project}', id::uuid, number, 1, number,
                        CASE origin WHEN 'author_delete' THEN 'deleted' ELSE 'current' END,
                        entry_type,
                        CASE origin WHEN 'author_delete' THEN 'author_request' ELSE origin END,
                        title, '述。', '文。',
                        jsonb_build_object('kind', CASE origin WHEN 'author_delete'
                                                   THEN 'author_delete' ELSE 'seed' END),
                        CASE origin
                          WHEN 'chapter' THEN jsonb_build_array(jsonb_build_object(
                               'kind', 'chapter', 'chapter_id', id::uuid))
                          WHEN 'conversation' THEN jsonb_build_array(jsonb_build_object(
                               'kind', 'conversation', 'conversation_id', id::uuid))
                          ELSE '[]' END
                   FROM rows)
             INSERT INTO storyos.project_memory_heads
             SELECT '{owner}', '{project}', max(number), max(number) + 1 FROM rows",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            rows = Value::from(rows),
        ))
        .await
        .unwrap();
}

/// A merge of the entries `entries` into a result with the title `title`.
fn merge(entries: Value, title: &str) -> Value {
    json!({
        "action": "merge", "entry": null, "entries": entries, "type": null, "title": title,
        "description": "合述。", "text": "合文。", "observations": [1], "reason": null
    })
}

/// A conflict flag of the entry `entry` with the reason `reason` and the observation 1.
fn flag(entry: u64, reason: &str) -> Value {
    json!({
        "action": "flag_conflict", "entry": entry, "entries": null, "type": null, "title": null,
        "description": null, "text": null, "observations": [1], "reason": reason
    })
}

/// Each version of each entry, in the order of the numbers and the versions: the number, the
/// version, the Memory revision, the state, the origin, the title, the description, the text, the
/// cause kind, the reason of the flag, and the source kind of the flag.
async fn flag_lines(admin: &Client, scope: &ProjectScope) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_agg(jsonb_build_array(entry_number, version, memory_revision, state,
                      origin, title, description, text, cause ->> 'kind', flag_reason,
                      flag_source ->> 'kind') ORDER BY entry_number, version)::text
               FROM storyos.memory_entry_versions WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    version_lines(&serde_json::from_str(row.get(/*idx*/ 0)).unwrap())
}

/// Captures one observation with the script lines of `operations` and runs one pass.
async fn pass_on(
    (store, admin): (&PostgresProjectReader, &Client),
    scope: &ProjectScope,
    source: &MemoryCaptureSource,
    operations: &[Value],
) -> StartedMemoryConsolidation {
    let observations = json!([scripted("脚本", operations)]);
    capture(store, admin, scope, source, observations).await;
    run_pass(store, admin, scope).await
}

/// Each version of `versions` as one line of its values, with the source kinds joined by commas.
fn version_lines(versions: &Value) -> Value {
    let text = |value: &Value| match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .map(|item| item.as_str().unwrap())
            .collect::<Vec<_>>()
            .join(","),
        value => value.to_string(),
    };
    let rows = versions.as_array().unwrap().iter();
    let lines = rows.map(|row| {
        row.as_array()
            .unwrap()
            .iter()
            .map(text)
            .collect::<Vec<_>>()
            .join(" ")
    });
    lines.collect()
}

/// The source class and the lines of each item of the Context Assembly of the pass `pass_id`.
async fn input(admin: &Client, pass_id: &str) -> Vec<(String, Vec<Value>)> {
    let rows = admin
        .query(
            "SELECT item ->> 'source_class', item ->> 'content'
               FROM storyos.memory_consolidation_passes AS pass
               JOIN storyos.context_assembly_manifests AS manifest USING (run_id),
                    jsonb_array_elements(manifest.payload -> 'selected') WITH ORDINALITY
                      AS selected(item, number)
              WHERE pass.pass_id = $1::text::uuid AND manifest.manifest_role = 'decision'
              ORDER BY number",
            &[&pass_id],
        )
        .await
        .unwrap();
    rows.iter()
        .map(|row| {
            let lines = row.get::<_, &str>(1).lines().map(serde_json::from_str);
            (row.get(0), lines.collect::<Result<_, _>>().unwrap())
        })
        .collect()
}

/// The title, the description, and the text of the current version of the entry 1.
async fn first_entry(admin: &Client, scope: &ProjectScope) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_build_array(title, description, text)::text
               FROM storyos.memory_entry_versions
              WHERE project_id = $1::text::uuid AND entry_number = 1
              ORDER BY version DESC LIMIT 1",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get(/*idx*/ 0)).unwrap()
}

/// The entries of the author fill the Index after three background entries of one type. A
/// create is refused, the merge of the three entries makes room, and a later create of the same
/// list is applied. The result has the sources of each merged entry and of its observation, and
/// each other entry has a deleted version with its own sources.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_merge_of_three_entries_keeps_their_sources_and_makes_room() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "620").await;
    let titles: Vec<String> = (4..=300).map(|number| format!("条{number}")).collect();
    let mut rows = vec![
        (1, "character", "chapter", "林舟"),
        (2, "character", "conversation", "阿舟"),
        (3, "character", "chapter", "舟子"),
    ];
    rows.extend(
        (4..)
            .zip(&titles)
            .map(|(n, t)| (n, "other", "author_request", t.as_str())),
    );
    seed(&admin, &scope, &rows).await;
    let operations = [
        create("other", "桥", json!([1])),
        merge(json!([1, 2, 3]), "林舟"),
        create("other", "桥", json!([1])),
    ];
    let started = pass_on((&store, &admin), &scope, &source, &operations).await;

    assert_eq!(
        (
            settled(&admin, &started.run_id).await,
            version_lines(&versions(&admin, &scope).await)
        ),
        (
            json!([
                "completed",
                "applied",
                null,
                [
                    ["refused", "memory_index_full", null],
                    ["applied", null, 1],
                    ["applied", null, 301]
                ],
                ["consumed"]
            ]),
            json!([
                "1 2 301 current character chapter 林舟 合文。 consolidation \
                 chapter,conversation,chapter,conversation",
                "2 2 302 deleted character conversation 阿舟  consolidation conversation",
                "3 2 303 deleted character chapter 舟子  consolidation chapter",
                "301 1 304 current other conversation 桥 文。 consolidation conversation",
            ])
        )
    );
}

/// One list with each refusal of a merge: an entry of the author, two types, and an entry that
/// an update of the same list names. Only the update writes a version.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_refusal_of_a_merge_refuses_only_its_operation() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "621").await;
    let rows = [
        (1, "character", "author_request", "阿青"),
        (2, "character", "chapter", "林舟"),
        (3, "other", "chapter", "渡口"),
        (4, "other", "chapter", "桥"),
        (5, "other", "chapter", "灯"),
        (6, "character", "chapter", "阿舟"),
    ];
    seed(&admin, &scope, &rows).await;
    let operations = [
        merge(json!([2, 1]), "林舟"),
        merge(json!([6, 3]), "阿舟"),
        update(4, "新桥。"),
        merge(json!([4, 5]), "桥"),
    ];
    let started = pass_on((&store, &admin), &scope, &source, &operations).await;
    let refused = |label: &str| json!(["refused", label, null]);

    assert_eq!(
        (
            settled(&admin, &started.run_id).await[3].clone(),
            version_lines(&versions(&admin, &scope).await)
        ),
        (
            json!([
                refused("author_entry"),
                refused("merge_type_mismatch"),
                ["applied", null, 4],
                refused("entry_named_twice")
            ]),
            json!(["4 2 7 current other chapter 林舟 新桥。 consolidation chapter,conversation"])
        )
    );
}

/// The input of the pass ends with the `memory_delete_marks` item. A create, an update, and a
/// merge that give an entry the type and the title of the mark are refused, with no regard to
/// white space at the two ends.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_delete_mark_refuses_a_create_an_update_and_a_merge() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "622").await;
    let rows = [
        (1, "other", "author_delete", "旧灯"),
        (2, "other", "chapter", "条2"),
        (3, "other", "chapter", "条3"),
        (4, "other", "chapter", "条4"),
    ];
    seed(&admin, &scope, &rows).await;
    let mut renamed = update(2, "文。");
    renamed["title"] = json!("旧灯 ");
    let operations = [
        create("other", " 旧灯", json!([1])),
        renamed,
        merge(json!([3, 4]), "旧灯"),
    ];
    let started = pass_on((&store, &admin), &scope, &source, &operations).await;
    let refused = json!(["refused", "deleted_by_author", null]);

    assert_eq!(
        (
            input(&admin, &started.pass_id).await.pop(),
            settled(&admin, &started.run_id).await[3].clone(),
            versions(&admin, &scope).await
        ),
        (
            Some((
                "memory_delete_marks".to_owned(),
                vec![json!({ "type": "other", "title": "旧灯", "description": "述。" })]
            )),
            json!([refused, refused, refused]),
            json!([])
        )
    );
}

/// Each operation kind on the entry of the author is refused in its own pass, but for the conflict
/// flag, and its title, description, and text stay the same. The 300 entries of the author fill
/// the Index, so a create of a new title is refused. No pass writes a record of AP-22.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn no_pass_changes_an_entry_of_the_author_and_its_entries_fill_the_index() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "623").await;
    let titles: Vec<String> = (1..=300).map(|number| format!("条{number}")).collect();
    let rows: Vec<Seed<'_>> = (1..)
        .zip(&titles)
        .map(|(number, title)| (number, "other", "author_request", title.as_str()))
        .collect();
    seed(&admin, &scope, &rows).await;
    let before = (
        first_entry(&admin, &scope).await,
        guarded(&admin, &scope).await,
    );
    // One pass for each kind, so that each kind meets the entry of the author alone.
    let kinds = [
        create("other", "条1", json!([1])),
        update(1, "新文。"),
        merge(json!([2, 1]), "条1"),
        delete(1),
        create("other", "桥", json!([1])),
        flag(1, "与第三章不符。"),
    ];
    let mut results = Vec::new();
    for kind in kinds {
        let observations = json!([scripted("脚本", &[kind])]);
        capture(&store, &admin, &scope, &source, observations).await;
        let started = run_pass(&store, &admin, &scope).await;
        let pass = settled(&admin, &started.run_id).await;
        results.push(pass[3][0].clone());
        // A refused observation returns one time, and the second refusal consumes it.
        if pass[4].as_array().unwrap().contains(&json!("waiting")) {
            run_pass(&store, &admin, &scope).await;
        }
    }
    let refused = |label: &str| json!(["refused", label, null]);

    assert_eq!(
        (
            results,
            (
                first_entry(&admin, &scope).await,
                guarded(&admin, &scope).await
            ),
        ),
        (
            vec![
                refused("memory_entry_exists"),
                refused("author_entry"),
                refused("author_entry"),
                refused("author_entry"),
                refused("memory_index_full"),
                json!(["applied", null, 1])
            ],
            before
        )
    );
}

/// The test administrator moves the stored times back. Only the background entry with an old
/// change and no read gets the mark `unused`: a read of a Run of the author 89 days ago, a change
/// 89 days ago, and the origin `author_request` give no mark. No test waits.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_input_marks_an_old_background_entry_with_no_recent_read() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, _) = project(&store, &admin, "624").await;
    let rows = [
        (1, "other", "chapter", "旧"),
        (2, "other", "chapter", "读"),
        (3, "other", "author_request", "阿青"),
        (4, "other", "conversation", "新"),
    ];
    seed(&admin, &scope, &rows).await;
    let message = "SCRIPT:read_memory_entry 2";
    let new = ConversationSelection::New;
    let read = admitted(&store, &scope, &chapter_id, "625", message, new).await;
    complete(&store, &admin, &scope, &read).await;
    admin
        .execute(
            "WITH versions AS (
                 UPDATE storyos.memory_entry_versions
                    SET recorded_at = statement_timestamp() - CASE entry_number
                          WHEN 4 THEN interval '89 days' ELSE interval '91 days' END
                  WHERE project_id = $1::text::uuid)
             UPDATE storyos.memory_entry_reads
                SET read_at = statement_timestamp() - interval '89 days'
              WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    let started = run_pass(&store, &admin, &scope).await;
    let items = input(&admin, &started.pass_id).await;
    let marks: Vec<(&Value, bool)> = items[0]
        .1
        .iter()
        .map(|line| (&line["id"], line.get("unused") == Some(&json!(true))))
        .collect();

    assert_eq!(
        (items[0].0.as_str(), marks),
        (
            "memory_entries",
            vec![
                (&json!(1), true),
                (&json!(2), false),
                (&json!(3), false),
                (&json!(4), false)
            ]
        )
    );
}

/// A write of the author after the start of a pass makes the pass stale, so its merge applies
/// nothing and its observation waits.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_author_write_after_the_start_makes_a_merge_stale() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "626").await;
    let rows = [
        (1, "other", "chapter", "条1"),
        (2, "other", "chapter", "条2"),
        (3, "other", "author_request", "阿青"),
    ];
    seed(&admin, &scope, &rows).await;
    let operations = [merge(json!([1, 2]), "条1")];
    capture(
        &store,
        &admin,
        &scope,
        &source,
        json!([scripted("脚本", &operations)]),
    )
    .await;
    let started = store
        .start_memory_consolidation(&scope)
        .await
        .unwrap()
        .unwrap();
    let id: String = admin
        .query_one(
            "SELECT memory_entry_id::text FROM storyos.memory_entry_versions
              WHERE project_id = $1::text::uuid AND entry_number = 3",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let deleted = crate::delete_memory_entry::tests::delete(&store, &scope, 0x627a, &id, 1).await;
    run_capture(&store, &admin, &scope, &started.run_id).await;

    assert_eq!(
        (
            deleted.is_ok(),
            settled(&admin, &started.run_id).await,
            versions(&admin, &scope).await
        ),
        (
            true,
            json!([
                "completed",
                "stale",
                null,
                [["not_applied", null, null]],
                ["waiting"]
            ]),
            json!([])
        )
    );
}

/// A first flag and a second flag of an entry of the author each write one flag version with the
/// values of the version before it and the source of the observation. A flag of a background
/// entry is refused and writes nothing. The Project Export Archive has the columns of the flag.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_flag_of_an_entry_of_the_author_writes_one_flag_version() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "800").await;
    let rows = [
        (1, "other", "author_request", "阿青"),
        (2, "other", "chapter", "渡口"),
    ];
    seed(&admin, &scope, &rows).await;
    let first = [flag(1, " 与第三章不符。 "), flag(2, "与第三章不符。")];
    let first = pass_on((&store, &admin), &scope, &source, &first).await;
    let second = pass_on((&store, &admin), &scope, &source, &[flag(1, "又不符。")]).await;
    let [archived] = archived_rows(&store, &scope, "8f", ["memory_entry_versions"]).await;
    let mut archived: Vec<Value> = archived
        .iter()
        .filter(|row| !row["flag_reason"].is_null())
        .map(|row| {
            json!([
                row["version"],
                row["flag_reason"],
                row["flag_source"]["kind"]
            ])
        })
        .collect();
    archived.sort_by_key(Value::to_string);

    assert_eq!(
        (
            settled(&admin, &first.run_id).await[3].clone(),
            settled(&admin, &second.run_id).await[3].clone(),
            flag_lines(&admin, &scope).await,
            archived
        ),
        (
            json!([
                ["applied", null, 1],
                ["refused", "flag_needs_author_entry", null]
            ]),
            json!([["applied", null, 1]]),
            json!([
                "1 1 1 current author_request 阿青 述。 文。 seed null null",
                "1 2 3 current author_request 阿青 述。 文。 conflict_flag 与第三章不符。 \
                 conversation",
                "1 3 4 current author_request 阿青 述。 文。 conflict_flag 又不符。 conversation",
                "2 1 2 current chapter 渡口 述。 文。 seed null null",
            ]),
            vec![
                json!([2, "与第三章不符。", "conversation"]),
                json!([3, "又不符。", "conversation"])
            ]
        )
    );
}

/// The time of the last change of a flagged entry is the time of the version before the flag in
/// the input of the next pass, in the list of the settings page, and in the result of a read.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_time_of_the_last_change_skips_the_flag_version() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "802").await;
    seed(&admin, &scope, &[(1, "other", "author_request", "阿青")]).await;
    let time = "2026-01-02T03:04:05.006Z";
    admin
        .execute(
            "UPDATE storyos.memory_entry_versions SET recorded_at = $2::text::timestamptz
              WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref(), &time],
        )
        .await
        .unwrap();
    pass_on((&store, &admin), &scope, &source, &[flag(1, "不符。")]).await;
    let next = pass_on((&store, &admin), &scope, &source, &[]).await;
    let message = "SCRIPT:read_memory_entry 1";
    let new = ConversationSelection::New;
    let read = admitted(&store, &scope, &chapter_id, "804", message, new).await;
    complete(&store, &admin, &scope, &read).await;
    let listed = list_memory_entries(&store, &scope).await.unwrap().unwrap();

    assert_eq!(
        (
            flag_lines(&admin, &scope).await[1].clone(),
            input(&admin, &next.pass_id).await[0].1[0]["changed_at"].clone(),
            listed[0].changed_at.as_str(),
            calls(&admin, &read)
                .await
                .to_string()
                .contains(&format!("changed_at: {time}"))
        ),
        (
            json!("1 2 2 current author_request 阿青 述。 文。 conflict_flag 不符。 conversation"),
            json!(time),
            time,
            true
        )
    );
}

/// A pass that a delete of the author makes stale writes no flag, and its observation waits. The
/// next pass flags the entry, and a `write_memory` change of the author settles at once and
/// writes a version with no flag.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_stale_pass_writes_no_flag_and_a_change_of_the_author_removes_the_flag() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "803").await;
    let rows = [
        (1, "other", "author_request", "阿青"),
        (2, "other", "author_request", "阿舟"),
    ];
    seed(&admin, &scope, &rows).await;
    let observations = json!([scripted("脚本", &[flag(1, "不符。")])]);
    capture(&store, &admin, &scope, &source, observations).await;
    let stale = store
        .start_memory_consolidation(&scope)
        .await
        .unwrap()
        .unwrap();
    let id: String = admin
        .query_one(
            "SELECT memory_entry_id::text FROM storyos.memory_entry_versions
              WHERE project_id = $1::text::uuid AND entry_number = 2",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let deleted = crate::delete_memory_entry::tests::delete(&store, &scope, 0x80fa, &id, 1).await;
    run_capture(&store, &admin, &scope, &stale.run_id).await;
    let flagged = run_pass(&store, &admin, &scope).await;
    let message = r#"SCRIPT:write_memory change {"entry": 1, "title": "阿青嫂"}"#;
    let new = ConversationSelection::New;
    let changed = admitted(&store, &scope, &chapter_id, "805", message, new).await;
    complete(&store, &admin, &scope, &changed).await;

    assert_eq!(
        (
            deleted.is_ok(),
            settled(&admin, &stale.run_id).await[1].clone(),
            settled(&admin, &flagged.run_id).await[3].clone(),
            flag_lines(&admin, &scope).await
        ),
        (
            true,
            json!("stale"),
            json!([["applied", null, 1]]),
            json!([
                "1 1 1 current author_request 阿青 述。 文。 seed null null",
                "1 2 4 current author_request 阿青 述。 文。 conflict_flag 不符。 conversation",
                "1 3 5 current author_request 阿青嫂 述。 文。 author_request null null",
                "2 1 2 current author_request 阿舟 述。 文。 seed null null",
                "2 2 3 deleted author_request 阿舟 述。  author_delete null null",
            ])
        )
    );
}

#[path = "memory_consolidation_conflict_tests.rs"]
mod conflict_tests;

#[path = "memory_conflict_flag_show_tests.rs"]
mod flag_show_tests;

#[path = "memory_conflict_chapter_path_tests.rs"]
mod chapter_path_tests;

#[path = "memory_failed_work_tests.rs"]
mod failed_work_tests;
