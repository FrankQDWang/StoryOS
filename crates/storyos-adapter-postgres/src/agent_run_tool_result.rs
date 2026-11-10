//! The continuation RunStep of a durable function request Decision (S5-05, ADR 0033, ADR 0084).
//! When each ToolCall of the Decision is resolved, the store opens the next RunStep: a new
//! Operation Requirement `tool_result_continuation` with one `tool_result` Context Candidate for
//! each resolved ToolCall of the Run, and then the ordinary Create path of the Model Gateway. The
//! output text of a request always comes from the stored ToolCall; a delivery never starts a Tool
//! exchange.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError, ToolOutput, ToolOutputOrigin};
use storyos_core::{
    ContextSourceClass, CurrentPassageAssemblyRecord, ToolCallState, ToolResultCandidate,
};

use crate::agent_run_tool_authority::damaged;
use crate::agent_run_work::{RunPhaseRow, WorkPhase, complete_database_error};
use crate::create_agent_run::context::{PassageContextInput, persist_current_passage_assembly};

/// One stored ToolCall of the Run.
struct StoredToolCall {
    tool_call_id: String,
    call_id: String,
    owner_user_id: String,
    project_id: String,
    run_id: String,
    model_attempt_id: String,
    decision_id: String,
    state: ToolCallState,
    result_text: Option<String>,
}

/// Opens the continuation RunStep after the last ToolCall of the active Decision resolved: binds
/// the response of the function request as the prior response of the continuation, commits gates
/// one to six of its Context Assembly, and makes it the active RunStep. The next work phase
/// admits its Create.
pub(crate) async fn open_continuation(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
) -> Result<WorkPhase, CompleteAgentRunError> {
    let (Some(attempt_id), Some(decision_id)) = (&run.attempt_id, &run.decision_id) else {
        return Err(damaged("The function request Decision is missing"));
    };
    let payload: serde_json::Value =
        serde_json::from_str(run.attempt_payload.as_deref().unwrap_or("null"))
            .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    crate::agent_run_continuation::bind_produced_continuation(
        client,
        claim,
        decision_id,
        attempt_id,
        &payload,
    )
    .await?;
    let retained = run
        .disclosure_request
        .as_ref()
        .ok_or_else(|| damaged("The Context Assembly of the function request is damaged"))?;
    let receipt_id: String = client
        .query_one(
            "SELECT receipt_id::text FROM storyos.context_assembly_manifests
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND context_assembly_manifest_id = $3::text::uuid",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &run.assembly_manifest_id,
            ],
        )
        .await
        .map_err(complete_database_error)?
        .get(/*idx*/ 0);
    let stored = stored_tool_calls(client, claim).await?;
    let candidates: Vec<ToolResultCandidate<'_>> = stored
        .iter()
        .map(|call| ToolResultCandidate {
            tool_call_id: &call.tool_call_id,
            call_id: &call.call_id,
            owner_user_id: &call.owner_user_id,
            project_id: &call.project_id,
            run_id: &call.run_id,
            model_attempt_id: &call.model_attempt_id,
            decision_id: &call.decision_id,
            state: call.state,
            result_text: call.result_text.as_deref(),
        })
        .collect();
    let next_position = (run
        .decision_position
        .parse::<u64>()
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?
        + 1)
    .to_string();
    let requirement = &retained.operation_requirement;
    let references: Option<Vec<storyos_core::SelectionReference>> =
        requirement.selection_references.as_ref().map(|references| {
            references
                .iter()
                .map(|admitted| admitted.reference.clone())
                .collect()
        });
    persist_current_passage_assembly(
        client,
        &PassageContextInput {
            project_scope: &claim.project_scope,
            run_id: &claim.run_id,
            chapter_id: &run.chapter_id,
            author_message: &run.author_message,
            receipt_id: &receipt_id,
            decision_position: &next_position,
            passage_targets: requirement.passage_targets.as_deref(),
            candidate_target: requirement.candidate_target.as_ref(),
            selection_references: references.as_deref(),
            tool_results: Some(&candidates),
        },
        &requirement.destination_identity,
    )
    .await
    .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    client
        .execute(
            "UPDATE storyos.agent_runs SET active_decision_position = $4::text::numeric
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid AND fence_token = $5",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &next_position,
                &claim.fence_token,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    Ok(WorkPhase::Hold("tool_result_continuation"))
}

