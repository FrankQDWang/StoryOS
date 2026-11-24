//! The Host Tool `load_skill` of a Run with a pinned catalog (ADR 0089 sections 3 and 7): the
//! settled load, the request after it, the refused loads, and the summary request of a
//! compaction that waits for the output of a load.

use crate::statement_cache::Client;
use serde_json::{Value, json};
use storyos_application::{AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore};
use storyos_core::{
    ACTIVE_COMPACTION_POSITIONS_PREFIX, ACTIVE_COMPACTION_REQUEST_PREFIX, CallValidationRefusal,
    FINISH_DOES_NOT_COUNT_RESULT, IntakeRefusal, LoadSkillRefusal, SKILL_MD_PATH, SkillFileInput,
    SkillPackage, count_stored_text,
};

use crate::shared_tests::agent_run_load_skill::{TOOLS, calls, loaded_run, record, skill_md};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::create_agent_run_skill::{fixture, package, set};
use crate::shared_tests::json_row::json_of;

/// A package whose `SKILL.md` has exactly 32,000 counted tokens, the limit of ADR 0089 section
/// 9, with `<`, `&`, and `"` in its text.
fn long_package() -> SkillPackage {
    let head = "---\nname: long-dialogue\ndescription: Use it for long dialogue scenes.\n---\n";
    let line = "Write <each> line & keep the \"exact\" quote marks.\n";
    let mut text = head.to_owned();
    while count_stored_text(&text).character_count + (line.len() as u64) <= 32_000 {
        text.push_str(line);
    }
    let missing = 32_000 - count_stored_text(&text).character_count;
    text.push_str(&"&".repeat(usize::try_from(missing).unwrap()));
    assert_eq!(count_stored_text(&text).character_count, 32_000);
    package(
        "long-dialogue",
        vec![SkillFileInput {
            path: SKILL_MD_PATH.to_owned(),
            bytes: text.into_bytes(),
        }],
    )
}

