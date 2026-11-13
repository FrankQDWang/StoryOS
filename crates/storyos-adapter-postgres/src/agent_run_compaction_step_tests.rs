use serde_json::{Value, json};
use storyos_application::{AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore};
use storyos_core::{ACTIVE_COMPACTION_REQUEST_PREFIX, ADVISORY_TEXT, AgentRunStatus};

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
