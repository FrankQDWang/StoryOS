//! The function calls of the prior response that a request names. The request gives exactly one
//! output for each of them, because each native function call needs its output (ADR 0085
//! section 3). The response comes from the prior binding of the request, not from a RunStep
//! position: a RunStep that read a steering input before its request has no Model Attempt.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError, ToolOutput, ToolOutputOrigin};

use crate::agent_run_tool_authority::damaged;
use crate::agent_run_work::complete_database_error;

/// The record that names the prior response of a request.
#[derive(Clone, Copy)]
pub(crate) enum PriorKey<'a> {
    /// The prior Model Continuation Binding of an ordinary request.
    Binding(&'a str),
    /// The prior Model Attempt of the chain of a summary request.
    Attempt(&'a str),
}

/// The bound response that a request continues.
pub(crate) struct PriorResponse {
    decision_id: Option<String>,
    /// Each call of a function request Decision is a ToolCall of the Run.
    function_request: bool,
    /// The call identifiers of the function call items of the response, in item order.
    call_ids: Vec<String>,
}

impl PriorResponse {
    /// The bound response that `key` names. A reference does not name one response: a test
    /// adapter can report the reference of an earlier response again.
    pub(crate) async fn load(
        client: &tokio_postgres::Client,
        claim: &ClaimedAgentRun,
        key: PriorKey<'_>,
    ) -> Result<Self, CompleteAgentRunError> {
        let (binding, attempt) = match key {
            PriorKey::Binding(binding) => (Some(binding), None),
            PriorKey::Attempt(attempt) => (None, Some(attempt)),
        };
        let row = client
            .query_opt(
                "SELECT decision_id::text, payload #>> '{decision,kind}',
                        jsonb_path_query_array(payload,
                                               '$.items[*] ? (@.role == \"tool\").call_id')::text
                   FROM storyos.model_attempts
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND continuation_binding_id IS NOT NULL
                    AND (continuation_binding_id = $3::text::uuid
                         OR model_attempt_id = $4::text::uuid)",
                &[
                    &claim.project_scope.owner_user_id.as_ref(),
                    &claim.project_scope.project_id.as_ref(),
                    &binding,
                    &attempt,
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
