//! The Host Tool `read_skill_resource` of a Run with a pinned catalog (ADR 0089 sections 3, 7,
//! and 9): the settled read of a file of each fixture, the request after it, each refusal, the
//! limit of 16 reads, the exact read output after a compaction, and a `read_tool_result` call of a
//! Skill call.

use std::sync::Arc;

use serde_json::{Value, json};
use storyos_application::{
    AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore, ProjectScope,
};
use storyos_core::{
    ACTIVE_COMPACTION_POSITIONS_PREFIX, ACTIVE_COMPACTION_REQUEST_PREFIX, ReadToolResultRefusal,
    SKILL_MD_PATH, SkillFileInput, SkillPackage, SkillPackageReference, SkillResourceRefusal,
    SkillSet,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_finish_rule::complete;
use crate::shared_tests::agent_run_load_skill::{TOOLS, calls, loaded_run, skill_md};
use crate::shared_tests::agent_run_steering::queued;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::create_agent_run_skill::{fixture, package, set};
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::model_gateway::Admission;

const EXPIRES: &str = "SCRIPT:step-reference-expires";

/// A package `name` with a short `SKILL.md` and `files`.
fn with_files(name: &str, files: Vec<(&str, Vec<u8>)>) -> SkillPackage {
    let skill_md = format!("---\nname: {name}\ndescription: Use it for case {name}.\n---\nRead.\n");
    let mut inputs = vec![SkillFileInput {
        path: SKILL_MD_PATH.to_owned(),
        bytes: skill_md.into_bytes(),
    }];
    inputs.extend(files.into_iter().map(|(path, bytes)| SkillFileInput {
        path: path.to_owned(),
        bytes,
    }));
    package(name, inputs)
}

fn file(package: &SkillPackage, path: &str) -> String {
    let file = package.files.iter().find(|file| file.path == path);
    String::from_utf8(file.unwrap().bytes.clone()).unwrap()
}

/// Completes the first Run of a new Project, then admits and completes one Run with `message`
/// that pins the catalog of `skills` and selects `selected`.
async fn selected_run(
    store: &PostgresProjectReader,
    admin: &Client,
    prefixes: [&str; 2],
    message: &str,
    (skills, selected): (Arc<SkillSet>, Vec<SkillPackageReference>),
) -> (ProjectScope, String) {
    let (scope, chapter_id, first) = queued(store, prefixes[0]).await;
    complete(store, admin, &scope, &first).await;
    let run_id = Admission {
        message,
        skills: (skills, selected),
        ..Default::default()
    }
    .admit(store, &scope, &chapter_id, prefixes[1])
    .await;
    complete(store, admin, &scope, &run_id).await;
    (scope, run_id)
}

/// For each Model Attempt of the Run in the order of its creation: its role, its RunStep, the
/// Tools that it declared, and the call identifier and output of each Tool output that it sent.
async fn sent(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
        "SELECT jsonb_agg(jsonb_build_array(attempt_role, decision_position, wire -> 'tools',
                      COALESCE((SELECT jsonb_agg(jsonb_build_array(output ->> 'call_id',
                                                                   output ->> 'output'))
                                  FROM jsonb_array_elements(wire -> 'tool_outputs') AS output),
                               '[]'::jsonb))
                      ORDER BY model_attempt_id)::text
               FROM storyos.model_attempts,
                    LATERAL (SELECT (payload #>> '{wire,serialized_payload}')::jsonb) AS w(wire)
              WHERE run_id = $1::text::uuid",
        &[&run_id],
    )
    .await
}

/// The read sources of the Operation Requirement and each selected `skill_resource` item of the
/// decision Context Assembly at `position`.
async fn read_record(admin: &Client, run_id: &str, position: i32) -> Value {
    json_of(
        admin,
        "SELECT jsonb_build_array(jsonb_path_query_array(payload,
                        '$.operation_requirement.tool_results[*] ? (exists (@.resource_path))'),
                      jsonb_path_query_array(payload,
                        '$.selected[*] ? (@.source_class == \"skill_resource\")'))::text
               FROM storyos.context_assembly_manifests
              WHERE run_id = $1::text::uuid AND manifest_role = 'decision'
                AND decision_position = $2::text::numeric",
        &[&run_id, &position.to_string()],
    )
    .await
}

fn settled(name: &str, text: &str) -> Value {
    json!([name, "settled", null, text, true])
}

fn refused(refusal: SkillResourceRefusal) -> Value {
    json!([
        "read_skill_resource",
        "refused",
        refusal.label(),
        refusal.result().text,
        true
    ])
}

