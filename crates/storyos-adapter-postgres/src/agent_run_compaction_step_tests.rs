use serde_json::{Value, json};
use storyos_application::{AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore};
use storyos_core::{
    ACTIVE_COMPACTION_REQUEST_PREFIX, ADVISORY_TEXT, AgentRunStatus, RESEARCH_TOOL_OFFER,
};

use crate::agent_run_dispatch::tests::stores;
use crate::agent_run_finish_rule::tests::{steps, tool_loop_run};

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_first_run_step_with_a_prior_response_installs_the_summary_and_the_run_continues() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let message = format!("{ACTIVE_COMPACTION_REQUEST_PREFIX} Help with this passage.");
    // In a new Conversation, only RunStep 2 has a prior response: the response of RunStep 1. In
    // the Conversation of the first Run, RunStep 1 compacts, and RunStep 2 continues the later
    // request.
    let cases = [(["660", "661"], false, 1), (["662", "663"], true, 0)];
    for (prefixes, same_conversation, position) in cases {
        let (scope, _, _, run_id) =
            tool_loop_run(&store, &admin, prefixes, &message, None, same_conversation).await;
        let record = store
            .read_agent_run(&scope, &run_id, &AgentRunReadSelection::Current)
            .await
            .unwrap()
            .unwrap();
        let compaction: Value = admin
            .query_one(
                "SELECT jsonb_build_array(record.install_state, attempt.decision_position,
                          (SELECT jsonb_agg(output -> 'call_id') FROM jsonb_array_elements(
                             (attempt.payload #>> '{wire,serialized_payload}')::jsonb
                               -> 'tool_outputs') AS output))::text
                   FROM storyos.active_context_compactions AS record
                   JOIN storyos.model_attempts AS attempt
                     ON attempt.model_attempt_id = record.installed_model_attempt_id
                  WHERE record.run_id = $1::text::uuid",
                &[&run_id],
            )
            .await
            .map(|row| serde_json::from_str(row.get(0)).unwrap())
            .unwrap();
        let steps = steps(&admin, &run_id).await;
        // The later request sends no prior response, and the RunStep after it continues it.
        let prior: Vec<_> = steps
            .iter()
            .enumerate()
            .map(|(index, step)| match index {
                _ if index == position => step[3].is_null(),
                0 => true,
                _ => step[3] == steps[index - 1][2],
            })
            .collect();
        let outputs = match position {
            0 => Value::Null,
            _ => json!(["call_finish"]),
        };

        assert_eq!(
            (
                compaction,
                prior,
                record.status,
                matches!(record.decision, AgentRunDecisionInspect::Advisory { ref text, .. }
                    if text == ADVISORY_TEXT),
            ),
            (
                json!(["installed", position, outputs]),
                vec![true, true],
                AgentRunStatus::Completed,
                true,
            ),
            "{same_conversation}"
        );
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_later_run_step_compacts_while_the_tool_results_of_the_prior_response_wait() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    // RunStep 1 calls `web_search` and `finish`. This process has no Tool adapter, so the search
    // is refused, and the `finish` call does not count. Both results wait for the request of
    // RunStep 2, and RunStep 2 compacts first.
    let message = format!("{ACTIVE_COMPACTION_REQUEST_PREFIX} Search the web: rain");
    let (scope, _, _, run_id) = tool_loop_run(
        &store,
        &admin,
        ["664", "665"],
        &message,
        Some(&RESEARCH_TOOL_OFFER),
        /*same_conversation*/ false,
    )
    .await;
    let record = store
        .read_agent_run(&scope, &run_id, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    let sent = |role: &str| {
        format!(
            "(SELECT (payload #>> '{{wire,serialized_payload}}')::jsonb -> 'tool_outputs'
                FROM storyos.model_attempts
               WHERE run_id = $1::text::uuid AND attempt_role = '{role}'
                 AND decision_position = 1)"
        )
    };
    let row: Value = admin
        .query_one(
            &format!(
                "SELECT jsonb_build_array({}, {},
                   (SELECT jsonb_agg(jsonb_build_array(call_id, state, result_text)
                                     ORDER BY call_position)
                      FROM storyos.tool_calls
                     WHERE run_id = $1::text::uuid AND decision_position = 0),
                   (SELECT install_state FROM storyos.active_context_compactions
                     WHERE run_id = $1::text::uuid))::text",
                sent("compaction"),
                sent("decision")
            ),
            &[&run_id],
        )
        .await
        .map(|row| serde_json::from_str(row.get(0)).unwrap())
        .unwrap();
    let (summary, later, stored) = (&row[0], &row[1], &row[2]);
    let delivered: Vec<_> = stored
        .as_array()
        .unwrap()
        .iter()
        .zip(later.as_array().unwrap())
        .map(|(call, output)| {
            let text = output["output"].as_str().unwrap();
            (
                text.starts_with(&format!(
                    r#"<tool_result source="{}""#,
                    call[0].as_str().unwrap()
                )),
                call[2].as_str().is_none_or(|result| text.contains(result)),
            )
        })
        .collect();

    // The summary request answers each call of the prior response, and the later request sends
    // each result again with its call, so the model gets each result after the install.
    assert_eq!(
        (
            summary == later,
            delivered,
            stored.as_array().unwrap().len(),
            row[3].clone(),
            record.status,
            matches!(record.decision, AgentRunDecisionInspect::Advisory { ref text, .. }
                if text == ADVISORY_TEXT),
        ),
        (
            true,
            vec![(true, true), (true, true)],
            2,
            json!("installed"),
            AgentRunStatus::Completed,
            true,
        )
    );
}
