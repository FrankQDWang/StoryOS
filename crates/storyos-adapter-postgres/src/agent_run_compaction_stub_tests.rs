use serde_json::{Value, json};
use storyos_application::{AgentRunReadSelection, AgentRunReadStore};
use storyos_core::{
    ACTIVE_COMPACTION_POSITIONS_PREFIX, ACTIVE_COMPACTION_REQUEST_PREFIX, ContextSourceClass,
    ProjectionMode, RESEARCH_RECORDED_RESULT, ReadToolResultRefusal, ToolResultProjection,
};

use crate::agent_run_submit_research::tests::{ResearchProject, research_project};

/// A search result of 30 entries, so that its exact form is larger than a stub.
fn long_search() -> String {
    (1..=30)
        .map(|entry| {
            format!(
                "Title: Liverpool tide table {entry:02}\nURL: https://example.org/tides/{entry:02}\n\
                 Published: N/A\nAuthor: N/A\nHighlights:\nHigh water at 10.{entry:02} on the \
                 day {entry} of the month, and low water six hours later."
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

/// An author message that compacts at each RunStep of `positions` and then runs `script`.
fn compacting(positions: &str, script: &str) -> String {
    format!(
        "{ACTIVE_COMPACTION_REQUEST_PREFIX} {ACTIVE_COMPACTION_POSITIONS_PREFIX} {positions}. \
         {script}"
    )
}

/// The function outputs that the decision request at `position` sent, by call identifier. At a
/// position that compacts, this is the later request.
async fn sent_outputs(project: &ResearchProject, run_id: &str, position: u32) -> Vec<Value> {
    let row = project
        .admin
        .query_one(
            "SELECT ((attempt.payload #>> '{wire,serialized_payload}')::jsonb
                      -> 'tool_outputs')::text
               FROM storyos.model_attempts AS attempt
              WHERE attempt.run_id = $1::text::uuid
                AND attempt.decision_position = $2::text::numeric
                AND attempt.attempt_role = 'decision'
              ORDER BY attempt.model_attempt_id DESC LIMIT 1",
            &[&run_id, &position.to_string()],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get::<_, &str>(/*idx*/ 0)).unwrap()
}

/// For each compaction record of the Run in RunStep order: its position, its install state, the
/// function name of each call that its later request sent as a stub, and the projection mode of
/// each `tool_result` item of its later request.
async fn stub_records(project: &ResearchProject, run_id: &str) -> Value {
    let row = project
        .admin
        .query_one(
            "SELECT jsonb_agg(jsonb_build_array(record.decision_position, record.install_state,
                      (SELECT jsonb_agg(call.function_name ORDER BY stub.ordinal)
                         FROM jsonb_array_elements_text(record.stub_tool_call_ids)
                              WITH ORDINALITY AS stub(id, ordinal)
                         JOIN storyos.tool_calls AS call ON call.tool_call_id = stub.id::uuid),
                      (SELECT jsonb_agg(item->>'projection_mode')
                         FROM storyos.context_assembly_manifests AS later,
                              jsonb_array_elements(later.payload->'selected') AS item
                        WHERE later.run_id = record.run_id
                          AND later.manifest_role = 'later_request'
                          AND later.decision_position = record.decision_position
                          AND item->>'source_class' = 'tool_result'))
                      ORDER BY record.decision_position)::text
               FROM storyos.active_context_compactions AS record
              WHERE record.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get::<_, &str>(/*idx*/ 0)).unwrap()
}

/// The ToolCall identity of each call of the Run with the provider call identifier of `calls`.
async fn tool_call_ids(project: &ResearchProject, run_id: &str, calls: &[&str]) -> Vec<String> {
    let mut ids = Vec::new();
    for call in calls {
        let row = project
            .admin
            .query_one(
                "SELECT tool_call_id::text FROM storyos.tool_calls
                  WHERE run_id = $1::text::uuid AND call_id = $2",
                &[&run_id, call],
            )
            .await
            .unwrap();
        ids.push(row.get(/*idx*/ 0));
    }
    ids
}

fn output(call_id: &str, output: &Value) -> Value {
    json!({"call_id": call_id, "output": output})
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_model_reads_a_stub_result_again_and_a_later_compaction_makes_the_read_a_stub() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let project = research_project("850").await;
    // RunStep 0 searches, RunStep 1 reads an unknown call, RunStep 2 reads the search after it
    // got its stub, RunStep 3 submits and finishes, and RunStep 4 answers. The Run compacts at
    // RunSteps 2 and 4.
    let run_id = project
        .settle(
            "851",
            &compacting(
                "2 4",
                "SCRIPT:read_tool_result web_search_late call_web_search",
            ),
            &long_search(),
        )
        .await;
    let exact_search = sent_outputs(&project, &run_id, 1).await[0]["output"].clone();
    let at_two = sent_outputs(&project, &run_id, 2).await;
    let at_four = sent_outputs(&project, &run_id, 4).await;
    let search_stub = at_two[0]["output"].clone();
    let refused_read = at_two[1]["output"].clone();
    let read_stub = at_four[2]["output"].clone();
    let text = |value: &Value| value.as_str().unwrap().to_owned();
    let stub_calls = tool_call_ids(
        &project,
        &run_id,
        &["call_web_search", "call_read_tool_result"],
    )
    .await;
    let read = project
        .store
        .read_agent_run(&project.scope, &run_id, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    let public = read
        .compactions
        .into_iter()
        .map(|record| record.stub_tool_call_ids)
        .collect::<Vec<_>>();
    // The read of the Run shows the decision Context Assembly of its active RunStep 4. It keeps
    // the stub of the installed compaction of RunStep 2, so a full input of RunStep 4 sends the
    // stub too.
    let public_modes = read
        .context
        .record
        .selected
        .iter()
        .filter(|item| item.source_class == ContextSourceClass::ToolResult)
        .map(|item| item.projection_mode)
        .collect::<Vec<_>>();
    let facts = project.facts(&run_id).await;
    let unknown = ReadToolResultRefusal::UnknownCall.result().text;

    assert_eq!(
        (
            json!({"status": facts["status"], "calls": facts["calls"],
                   "artifacts": facts["artifacts"]}),
            // The later request of RunStep 2 sends the stub of the search and the refused read,
            // which waits for it. The model then reads the search and gets its stored result.
            at_two.clone(),
            // The later request of RunStep 4 keeps the stub of the search, sends a stub of the
            // read, and the exact results of the Host Tools.
            at_four.clone(),
            stub_records(&project, &run_id).await,
            public,
            public_modes,
        ),
        (
            json!({
                "status": "completed",
                "calls": [["web_search", "settled", null, null],
                          ["read_tool_result", "refused", "unknown_call", unknown],
                          ["read_tool_result", "settled", null, exact_search],
                          ["submit_research", "settled", null, RESEARCH_RECORDED_RESULT],
                          ["finish", "settled", null, ""]],
                "artifacts": [["snapshot"]],
            }),
            vec![
                output("call_web_search", &search_stub),
                output("call_read_tool_result_none", &refused_read),
            ],
            vec![
                output("call_web_search", &search_stub),
                output("call_read_tool_result_none", &refused_read),
                output("call_read_tool_result", &read_stub),
                output(
                    "call_submit_research",
                    &json!(
                        ToolResultProjection::Ok(RESEARCH_RECORDED_RESULT)
                            .continuation_text("call_submit_research")
                    )
                ),
                output(
                    "call_finish",
                    &json!(ToolResultProjection::Empty.continuation_text("call_finish"))
                ),
            ],
            json!([
                [
                    2,
                    "installed",
                    ["web_search"],
                    ["tool_result_stub", "exact_required"]
                ],
                [
                    4,
                    "installed",
                    ["web_search", "read_tool_result"],
                    [
                        "tool_result_stub",
                        "exact_required",
                        "tool_result_stub",
                        "exact_required",
                        "exact_required"
                    ]
                ],
            ]),
            vec![vec![stub_calls[0].clone()], stub_calls.clone()],
            vec![
                ProjectionMode::ToolResultStub,
                ProjectionMode::ExactRequired,
                ProjectionMode::ExactRequired,
                ProjectionMode::ExactRequired,
                ProjectionMode::ExactRequired,
            ],
        )
    );
    // A stub is shorter than its result, keeps its call and its first sources, and says how
    // to read the result again. The stub of the read names the call that it read.
    let first_source = text(&exact_search)
        .split("<source id=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap()
        .to_owned();
    assert_eq!(
        (
            text(&search_stub)
                .starts_with("<tool_result source=\"call_web_search\" state=\"ok\" form=\"stub\">"),
            text(&search_stub).contains(&format!("<source id=\"{first_source}\" ")),
            text(&search_stub).contains("of the 30 sources of the result"),
            text(&search_stub).chars().count() <= 2_000,
            text(&exact_search).chars().count() > 2_000,
            text(&read_stub).contains(
                "This result was the full result of the call call_web_search, and the stub of \
                 that call gives its sources."
            ),
        ),
        (true, true, true, true, true, true)
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn after_a_compaction_submit_research_cites_a_source_of_a_stub() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let project = research_project("852").await;
    let synthesis = json!({"summary": "The tides of 1890.",
                           "claims": [{"text": "High water came at 10.01.",
                                       "citations": ["$SOURCE"]}]});
    // RunStep 0 searches, RunStep 1 reads an unknown call, and RunStep 2 continues after a
    // compaction: it has only the stub of the search to cite from.
    let run_id = project
        .settle(
            "853",
            &compacting(
                "2 3",
                &format!("SCRIPT:submit_research late web_search {synthesis}"),
            ),
            &long_search(),
        )
        .await;
    let at_two = sent_outputs(&project, &run_id, 2).await;
    let at_three = sent_outputs(&project, &run_id, 3).await;
    let refusal = ReadToolResultRefusal::UnknownCall.result().text;
    let sent_refusal = json!(
        ToolResultProjection::Rejected(Some(&refusal)).continuation_text("call_read_tool_result")
    );
    let facts = project.facts(&run_id).await;

    assert_eq!(
        (
            json!({"status": facts["status"], "calls": facts["calls"],
                   "artifacts": facts["artifacts"]}),
            at_two[1].clone(),
            // The refused read is the result of a Host Tool. It keeps its exact form at the
            // compaction of RunStep 3, although no request waits for it.
            at_three[1].clone(),
            stub_records(&project, &run_id).await,
        ),
        (
            json!({
                "status": "completed",
                "calls": [["web_search", "settled", null, null],
                          ["read_tool_result", "refused", "unknown_call", refusal],
                          ["submit_research", "settled", null, RESEARCH_RECORDED_RESULT],
                          ["finish", "settled", null, ""]],
                "artifacts": [["snapshot"]],
            }),
            output("call_read_tool_result", &sent_refusal),
            output("call_read_tool_result", &sent_refusal),
            json!([
                [
                    2,
                    "installed",
                    ["web_search"],
                    ["tool_result_stub", "exact_required"]
                ],
                [
                    3,
                    "installed",
                    ["web_search"],
                    [
                        "tool_result_stub",
                        "exact_required",
                        "exact_required",
                        "exact_required"
                    ]
                ],
            ]),
        )
    );
    assert!(
        at_two[0]["output"]
            .as_str()
            .unwrap()
            .contains("form=\"stub\"")
    );
}
