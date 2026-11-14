use serde_json::{Value, json};
use storyos_application::{AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore};
use storyos_core::{RESEARCH_RECORDED_RESULT, ReadToolResultRefusal};

use crate::agent_run_submit_research::tests::{ResearchProject, research_project};
use crate::research_source::tests::SEARCH_TEXT;

fn script(first: &str, identity: &str) -> String {
    format!("SCRIPT:read_tool_result {first} {identity}")
}

/// The function outputs that the request at `position` of the Run sent, by call identifier.
async fn outputs(project: &ResearchProject, run_id: &str, position: i32) -> Value {
    let row = project
        .admin
        .query_one(
            "SELECT ((payload #>> '{wire,serialized_payload}')::jsonb -> 'tool_outputs')::text
               FROM storyos.model_attempts
              WHERE run_id = $1::text::uuid AND decision_position = $2::numeric",
            &[&run_id, &position],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get::<_, &str>(/*idx*/ 0)).unwrap()
}

/// The status of the Run and each ToolCall after the first as its function name, state,
/// refusal, and Host result.
async fn later_calls(project: &ResearchProject, run_id: &str) -> Value {
    let facts = project.facts(run_id).await;
    json!({"status": facts["status"], "calls": facts["calls"]})
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_read_gives_the_stored_result_as_the_first_read_and_submit_research_cites_its_source() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let project = research_project("260").await;
    let run_id = project
        .settle("261", &script("web_search", "call_web_search"), SEARCH_TEXT)
        .await;

    // The request after the search sent its result. The request after the read sent the same
    // text again as the output of the read, with the source tag and the sources of the search.
    let first = outputs(&project, &run_id, 1).await;
    let read = outputs(&project, &run_id, 2).await;
    let search_output = first[0]["output"].as_str().unwrap().to_owned();
    let row = project
        .admin
        .query_one(
            "SELECT destination_attempt_id IS NULL AND attempt IS NULL, registration_id
               FROM storyos.tool_calls
              WHERE run_id = $1::text::uuid AND function_name = 'read_tool_result'",
            &[&run_id],
        )
        .await
        .unwrap();
    let read_attempt: String = project
        .admin
        .query_one(
            "SELECT model_attempt_id::text FROM storyos.model_attempts
              WHERE run_id = $1::text::uuid AND decision_position = 1",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let public = project
        .store
        .read_agent_run(
            &project.scope,
            &run_id,
            &AgentRunReadSelection::ModelAttempt(read_attempt),
        )
        .await
        .unwrap()
        .unwrap()
        .decision;

    assert_eq!(
        (
            project.facts(&run_id).await,
            &first,
            &read,
            row.get::<_, bool>(/*idx*/ 0),
            row.get::<_, String>(/*idx*/ 1),
        ),
        (
            json!({
                "status": "completed",
                "steps": [["tool_request", "required"], ["tool_request", "required"],
                          ["tool_request", "required"], ["advisory", "none"]],
                "calls": [["web_search", "settled", null, null],
                          ["read_tool_result", "settled", null, search_output],
                          ["submit_research", "settled", null, RESEARCH_RECORDED_RESULT],
                          ["finish", "settled", null, ""]],
                "artifacts": [["snapshot"]],
            }),
            &json!([{"call_id": "call_web_search", "output": search_output}]),
            &json!([{"call_id": "call_read_tool_result", "output": search_output}]),
            true,
            "host_tools".to_owned(),
        )
    );
    assert!(search_output.starts_with("<tool_result source=\"call_web_search\" state=\"ok\">"));
    assert!(search_output.contains("<tool_sources source=\"call_web_search\"><source id=\""));
    // The public ToolCall list has no Host ToolCall.
    assert!(matches!(
        public,
        AgentRunDecisionInspect::ToolRequest { ref calls, .. } if calls.is_empty()
    ));
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_refused_read_gives_its_reason_to_the_model_and_the_run_continues() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let project = research_project("262").await;
    let refused = |refusal: ReadToolResultRefusal| {
        json!([
            "read_tool_result",
            "refused",
            refusal.label(),
            refusal.result().text
        ])
    };
    let finish = json!(["finish", "settled", null, ""]);
    let search = json!(["web_search", "settled", null, null]);
    // The source of the search result becomes restricted before the read.
    let restrict = format!(
        "UPDATE storyos.model_attempts
            SET payload = payload || '{{\"covered_copy_restricted\": true}}'
          WHERE project_id = '{}' AND model_attempt_id IN (
                SELECT model_attempt_id FROM storyos.tool_calls
                 WHERE project_id = '{}' AND function_name = 'web_search'
                   AND state = 'dispatched')",
        project.scope.project_id.as_ref(),
        project.scope.project_id.as_ref(),
    );
    let cases = [
        (
            "263",
            script("none", "call_unknown"),
            "",
            vec![refused(ReadToolResultRefusal::UnknownCall), finish.clone()],
        ),
        (
            "264",
            script("web_search", "call_read_tool_result"),
            "",
            vec![
                search.clone(),
                refused(ReadToolResultRefusal::HostToolCall),
                finish.clone(),
            ],
        ),
        (
            "265",
            script("none", "call_web_search"),
            "",
            vec![refused(ReadToolResultRefusal::OtherRun), finish.clone()],
        ),
        (
            "266",
            script("refused_search", "call_web_search"),
            "",
            vec![
                json!(["web_search", "refused", "intake_query", null]),
                refused(ReadToolResultRefusal::NoStoredResult),
                finish.clone(),
            ],
        ),
        (
            "267",
            script("web_search", "call_web_search"),
            restrict.as_str(),
            vec![
                search,
                refused(ReadToolResultRefusal::RestrictedSource),
                finish,
            ],
        ),
    ];
    let mut facts = Vec::new();
    let mut expected = Vec::new();
    for (prefix, message, after_exchange, calls) in cases {
        let run_id = project
            .settle_hooked(prefix, &message, SEARCH_TEXT, after_exchange)
            .await;
        facts.push(later_calls(&project, &run_id).await);
        expected.push(json!({"status": "completed", "calls": calls}));
    }
    assert_eq!(facts, expected);
}
