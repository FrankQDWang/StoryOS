//! The bounds of the loads of a Run (ADR 0089 sections 6 and 9): at most 8 Skills with the
//! selected Skills included, each Skill one time, two loads as two separate outputs in call
//! order, and the exact load outputs after a compaction (ADR 0093 section 5).

use serde_json::{Value, json};
use storyos_core::{
    ACTIVE_COMPACTION_POSITIONS_PREFIX, ACTIVE_COMPACTION_REQUEST_PREFIX, LoadSkillRefusal,
    SKILL_MD_PATH, SkillFileInput, SkillPackage,
};
use tokio_postgres::Client;

use crate::shared_tests::agent_run_finish_rule::complete;
use crate::shared_tests::agent_run_load_skill::{calls, loaded_run, record, skill_md};
use crate::shared_tests::agent_run_steering::queued;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::create_agent_run_skill::{fixture, package, set};
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::model_gateway::Admission;

/// The Skills `skill-1` to `skill-<count>`, each with a short `SKILL.md`.
fn numbered(count: usize) -> Vec<SkillPackage> {
    (1..=count)
        .map(|index| {
            let name = format!("skill-{index}");
            let text = format!(
                "---\nname: {name}\ndescription: Use it for case {index}.\n---\nRule {index}.\n"
            );
            package(
                &name,
                vec![SkillFileInput {
                    path: SKILL_MD_PATH.to_owned(),
                    bytes: text.into_bytes(),
                }],
            )
        })
        .collect()
}

