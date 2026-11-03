//! The Tool qualification probe of one AgentRun and its recorded native Tool facts (ADR 0049).

use storyos_application::{
    ClaimedAgentRun, CompleteAgentRunError, ToolEvidence, ToolItemEvidence, ToolProbeRequest,
};
use storyos_core::ToolProbe;

use crate::agent_run_work::{RunPhaseRow, complete_database_error};

/// The probe of one AgentRun and the prior response reference that it continues.
pub(crate) struct ProbeRequest {
    pub request: ToolProbeRequest,
    pub previous_response_reference: Option<String>,
}

/// The probe that the author message of `run` asks for under the current compatibility
/// Decision, or `None` for an ordinary request. A function output probe needs a function call of
/// an earlier probe of the same Conversation; without one, the message is an ordinary request.
pub(crate) async fn probe_request(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
) -> Result<Option<ProbeRequest>, CompleteAgentRunError> {
    let qualification =
        crate::update_project_assistance::read_assistance_record(client, &claim.project_scope)
            .await
            .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?
            .map(|record| record.runtime_qualification);
    let Some(probe) = qualification.and_then(|qualification| {
        storyos_core::tool_probe(
            &run.author_message,
            &run.model_registration_revision,
            qualification,
        )
    }) else {
        return Ok(None);
    };
    let request = match probe {
        ToolProbe::FunctionCall => ToolProbeRequest::FunctionCall,
        ToolProbe::HostedWebSearch => ToolProbeRequest::HostedWebSearch,
        ToolProbe::RemoteMcp => ToolProbeRequest::RemoteMcp,
        ToolProbe::MaxToolCallsBound => ToolProbeRequest::MaxToolCallsBound,
        ToolProbe::FunctionCallOutput => {
            return Ok(prior_function_call(client, claim, &run.conversation_id)
                .await?
                .map(|(reference, call_id)| ProbeRequest {
                    request: ToolProbeRequest::FunctionCallOutput { call_id },
                    previous_response_reference: Some(reference),
                }));
        }
    };
    Ok(Some(ProbeRequest {
        request,
        previous_response_reference: None,
    }))
}

/// The response reference and the call ID of the latest function call probe of the Conversation.
async fn prior_function_call(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    conversation_id: &str,
) -> Result<Option<(String, String)>, CompleteAgentRunError> {
    Ok(client
        .query_opt(
            "SELECT attempt.payload->>'response_reference', item->>'call_id'
               FROM storyos.model_attempts AS attempt
              CROSS JOIN LATERAL jsonb_array_elements(
                      COALESCE(attempt.payload->'tool_evidence'->'items', '[]'::jsonb)) AS item
              WHERE attempt.owner_user_id = $1::text::uuid AND attempt.project_id = $2::text::uuid
                AND attempt.conversation_id = $3::text::uuid
                AND attempt.payload->>'tool_qualification' = $4
                AND attempt.payload->>'response_reference' IS NOT NULL
                AND item->>'kind' = 'function_call' AND item->>'call_id' IS NOT NULL
              ORDER BY attempt.model_attempt_id DESC
              LIMIT 1",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &conversation_id,
                &ToolProbe::FunctionCall.as_str(),
            ],
        )
        .await
        .map_err(complete_database_error)?
        .map(|row| (row.get(0), row.get(1))))
}

pub(crate) fn encode_tool_evidence(evidence: &ToolEvidence) -> serde_json::Value {
    let item = |item: &ToolItemEvidence| {
        serde_json::json!({
            "kind": item.kind,
            "item_id": item.item_id,
            "status": item.status,
            "call_id": item.call_id,
            "name": item.name,
            "server_label": item.server_label,
            "arguments": item.arguments,
            "listed_tools": item.listed_tools,
            "output_present": item.output_present,
            "error": item.error,
        })
    };
    serde_json::json!({
        "items": evidence.items.iter().map(item).collect::<Vec<_>>(),
        "citations": evidence.citations,
        "tool_usage": evidence
            .tool_usage
            .iter()
            .map(|(name, count)| (name.clone(), serde_json::json!(count)))
            .collect::<serde_json::Map<_, _>>(),
    })
}
