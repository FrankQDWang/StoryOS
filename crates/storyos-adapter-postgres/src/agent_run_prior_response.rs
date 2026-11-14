//! The function calls of the prior response that a request names. The request gives exactly one
//! output for each of them, because each native function call needs its output (ADR 0085
//! section 3). The response comes from the prior binding of the request, not from a RunStep
//! position: a RunStep that read a steering input before its request has no Model Attempt.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError, ToolOutput, ToolOutputOrigin};

use crate::agent_run_tool_authority::damaged;
use crate::agent_run_work::complete_database_error;

/// The bound response that a request continues.
pub(crate) struct PriorResponse {
    decision_id: Option<String>,
    /// Each call of a function request Decision is a ToolCall of the Run.
    function_request: bool,
    /// The call identifiers of the function call items of the response, in item order.
    call_ids: Vec<String>,
}

impl PriorResponse {
    /// The bound response of this Conversation with the destination reference `reference`.
    pub(crate) async fn load(
        client: &tokio_postgres::Client,
        claim: &ClaimedAgentRun,
        reference: &str,
    ) -> Result<Self, CompleteAgentRunError> {
        let row = client
            .query_opt(
                "SELECT attempt.decision_id::text, attempt.payload #>> '{decision,kind}',
                        jsonb_path_query_array(attempt.payload,
                                               '$.items[*] ? (@.role == \"tool\").call_id')::text
                   FROM storyos.model_attempts AS attempt
                   JOIN storyos.agent_runs AS run
                     ON (run.owner_user_id, run.project_id, run.conversation_id) =
                        (attempt.owner_user_id, attempt.project_id, attempt.conversation_id)
                  WHERE run.owner_user_id = $1::text::uuid AND run.project_id = $2::text::uuid
                    AND run.run_id = $3::text::uuid
                    AND attempt.continuation_binding_id IS NOT NULL
                    AND attempt.payload->>'response_reference' = $4",
                &[
                    &claim.project_scope.owner_user_id.as_ref(),
                    &claim.project_scope.project_id.as_ref(),
                    &claim.run_id,
                    &reference,
                ],
            )
            .await
            .map_err(complete_database_error)?
            .ok_or_else(|| damaged("The prior response of the request is missing"))?;
        let call_ids: Vec<Option<String>> = serde_json::from_str(row.get(/*idx*/ 2))
            .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
        Ok(Self {
            decision_id: row.get(/*idx*/ 0),
            function_request: row.get::<_, Option<&str>>(/*idx*/ 1) == Some("tool_request"),
            call_ids: call_ids.into_iter().flatten().collect(),
        })
    }

    /// The Decision whose ToolCalls answer the calls of the response.
    pub(crate) fn decision_id(&self) -> Option<&str> {
        self.decision_id.as_deref()
    }

    /// The calls of the response that no output of `outputs` with the origin `PriorResponse`
    /// answers. The Host executed none of them: they are the calls of the last RunStep of the
    /// limit (ADR 0091 section 5). A function request has a ToolCall for each call, so each of
    /// its calls must have its output.
    pub(crate) fn unexecuted_calls(
        &self,
        outputs: &[ToolOutput],
    ) -> Result<Vec<String>, CompleteAgentRunError> {
        let unexecuted: Vec<String> = self
            .call_ids
            .iter()
            .filter(|call_id| {
                !outputs.iter().any(|output| {
                    output.origin == ToolOutputOrigin::PriorResponse && &output.call_id == *call_id
                })
            })
            .cloned()
            .collect();
        if self.function_request && !unexecuted.is_empty() {
            return Err(damaged("A ToolCall of the prior response has no output"));
        }
        Ok(unexecuted)
    }
}

#[cfg(test)]
#[path = "agent_run_prior_response_tests.rs"]
mod tests;