/// Each resolved ToolCall of the Run, in item order across its RunSteps.
async fn stored_tool_calls(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<Vec<StoredToolCall>, CompleteAgentRunError> {
    let rows = client
        .query(
            "SELECT tool_call_id::text, call_id, owner_user_id::text, project_id::text,
                    run_id::text, model_attempt_id::text, decision_id::text, state, result_text
               FROM storyos.tool_calls
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid
              ORDER BY decision_position, call_position",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    rows.into_iter()
        .map(|row| {
            Ok(StoredToolCall {
                tool_call_id: row.get(/*idx*/ 0),
                call_id: row.get(/*idx*/ 1),
                owner_user_id: row.get(/*idx*/ 2),
                project_id: row.get(/*idx*/ 3),
                run_id: row.get(/*idx*/ 4),
                model_attempt_id: row.get(/*idx*/ 5),
                decision_id: row.get(/*idx*/ 6),
                state: ToolCallState::parse(row.get(/*idx*/ 7))
                    .ok_or_else(|| damaged("The ToolCall is damaged"))?,
                result_text: row.get(/*idx*/ 8),
            })
        })
        .collect()
}

/// The function outputs of the selected `tool_result` items of `assembly`, in item order. The
/// call of each output is the stored ToolCall that the item names. The outputs of the last
/// Decision answer the prior response; the others belong to earlier RunSteps.
pub(crate) async fn tool_outputs(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    assembly: &CurrentPassageAssemblyRecord,
) -> Result<Vec<ToolOutput>, CompleteAgentRunError> {
    let Some(sources) = &assembly.operation_requirement.tool_results else {
        return Ok(Vec::new());
    };
    let last_decision = sources.last().map(|source| source.decision_id.as_str());
    let selected = assembly
        .selected
        .iter()
        .filter(|item| item.source_class == ContextSourceClass::ToolResult);
    let mut outputs = Vec::new();
    for (source, item) in sources.iter().zip(selected) {
        if item.source_version != source.tool_call_id {
            return Err(damaged("The tool_result Context item is damaged"));
        }
        let row = client
            .query_opt(
                "SELECT function_name, arguments::text FROM storyos.tool_calls
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND run_id = $3::text::uuid AND tool_call_id = $4::text::uuid
                    AND call_id = $5",
                &[
                    &claim.project_scope.owner_user_id.as_ref(),
                    &claim.project_scope.project_id.as_ref(),
                    &claim.run_id,
                    &source.tool_call_id,
                    &source.call_id,
                ],
            )
            .await
            .map_err(complete_database_error)?
            .ok_or_else(|| damaged("The ToolCall of a tool_result is missing"))?;
        let arguments: serde_json::Value = serde_json::from_str(row.get(/*idx*/ 1))
            .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
        outputs.push(ToolOutput {
            call_id: source.call_id.clone(),
            name: row.get(/*idx*/ 0),
            arguments: storyos_core::canonical_json(&arguments),
            output: item.content.clone(),
            origin: if Some(source.decision_id.as_str()) == last_decision {
                ToolOutputOrigin::PriorResponse
            } else {
                ToolOutputOrigin::EarlierStep
            },
        });
    }
    Ok(outputs)
}

/// True when the active RunStep is a `tool_result_continuation` and the text that its request
/// sends, other than its Tool results, differs from that of the RunStep before it. The source
/// version of the author instruction is new in each RunStep, so only the class and the content of
/// each selected item count. The prior response read the items of that RunStep, so an incremental
/// request cannot carry the change.
pub(crate) async fn continuation_input_changed(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<bool, CompleteAgentRunError> {
    let row = client
        .query_opt(
            "WITH items AS (
               SELECT requirement.decision_position,
                      requirement.payload #> '{operation_requirement,tool_results}' IS NOT NULL
                        AS continuation,
                      (SELECT jsonb_agg(jsonb_build_array(item -> 'source_class',
                                                          item -> 'content')
                                        ORDER BY ordinality)
                         FROM jsonb_array_elements(requirement.payload -> 'selected')
                              WITH ORDINALITY AS selected(item, ordinality)
                        WHERE item ->> 'source_class' NOT IN ('tool_result', 'host_control'))
                        AS input
                 FROM storyos.operation_requirements AS requirement
                 JOIN storyos.agent_runs AS run
                   ON (run.owner_user_id, run.project_id, run.run_id) =
                      (requirement.owner_user_id, requirement.project_id, requirement.run_id)
                WHERE requirement.owner_user_id = $1::text::uuid
                  AND requirement.project_id = $2::text::uuid
                  AND requirement.run_id = $3::text::uuid
                  AND requirement.requirement_role = 'primary'
                  AND requirement.decision_position
                      IN (run.active_decision_position, run.active_decision_position - 1))
             SELECT active.input IS DISTINCT FROM earlier.input
               FROM items AS active
               JOIN items AS earlier ON earlier.decision_position = active.decision_position - 1
              WHERE active.continuation",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    Ok(row.is_some_and(|row| row.get(/*idx*/ 0)))
}

/// The lineage of a Proposal that the active RunStep opens, when that RunStep is a
/// `tool_result_continuation`: the Run, the RunStep (its position and Operation Requirement), the
/// Model Attempt, and the ToolCall identities.
pub(crate) async fn proposal_source_lineage(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<Option<String>, tokio_postgres::Error> {
    client
        .query_opt(
            "SELECT jsonb_build_object(
                      'kind', 'tool_result_continuation',
                      'run_id', run.run_id::text,
                      'decision_position', run.active_decision_position,
                      'operation_requirement_id', requirement.operation_requirement_id::text,
                      'model_attempt_id', attempt.model_attempt_id::text,
                      'tool_call_ids', (SELECT jsonb_agg(source->'tool_call_id')
                                          FROM jsonb_array_elements(requirement.payload
                                            #> '{operation_requirement,tool_results}') AS source)
                    )::text
               FROM storyos.agent_runs AS run
               JOIN storyos.operation_requirements AS requirement
                 ON (requirement.owner_user_id, requirement.project_id, requirement.run_id,
                     requirement.decision_position) =
                    (run.owner_user_id, run.project_id, run.run_id, run.active_decision_position)
                AND requirement.requirement_role = 'primary'
               JOIN storyos.model_attempts AS attempt
                 ON (attempt.owner_user_id, attempt.project_id, attempt.run_id,
                     attempt.decision_position) =
                    (run.owner_user_id, run.project_id, run.run_id, run.active_decision_position)
                AND attempt.attempt_role = 'decision'
              WHERE run.owner_user_id = $1::text::uuid AND run.project_id = $2::text::uuid
                AND run.run_id = $3::text::uuid
                AND requirement.payload #> '{operation_requirement,tool_results}' IS NOT NULL",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map(|row| row.map(|row| row.get(/*idx*/ 0)))
}

#[cfg(test)]
#[path = "agent_run_tool_result_tests.rs"]
mod tests;