/// For each Model Attempt of the Run in the order of its creation: its role, its RunStep, the
/// Tools that it declared, the Tool outputs that it sent, and whether it sent the current input.
async fn requests(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
        "SELECT jsonb_agg(jsonb_build_array(attempt_role, decision_position, wire -> 'tools',
                      wire -> 'tool_outputs', wire ? 'context')
                      ORDER BY model_attempt_id)::text
               FROM storyos.model_attempts,
                    LATERAL (SELECT (payload #>> '{wire,serialized_payload}')::jsonb) AS w(wire)
              WHERE run_id = $1::text::uuid",
        &[&run_id],
    )
    .await
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_load_settles_with_the_exact_skill_md_and_the_next_request_sends_it_as_the_call_output() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let long = long_package();
    let text = skill_md(&long);
    let skills = set(vec![fixture("dialogue-style-guide"), long.clone()]);
    let (scope, run_id) = loaded_run(
        &store,
        &admin,
        ["c81", "c82"],
        "SCRIPT:load_skill long-dialogue",
        skills,
    )
    .await;
    // The ToolCall of the load, its Model Attempt, and its Decision.
    let ids = admin
        .query_one(
            "SELECT tool_call_id::text, model_attempt_id::text, decision_id::text
               FROM storyos.tool_calls
              WHERE run_id = $1::text::uuid AND function_name = 'load_skill'",
            &[&run_id],
        )
        .await
        .unwrap();
    let (tool_call_id, attempt_id, decision_id): (String, String, String) =
        (ids.get(/*idx*/ 0), ids.get(/*idx*/ 1), ids.get(/*idx*/ 2));
    let public = store
        .read_agent_run(
            &scope,
            &run_id,
            &AgentRunReadSelection::ModelAttempt(attempt_id.clone()),
        )
        .await
        .unwrap()
        .unwrap()
        .decision;

    // The load settles with the exact text of the Snapshot. The request after it continues the
    // response of the load with an unchanged input: it sends only the output, the exact text
    // with no tag. The closing request sends the output of `finish`.
    let output = json!({"call_id": "call_load_skill_0", "output": text});
    let finish = json!({
        "call_id": "call_finish",
        "output": "<tool_result source=\"call_finish\" state=\"empty\">The tool answered with no \
                   entry.</tool_result>",
    });
    assert_eq!(
        (
            calls(&admin, &run_id).await,
            requests(&admin, &run_id).await
        ),
        (
            json!([
                "completed",
                [
                    ["load_skill", "settled", null, text, true],
                    ["finish", "settled", null, "", true],
                ]
            ]),
            json!([
                ["decision", 0, TOOLS, [], true],
                ["decision", 1, TOOLS, [output], false],
                ["decision", 2, TOOLS, [output, finish], false],
            ]),
        )
    );
    // The Operation Requirement records the ToolCall with its SkillPackage Reference, and the
    // output is a `skill_instruction` item with the exact text.
    let source = json!({
        "tool_call_id": tool_call_id,
        "call_id": "call_load_skill_0",
        "model_attempt_id": attempt_id,
        "decision_id": decision_id,
        "state": "ok",
        "trust": "skill_instruction",
        "kept_result_tokens": null,
        "skill": {"name": "long-dialogue", "package_digest": long.package_digest},
    });
    let item = json!({
        "source_class": "skill_instruction",
        "source_version": tool_call_id,
        "projection_mode": "exact_required",
        "token_count": 32_000,
        "content": text,
    });
    assert_eq!(
        record(&admin, &run_id, "decision", 1).await,
        json!([[source], [item]])
    );
    assert!(!text.contains("<tool_result") && text.contains("<each> line & keep the \"exact\""));
    // The public ToolCall list has no Host ToolCall.
    assert!(matches!(
        public,
        AgentRunDecisionInspect::ToolRequest { ref calls, .. } if calls.is_empty()
    ));
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_refused_load_gives_its_reason_and_a_run_with_no_skill_declares_no_load_skill() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let skills = || set(vec![fixture("dialogue-style-guide")]);
    let (_, unknown) = loaded_run(
        &store,
        &admin,
        ["c83", "c84"],
        "SCRIPT:load_skill no-such-skill",
        skills(),
    )
    .await;
    let (_, empty_reason) = loaded_run(
        &store,
        &admin,
        ["c85", "c86"],
        r#"SCRIPT:refuse load_skill {"skill":"dialogue-style-guide","reason":""}"#,
        skills(),
    )
    .await;
    let (_, no_skill) = loaded_run(
        &store,
        &admin,
        ["c87", "c88"],
        "SCRIPT:load_skill dialogue-style-guide",
        set(Vec::new()),
    )
    .await;

    // Each refused call gives its reason in one sentence as a `tool_result` output, and the Run
    // continues to `finish`.
    let not_in_catalog = LoadSkillRefusal::NotInCatalog.result().text;
    let reason = CallValidationRefusal::Intake(IntakeRefusal::Reason).result_text();
    let finish = json!(["finish", "settled", null, "", true]);
    assert_eq!(
        (
            calls(&admin, &unknown).await,
            calls(&admin, &empty_reason).await
        ),
        (
            json!([
                "completed",
                [
                    [
                        "load_skill",
                        "refused",
                        "skill_not_in_catalog",
                        not_in_catalog,
                        true
                    ],
                    finish,
                ]
            ]),
            json!([
                "completed",
                [
                    ["load_skill", "refused", "intake_reason", reason, true],
                    [
                        "finish",
                        "settled",
                        null,
                        FINISH_DOES_NOT_COUNT_RESULT,
                        true
                    ],
                    finish,
                ]
            ]),
        )
    );
    let first_output = |requests: &Value| requests[1][3][0]["output"].as_str().unwrap().to_owned();
    for (run_id, call_id, text) in [
        (&unknown, "call_load_skill_0", &not_in_catalog),
        (&empty_reason, "call_load_skill_refused", &reason),
    ] {
        let sent = requests(&admin, run_id).await;
        assert_eq!(
            first_output(&sent),
            format!("<tool_result source=\"{call_id}\" state=\"rejected\">{text}</tool_result>")
        );
        assert_eq!(record(&admin, run_id, "decision", 1).await[1], json!([]));
    }
    // A Run whose pinned catalog has no Skill declares the Tools of before, so its load call is
    // an unexposed name.
    let sent = requests(&admin, &no_skill).await;
    assert_eq!(
        sent[0][2],
        json!([
            "propose_edit",
            "submit_research",
            "read_tool_result",
            "read_chapter",
            "search_manuscript",
            "write_memory",
            "read_memory_entry",
            "search_memory",
            "update_plan",
            "start_subrun",
            "finish"
        ])
    );
    assert_eq!(
        calls(&admin, &no_skill).await[1][0],
        json!([
            "load_skill",
            "refused",
            "unexposed_name",
            CallValidationRefusal::Unexposed.result_text(),
            false
        ])
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_summary_request_of_a_compaction_at_the_step_of_the_load_sends_the_load_output() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let dialogue = fixture("dialogue-style-guide");
    let text = skill_md(&dialogue);
    let message = format!(
        "{ACTIVE_COMPACTION_REQUEST_PREFIX} {ACTIVE_COMPACTION_POSITIONS_PREFIX} 1. \
         SCRIPT:load_skill dialogue-style-guide"
    );
    let (_, run_id) = loaded_run(
        &store,
        &admin,
        ["c89", "c8a"],
        &message,
        set(vec![dialogue]),
    )
    .await;

    // The summary request continues the response of the load, so it sends the output of the
    // load. The later request is a full input: it sends the call and its output again.
    let output = json!({"call_id": "call_load_skill_0", "output": text});
    let sent = requests(&admin, &run_id).await;
    assert_eq!(
        (
            calls(&admin, &run_id).await[0].clone(),
            sent[1][0].clone(),
            sent[1][3].clone(),
            sent[2].as_array().unwrap()[..4].to_vec(),
        ),
        (
            json!("completed"),
            json!("compaction"),
            json!([output]),
            vec![json!("decision"), json!(1), json!(TOOLS), json!([output])],
        )
    );
    // The current input of the later full input has no load item, so the load output is not a
    // part of the current input digest (ADR 0085).
    let context = json_of(
        &admin,
        "SELECT ((payload #>> '{wire,serialized_payload}')::jsonb -> 'context')::text
               FROM storyos.model_attempts
              WHERE run_id = $1::text::uuid AND attempt_role = 'decision'
                AND decision_position = 1",
        &[&run_id],
    )
    .await;
    let classes: Vec<_> = context
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["source_class"].clone())
        .collect();
    assert_eq!(
        classes,
        [
            "conversation_summary",
            "author_instruction",
            "working_target",
            "skill_catalog",
            "chapter_list"
        ]
    );
    let summary = record(&admin, &run_id, "compaction", 1).await;
    assert_eq!(
        (
            summary[0][0]["call_id"].clone(),
            summary[1][0]["content"].clone()
        ),
        (json!("call_load_skill_0"), json!(text))
    );
}