fn names(range: std::ops::RangeInclusive<usize>) -> String {
    range
        .map(|index| format!("skill-{index}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// The ToolCall row of a call as `calls` gives it.
fn settled(text: &str) -> Value {
    json!(["load_skill", "settled", null, text, true])
}

fn refused(refusal: LoadSkillRefusal) -> Value {
    json!([
        "load_skill",
        "refused",
        refusal.label(),
        refusal.result().text,
        true
    ])
}

fn finish() -> Value {
    json!(["finish", "settled", null, "", true])
}

/// For each Model Attempt of the Run in the order of its creation: its role, its RunStep,
/// whether it continues a prior response, and the call identifier and output of each Tool
/// output that it sent.
async fn sent(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
        "SELECT jsonb_agg(jsonb_build_array(attempt_role, decision_position,
                      wire ? 'previous_response_reference',
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

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn nine_loads_of_one_response_settle_eight_and_refuse_the_ninth_at_the_limit() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let packages = numbered(9);
    let message = format!("SCRIPT:load_skill {}", names(1..=9));
    let (_, run_id) = loaded_run(
        &store,
        &admin,
        ["211", "212"],
        &message,
        set(packages.clone()),
    )
    .await;

    // The Host executes the loads in call order, and each settled load counts for the next.
    let mut expected: Vec<Value> = packages[..8]
        .iter()
        .map(|package| settled(&skill_md(package)))
        .collect();
    expected.extend([refused(LoadSkillRefusal::LoadLimit), finish()]);
    assert_eq!(calls(&admin, &run_id).await, json!(["completed", expected]));
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_second_load_of_a_skill_and_a_load_of_a_selected_skill_are_refused() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let dialogue = fixture("dialogue-style-guide");
    let (_, twice) = loaded_run(
        &store,
        &admin,
        ["213", "214"],
        "SCRIPT:load_skill dialogue-style-guide,dialogue-style-guide",
        set(vec![dialogue.clone()]),
    )
    .await;
    // Two selected Skills use two of the 8 loads, so 6 loads settle.
    let packages = numbered(9);
    let selections = vec![packages[0].reference(), packages[1].reference()];
    let (scope, chapter_id, first) = queued(&store, "215").await;
    complete(&store, &admin, &scope, &first).await;
    let selected = Admission {
        message: &format!("SCRIPT:load_skill skill-1,{}", names(3..=9)),
        skills: (set(packages.clone()), selections),
        ..Default::default()
    }
    .admit(&store, &scope, &chapter_id, "216")
    .await;
    complete(&store, &admin, &scope, &selected).await;

    let already = refused(LoadSkillRefusal::AlreadyLoaded);
    let mut expected = vec![already.clone()];
    expected.extend(
        packages[2..8]
            .iter()
            .map(|package| settled(&skill_md(package))),
    );
    expected.extend([refused(LoadSkillRefusal::LoadLimit), finish()]);
    assert_eq!(
        (calls(&admin, &twice).await, calls(&admin, &selected).await),
        (
            json!([
                "completed",
                [settled(&skill_md(&dialogue)), already, finish()]
            ]),
            json!(["completed", expected]),
        )
    );
}

/// A full input of each RunStep after the first, because the destination loses each reference.
const EXPIRES: &str = "SCRIPT:step-reference-expires";

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn two_loads_are_two_outputs_in_call_order_and_each_full_input_sends_both_again() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let dialogue = fixture("dialogue-style-guide");
    let comms = fixture("internal-comms");
    let skills = || set(vec![dialogue.clone(), comms.clone()]);
    let (_, one_response) = loaded_run(
        &store,
        &admin,
        ["217", "218"],
        &format!("SCRIPT:load_skill internal-comms,dialogue-style-guide\n{EXPIRES}"),
        skills(),
    )
    .await;
    let (_, two_steps) = loaded_run(
        &store,
        &admin,
        ["219", "21a"],
        &format!("SCRIPT:load_skill internal-comms;dialogue-style-guide\n{EXPIRES}"),
        skills(),
    )
    .await;

    // Each output is the exact text of one Skill, in the item order of its call and not in the
    // order of the Skill names. Each request after a lost reference sends its full input with
    // each output again.
    let (comms_md, dialogue_md) = (skill_md(&comms), skill_md(&dialogue));
    let first = json!(["call_load_skill_0", comms_md]);
    let second = json!(["call_load_skill_1", dialogue_md]);
    let finish = json!([
        "call_finish",
        "<tool_result source=\"call_finish\" state=\"empty\">The tool answered with no \
         entry.</tool_result>"
    ]);
    assert_eq!(
        (
            sent(&admin, &one_response).await,
            sent(&admin, &two_steps).await
        ),
        (
            json!([
                ["decision", 0, false, []],
                ["rejected_create", 1, true, [first, second]],
                ["decision", 1, false, [first, second]],
                ["rejected_create", 2, true, [first, second, finish]],
                ["decision", 2, false, [first, second, finish]],
            ]),
            json!([
                ["decision", 0, false, []],
                ["rejected_create", 1, true, [first]],
                ["decision", 1, false, [first]],
                ["rejected_create", 2, true, [first, second]],
                ["decision", 2, false, [first, second]],
                ["rejected_create", 3, true, [first, second, finish]],
                ["decision", 3, false, [first, second, finish]],
            ]),
        )
    );
    // Two Tool sources with their own call and reference, and two items with their own
    // ToolCall identity, in call order.
    let items = |record: &Value| -> Vec<Value> {
        let skills = record[0]
            .as_array()
            .unwrap()
            .iter()
            .map(|source| json!([source["call_id"], source["skill"]["name"]]));
        let texts = record[1].as_array().unwrap().iter().map(|item| {
            let source = record[0]
                .as_array()
                .unwrap()
                .iter()
                .find(|source| source["tool_call_id"] == item["source_version"]);
            json!([
                source.map(|source| source["call_id"].clone()),
                item["content"]
            ])
        });
        skills.chain(texts).collect()
    };
    let expected = vec![
        json!(["call_load_skill_0", "internal-comms"]),
        json!(["call_load_skill_1", "dialogue-style-guide"]),
        first,
        second,
    ];
    assert_eq!(
        (
            items(&record(&admin, &one_response, "decision", 1).await),
            items(&record(&admin, &two_steps, "decision", 2).await),
        ),
        (expected.clone(), expected)
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_compaction_after_the_loads_keeps_each_load_output_exact_and_names_no_load_stub() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let dialogue = fixture("dialogue-style-guide");
    let comms = fixture("internal-comms");
    // RunStep 0 and RunStep 1 load one Skill each, and the Run compacts at RunStep 2. Each later
    // RunStep then sends its full input.
    let message = format!(
        "{ACTIVE_COMPACTION_REQUEST_PREFIX} {ACTIVE_COMPACTION_POSITIONS_PREFIX} 2. \
         SCRIPT:load_skill dialogue-style-guide;internal-comms\n{EXPIRES}"
    );
    let (_, run_id) = loaded_run(
        &store,
        &admin,
        ["21b", "21c"],
        &message,
        set(vec![dialogue.clone(), comms.clone()]),
    )
    .await;

    let first = json!(["call_load_skill_0", skill_md(&dialogue)]);
    let second = json!(["call_load_skill_1", skill_md(&comms)]);
    let sent = sent(&admin, &run_id).await;
    let roles: Vec<Value> = sent
        .as_array()
        .unwrap()
        .iter()
        .map(|attempt| json!([attempt[0], attempt[1], attempt[2]]))
        .collect();
    let load_outputs = |attempt: &Value| -> Vec<Value> {
        attempt[3]
            .as_array()
            .unwrap()
            .iter()
            .filter(|output| [&first[0], &second[0]].contains(&&output[0]))
            .cloned()
            .collect()
    };
    let stubs: Value = json_of(
        &admin,
        "SELECT jsonb_agg(jsonb_build_array(record.decision_position,
                          (SELECT jsonb_agg(call.call_id ORDER BY stub.ordinal)
                             FROM jsonb_array_elements_text(record.stub_tool_call_ids)
                                  WITH ORDINALITY AS stub(id, ordinal)
                             JOIN storyos.tool_calls AS call
                               ON call.tool_call_id = stub.id::uuid)))::text
                   FROM storyos.active_context_compactions AS record
                  WHERE record.run_id = $1::text::uuid",
        &[&run_id],
    )
    .await;
    // The summary request sends the output of the load that waits for it. The later request and
    // each later full input send each load output in its exact form, and no call is a stub.
    assert_eq!(
        (
            roles,
            sent.as_array()
                .unwrap()
                .iter()
                .skip(3)
                .map(load_outputs)
                .collect::<Vec<_>>(),
            stubs,
        ),
        (
            vec![
                json!(["decision", 0, false]),
                json!(["rejected_create", 1, true]),
                json!(["decision", 1, false]),
                json!(["compaction", 2, true]),
                json!(["decision", 2, false]),
                json!(["rejected_create", 3, true]),
                json!(["decision", 3, false]),
            ],
            vec![
                vec![second.clone()],
                vec![first.clone(), second.clone()],
                vec![first.clone(), second.clone()],
                vec![first, second],
            ],
            json!([[2, null]]),
        )
    );
}