fn finish() -> Value {
    settled("finish", "")
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_read_of_the_resource_of_each_fixture_settles_with_its_exact_text() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let comms = fixture("internal-comms");
    let dialogue = fixture("dialogue-style-guide");
    let skills = || set(vec![comms.clone(), dialogue.clone()]);
    let updates = file(&comms, "examples/3p-updates.md");
    let examples = file(&dialogue, "references/examples.md");
    // The Agent loads `internal-comms` and then reads a file of it.
    let (scope, loaded) = loaded_run(
        &store,
        &admin,
        ["221", "222"],
        "SCRIPT:read_skill_resource load internal-comms examples/3p-updates.md",
        skills(),
    )
    .await;
    // The author selected `dialogue-style-guide`, so the first response reads a file of it.
    let selection = SkillPackageReference {
        name: dialogue.name.clone(),
        package_digest: dialogue.package_digest.clone(),
    };
    let (_, selected) = selected_run(
        &store,
        &admin,
        ["223", "224"],
        "SCRIPT:read_skill_resource dialogue-style-guide references/examples.md",
        (skills(), vec![selection]),
    )
    .await;

    // Each read settles with the exact text of the file. It has no Destination Attempt and no
    // Outbound Disclosure Event, and it pins the Host Tool Registration.
    assert_eq!(
        (calls(&admin, &loaded).await, calls(&admin, &selected).await),
        (
            json!([
                "completed",
                [
                    settled("load_skill", &skill_md(&comms)),
                    settled("read_skill_resource", &updates),
                    finish(),
                ]
            ]),
            json!([
                "completed",
                [settled("read_skill_resource", &examples), finish()]
            ]),
        )
    );
    // The request after the read sends the exact text as the output of the call, with no tag.
    let load = json!(["call_load_skill_0", skill_md(&comms)]);
    let read = |text: &str| json!(["call_read_skill_resource_0", text]);
    let finish_output = json!([
        "call_finish",
        "<tool_result source=\"call_finish\" state=\"empty\">The tool answered with no \
         entry.</tool_result>"
    ]);
    assert_eq!(
        (sent(&admin, &loaded).await, sent(&admin, &selected).await),
        (
            json!([
                ["decision", 0, TOOLS, []],
                ["decision", 1, TOOLS, [load]],
                ["decision", 2, TOOLS, [load, read(&updates)]],
                ["decision", 3, TOOLS, [load, read(&updates), finish_output]],
            ]),
            json!([
                ["decision", 0, TOOLS, []],
                ["decision", 1, TOOLS, [read(&examples)]],
                ["decision", 2, TOOLS, [read(&examples), finish_output]],
            ]),
        )
    );
    // The Operation Requirement records the read with its SkillPackage Reference and its path,
    // and the output is a `skill_resource` item with the exact text.
    let ids = admin
        .query_one(
            "SELECT tool_call_id::text, model_attempt_id::text, decision_id::text
               FROM storyos.tool_calls
              WHERE run_id = $1::text::uuid AND function_name = 'read_skill_resource'",
            &[&loaded],
        )
        .await
        .unwrap();
    let (tool_call_id, attempt_id, decision_id): (String, String, String) =
        (ids.get(/*idx*/ 0), ids.get(/*idx*/ 1), ids.get(/*idx*/ 2));
    let source = json!({
        "tool_call_id": tool_call_id,
        "call_id": "call_read_skill_resource_0",
        "model_attempt_id": attempt_id,
        "decision_id": decision_id,
        "state": "ok",
        "trust": "skill_resource",
        "kept_result_tokens": null,
        "skill": {"name": "internal-comms", "package_digest": comms.package_digest},
        "resource_path": "examples/3p-updates.md",
    });
    let item = json!({
        "source_class": "skill_resource",
        "source_version": tool_call_id,
        "projection_mode": "exact_required",
        "token_count": storyos_core::count_stored_text(&updates).character_count,
        "content": updates,
    });
    assert_eq!(
        read_record(&admin, &loaded, 2).await,
        json!([[source], [item]])
    );
    // The public ToolCall list has no Host ToolCall.
    let public = store
        .read_agent_run(
            &scope,
            &loaded,
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
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_refused_read_gives_its_reason_and_the_run_continues() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let comms = fixture("internal-comms");
    let odd = with_files(
        "odd-files",
        vec![
            ("assets/mark.bin", vec![0xff, 0xfe, 0x00]),
            ("references/long.md", "a".repeat(32_001).into_bytes()),
            ("references/nul.md", b"Line one.\0Line two.".to_vec()),
        ],
    );
    let skills = || set(vec![comms.clone(), odd.clone()]);
    // A load in the same response as the read does not count.
    let (_, beside) = loaded_run(
        &store,
        &admin,
        ["225", "226"],
        "SCRIPT:read_skill_resource load_beside internal-comms examples/3p-updates.md",
        skills(),
    )
    .await;
    let (_, unknown) = loaded_run(
        &store,
        &admin,
        ["227", "228"],
        "SCRIPT:read_skill_resource no-such-skill SKILL.md",
        skills(),
    )
    .await;
    let (_, not_found) = loaded_run(
        &store,
        &admin,
        ["229", "22a"],
        "SCRIPT:read_skill_resource load internal-comms examples/missing.md \
         /examples/3p-updates.md examples/../SKILL.md examples//3p-updates.md \
         examples\\3p-updates.md",
        skills(),
    )
    .await;
    let (_, not_readable) = loaded_run(
        &store,
        &admin,
        ["22b", "22c"],
        "SCRIPT:read_skill_resource load odd-files assets/mark.bin references/long.md \
         references/nul.md",
        skills(),
    )
    .await;

    let not_loaded = refused(SkillResourceRefusal::NotLoaded);
    let missing = refused(SkillResourceRefusal::NotFound);
    let unreadable = refused(SkillResourceRefusal::NotReadable);
    let load = |package: &SkillPackage| settled("load_skill", &skill_md(package));
    assert_eq!(
        [
            calls(&admin, &beside).await,
            calls(&admin, &unknown).await,
            calls(&admin, &not_found).await,
            calls(&admin, &not_readable).await,
        ],
        [
            json!(["completed", [load(&comms), not_loaded, finish()]]),
            json!(["completed", [not_loaded, finish()]]),
            json!([
                "completed",
                [
                    load(&comms),
                    missing,
                    missing,
                    missing,
                    missing,
                    missing,
                    finish()
                ]
            ]),
            json!([
                "completed",
                [load(&odd), unreadable, unreadable, unreadable, finish()]
            ]),
        ]
    );
    // The output of a refused read stays a `tool_result` item.
    let text = SkillResourceRefusal::NotLoaded.result().text;
    assert_eq!(
        sent(&admin, &unknown).await[1][3],
        json!([[
            "call_read_skill_resource_0",
            format!(
                "<tool_result source=\"call_read_skill_resource_0\" state=\"rejected\">{text}\
                 </tool_result>"
            )
        ]])
    );
    assert_eq!(read_record(&admin, &unknown, 1).await, json!([[], []]));
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_read_of_an_empty_file_or_a_file_of_white_space_gives_its_exact_text() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let blank = with_files(
        "blank-files",
        vec![("empty.md", Vec::new()), ("blank.md", b" \n".to_vec())],
    );
    let (_, run_id) = loaded_run(
        &store,
        &admin,
        ["235", "236"],
        "SCRIPT:read_skill_resource load blank-files empty.md blank.md",
        set(vec![blank.clone()]),
    )
    .await;

    // Each read settles, and the request after the reads sends the exact text of each file as
    // a `skill_resource` item, not the tagged form of an empty result.
    assert_eq!(
        calls(&admin, &run_id).await,
        json!([
            "completed",
            [
                settled("load_skill", &skill_md(&blank)),
                settled("read_skill_resource", ""),
                settled("read_skill_resource", " \n"),
                finish(),
            ]
        ])
    );
    assert_eq!(
        sent(&admin, &run_id).await[2][3],
        json!([
            ["call_load_skill_0", skill_md(&blank)],
            ["call_read_skill_resource_0", ""],
            ["call_read_skill_resource_1", " \n"],
        ])
    );
    let record = read_record(&admin, &run_id, 2).await;
    let kept: Vec<Value> = record[0]
        .as_array()
        .unwrap()
        .iter()
        .zip(record[1].as_array().unwrap())
        .map(|(source, item)| {
            json!([
                source["resource_path"],
                source["trust"],
                item["projection_mode"],
                item["content"]
            ])
        })
        .collect();
    assert_eq!(
        kept,
        [
            json!(["empty.md", "skill_resource", "exact_required", ""]),
            json!(["blank.md", "skill_resource", "exact_required", " \n"]),
        ]
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn seventeen_reads_of_one_file_settle_sixteen_and_refuse_the_last_at_the_limit() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let notes = with_files(
        "notes",
        vec![("notes.md", b"Keep each note short.\n".to_vec())],
    );
    let paths = vec!["notes.md"; 17].join(" ");
    let (_, run_id) = loaded_run(
        &store,
        &admin,
        ["22d", "22e"],
        &format!("SCRIPT:read_skill_resource load notes {paths}"),
        set(vec![notes.clone()]),
    )
    .await;

    // The Host executes the reads of one response in call order, and each settled read counts.
    let mut expected = vec![settled("load_skill", &skill_md(&notes))];
    expected.extend(vec![
        settled(
            "read_skill_resource",
            "Keep each note short.\n"
        );
        16
    ]);
    expected.extend([refused(SkillResourceRefusal::ReadLimit), finish()]);
    assert_eq!(calls(&admin, &run_id).await, json!(["completed", expected]));
    let call_ids: Vec<Value> = sent(&admin, &run_id).await[3][3]
        .as_array()
        .unwrap()
        .iter()
        .map(|output| output[0].clone())
        .collect();
    let mut order = vec![json!("call_load_skill_0")];
    order.extend((0..17).map(|index| json!(format!("call_read_skill_resource_{index}"))));
    order.push(json!("call_finish"));
    assert_eq!(call_ids, order);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_compaction_keeps_a_long_read_output_exact_and_names_no_read_stub() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let text = "Write each scene in the present tense. ".repeat(80);
    let long = with_files("long-notes", vec![("notes.md", text.clone().into_bytes())]);
    // RunStep 0 loads the Skill, RunStep 1 reads the file, and the Run compacts at RunStep 2.
    // Each later RunStep then sends its full input.
    let message = format!(
        "{ACTIVE_COMPACTION_REQUEST_PREFIX} {ACTIVE_COMPACTION_POSITIONS_PREFIX} 2. \
         SCRIPT:read_skill_resource load long-notes notes.md\n{EXPIRES}"
    );
    let (_, run_id) = loaded_run(
        &store,
        &admin,
        ["231", "232"],
        &message,
        set(vec![long.clone()]),
    )
    .await;

    assert!(storyos_core::count_stored_text(&text).character_count > 2_000);
    let read = json!(["call_read_skill_resource_0", text]);
    let attempts = sent(&admin, &run_id).await;
    let reads: Vec<Value> = attempts
        .as_array()
        .unwrap()
        .iter()
        .map(|attempt| {
            let outputs = attempt[3].as_array().unwrap();
            json!([
                attempt[0],
                attempt[1],
                outputs
                    .iter()
                    .filter(|output| output[0] == read[0])
                    .collect::<Vec<_>>()
            ])
        })
        .collect();
    let stubs: Value = json_of(
        &admin,
        "SELECT jsonb_agg(jsonb_build_array(decision_position, stub_tool_call_ids))::text
                   FROM storyos.active_context_compactions WHERE run_id = $1::text::uuid",
        &[&run_id],
    )
    .await;
    // The summary request sends the read output that waits for it. The later request and each
    // later full input send the read output in its exact form, and no call is a stub.
    assert_eq!(
        (reads, stubs),
        (
            vec![
                json!(["decision", 0, []]),
                json!(["rejected_create", 1, []]),
                json!(["decision", 1, []]),
                json!(["compaction", 2, [read]]),
                json!(["decision", 2, [read]]),
                json!(["rejected_create", 3, [read]]),
                json!(["decision", 3, [read]]),
            ],
            json!([[2, []]]),
        )
    );
    // The current input of the later full input has no read item, so the read output is not a
    // part of the current input digest (ADR 0085).
    let context: Value = json_of(
        &admin,
        "SELECT ((payload #>> '{wire,serialized_payload}')::jsonb -> 'context')::text
                   FROM storyos.model_attempts
                  WHERE run_id = $1::text::uuid AND attempt_role = 'decision'
                    AND decision_position = 3",
        &[&run_id],
    )
    .await;
    let classes: Vec<&str> = context
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["source_class"].as_str().unwrap())
        .collect();
    assert!(!classes.contains(&"skill_resource") && classes.contains(&"skill_catalog"));
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_read_tool_result_call_of_a_skill_call_is_refused_as_a_host_tool_call() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let notes = with_files(
        "notes",
        vec![("notes.md", b"Keep each note short.\n".to_vec())],
    );
    let (_, run_id) = loaded_run(
        &store,
        &admin,
        ["233", "234"],
        "SCRIPT:read_skill_resource load notes notes.md\nSCRIPT:read_tool_result_again",
        set(vec![notes.clone()]),
    )
    .await;

    let host_call = json!([
        "read_tool_result",
        "refused",
        ReadToolResultRefusal::HostToolCall.label(),
        ReadToolResultRefusal::HostToolCall.result().text,
        true
    ]);
    assert_eq!(
        calls(&admin, &run_id).await,
        json!([
            "completed",
            [
                settled("load_skill", &skill_md(&notes)),
                settled("read_skill_resource", "Keep each note short.\n"),
                host_call,
                host_call,
                finish(),
            ]
        ])
    );
}
