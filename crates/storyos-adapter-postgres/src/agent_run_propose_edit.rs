//! The Host Tool `propose_edit` in the store (ADR 0091). The Decision transaction refuses each
//! call that a limit or the prose change validation refuses. The Tool Gateway opens the Proposal
//! of an accepted call with the code of the prose change reply, and keeps its record on the
//! Model Attempt of the call. The read gives a Run that made a Proposal the public
//! `prose_change` of that record.

use storyos_application::{
    AgentRunDecisionInspect, ClaimedAgentRun, CompleteAgentRunError, CreateAgentRunError,
    DeclaredTarget, ProjectScope,
};
use storyos_core::{
    DeclaredLocation, PROPOSAL_WAITS_RESULT, PROPOSE_EDIT_TOOL_SPEC, ProposeEditHistory,
    ProposeEditRefusal, ProposedEdit, RequestedToolCall, ToolCallBatch, ToolCallState,
};

use crate::agent_run_observation::open_selected_proposal;
use crate::agent_run_tool_authority::{Resolution, ToolCallRow, resolve};
use crate::agent_run_work::complete_database_error;
use crate::create_agent_run::{agent_run_database_error, agent_run_parse_error};

/// Refuses each `propose_edit` call of `batch` from `response` that the limits of the Run or the
/// prose change validation against `declared` refuse.
pub(crate) async fn screen(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    declared: &[DeclaredLocation],
    response: &storyos_application::ModelResponse,
    batch: ToolCallBatch,
) -> Result<ToolCallBatch, CompleteAgentRunError> {
    let name = storyos_core::ToolOperation::ProposeEdit.function_name();
    let ToolCallBatch::Valid(calls) = &batch else {
        return Ok(batch);
    };
    if !calls.iter().any(|call| match call {
        RequestedToolCall::Validated(call) => call.name == name,
        RequestedToolCall::Refused(call) => call.name == name,
    }) {
        return Ok(batch);
    }
    // A steering input permits the next call, as a steering decision does (ADR 0091).
    let active: String = client
        .query_one(
            "SELECT active_decision_position::text FROM storyos.agent_runs
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map_err(complete_database_error)?
        .get(/*idx*/ 0);
    let steered = crate::agent_run_steering::last_read_position(client, claim, &active).await?;
    let row = client
        .query_one(
            "SELECT count(*) FILTER (WHERE call.state = 'refused'),
                    COALESCE(bool_or(call.state = 'settled'
                                     AND call.decision_position >= $5::text::numeric), false)
               FROM storyos.tool_calls AS call
              WHERE call.owner_user_id = $1::text::uuid AND call.project_id = $2::text::uuid
                AND call.run_id = $3::text::uuid AND call.tool_spec_revision = $4",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &PROPOSE_EDIT_TOOL_SPEC.revision,
                &steered,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    let history = ProposeEditHistory {
        refused: usize::try_from(row.get::<_, i64>(/*idx*/ 0)).unwrap_or(usize::MAX),
        proposal_open: row.get(/*idx*/ 1),
    };
    Ok(storyos_core::screen_proposed_edits(
        batch,
        &response.items,
        declared,
        history,
    ))
}

/// Opens the Proposal of the pending `propose_edit` call `row` of the active RunStep and settles
/// the call. The Proposal source is the Decision that holds the call. A call that opens no
/// Proposal is refused.
pub(crate) async fn execute(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    edit: &ProposedEdit,
) -> Result<(), CompleteAgentRunError> {
    let run = crate::agent_run_work::load_run_phase(client, claim).await?;
    // The Attempt of the call can be a successor of the Attempt of the RunStep.
    let owner = client
        .query_one(
            "SELECT model_attempt_id::text, decision_id::text FROM storyos.tool_calls
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND tool_call_id = $3::text::uuid",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &row.tool_call_id,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    let (attempt_id, decision_id): (String, String) = (owner.get(0), owner.get(1));
    let declared: Vec<DeclaredLocation> =
        crate::agent_run_create_dispatch::declared_targets(client, claim, &run.chapter_id)
            .await?
            .iter()
            .map(DeclaredTarget::location)
            .collect();
    let refusal = match edit.validate(&declared) {
        Err(refusal) => Some(refusal),
        Ok(candidates) => {
            open(
                client,
                claim,
                &run,
                (&attempt_id, &decision_id),
                edit,
                &candidates,
            )
            .await?
        }
    };
    let text = refusal.map_or_else(
        || PROPOSAL_WAITS_RESULT.to_owned(),
        ProposeEditRefusal::result_text,
    );
    let result = storyos_core::ToolResultText {
        digest: format!("sha256:{}", storyos_core::hex_sha256(text.as_bytes())),
        bytes: text.len(),
        text,
    };
    let resolution = Resolution {
        state: match refusal {
            Some(_) => ToolCallState::Refused,
            None => ToolCallState::Settled,
        },
        refusal: refusal.map(ProposeEditRefusal::label),
        attempt: None,
        outcome: None,
        result: Some(&result),
    };
    resolve(client, claim, row, resolution).await
}

/// Opens the Proposal of the validated `candidates` and keeps its record on the Model Attempt
/// `attempt.0` of the Decision `attempt.1`. `StaleCandidate` when the referenced candidate
/// changed after the request read it, and `NotOpened` when no other block opened.
async fn open(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &crate::agent_run_work::RunPhaseRow,
    (attempt_id, decision_id): (&str, &str),
    edit: &ProposedEdit,
    candidates: &[storyos_core::ProseChangeCandidate],
) -> Result<Option<ProposeEditRefusal>, CompleteAgentRunError> {
    let opening = open_selected_proposal(
        client,
        claim,
        run,
        decision_id,
        &edit.summary,
        Some(candidates),
    )
    .await?;
    let Some(proposal_id) = opening.proposal_id else {
        let stale = opening.locations.iter().flatten().any(|location| {
            matches!(&location.outcome,
                storyos_contracts::ProseChangeLocationOutcome::Refused { reason }
                    if reason == "stale_candidate")
        });
        return Ok(Some(if stale {
            ProposeEditRefusal::StaleCandidate
        } else {
            ProposeEditRefusal::NotOpened
        }));
    };
    let record = serde_json::json!({
        "decision_id": decision_id,
        "text": edit.summary,
        "producer_input": storyos_core::canonical_json(&serde_json::json!(candidates)),
        "proposal_id": proposal_id,
        "locations": opening.locations,
    });
    client
        .execute(
            "UPDATE storyos.model_attempts
                SET payload = jsonb_set(payload, '{proposed_edit}', $4::text::jsonb)
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND model_attempt_id = $3::text::uuid",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &attempt_id,
                &record.to_string(),
            ],
        )
        .await
        .map_err(complete_database_error)?;
    Ok(None)
}

/// Gives a Run that made a Proposal with `propose_edit` the public `prose_change` of its last
/// such call in place of its final answer or of a later refusal. A Run with ToolCalls in
/// progress keeps its `decision`.
pub(crate) async fn public_decision(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    run_id: &str,
    decision: &mut AgentRunDecisionInspect,
) -> Result<(), CreateAgentRunError> {
    let continuation_binding_id = match decision {
        AgentRunDecisionInspect::Absent | AgentRunDecisionInspect::ExecutionRefused { .. } => None,
        AgentRunDecisionInspect::Advisory {
            continuation_binding_id,
            ..
        }
        | AgentRunDecisionInspect::Clarification {
            continuation_binding_id,
            ..
        } => continuation_binding_id.clone(),
        AgentRunDecisionInspect::ProseChange { .. }
        | AgentRunDecisionInspect::ToolRequest { .. }
        | AgentRunDecisionInspect::ResearchSynthesis { .. } => return Ok(()),
    };
    let Some(row) = client
        .query_opt(
            "SELECT (payload -> 'proposed_edit')::text FROM storyos.model_attempts
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid AND payload ? 'proposed_edit'
              ORDER BY decision_position DESC LIMIT 1",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
            ],
        )
        .await
        .map_err(agent_run_database_error)?
    else {
        return Ok(());
    };
    let record: serde_json::Value =
        serde_json::from_str(row.get(/*idx*/ 0)).map_err(agent_run_parse_error)?;
    let text = |field: &str| record[field].as_str().unwrap_or_default().to_owned();
    *decision = AgentRunDecisionInspect::ProseChange {
        decision_id: text("decision_id"),
        selected: true,
        text: text("text"),
        producer_input: text("producer_input"),
        locations: serde_json::from_value(record["locations"].clone())
            .map_err(agent_run_parse_error)?,
        continuation_binding_id,
        opened_proposal_id: Some(text("proposal_id")),
    };
    Ok(())
}

#[cfg(test)]
#[path = "agent_run_propose_edit_tests.rs"]
pub(crate) mod tests;

#[cfg(test)]
#[path = "agent_run_propose_edit_revise_tests.rs"]
pub(crate) mod revise_tests;
