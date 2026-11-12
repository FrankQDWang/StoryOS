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
    sources: Vec<storyos_core::CitableSource>,
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
    let (Some(attempt_id), Some(_)) = (&run.attempt_id, &run.decision_id) else {
        return Err(damaged("The function request Decision is missing"));
    };
    let payload: serde_json::Value =
        serde_json::from_str(run.attempt_payload.as_deref().unwrap_or("null"))
            .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    crate::agent_run_continuation::bind_produced_continuation(
        client, claim, attempt_id, &payload, &payload,
    )
    .await?;
    open_next_step(client, claim, run).await
}

/// Commits gates one to six of the next Context Assembly and makes it the active RunStep. A
/// successor Decision calls this directly: its Attempt already holds the produced binding of its
/// function request, so the next RunStep continues the successor response (ADR 0088).
pub(crate) async fn open_next_step(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
) -> Result<WorkPhase, CompleteAgentRunError> {
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
            sources: &call.sources,
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
               FROM storyos.tool_calls AS call
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
    let sources = crate::research_artifact::run_sources(client, claim).await?;
    rows.into_iter()
        .map(|row| {
            let tool_call_id: String = row.get(/*idx*/ 0);
            Ok(StoredToolCall {
                sources: sources
                    .iter()
                    .filter(|(call, _)| *call == tool_call_id)
                    .map(|(_, source)| source.clone())
                    .collect(),
                tool_call_id,
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
/// call of each output is the stored ToolCall that the item names. The outputs of the Decision of
/// the prior RunStep answer the prior response; the others belong to earlier RunSteps. A prior
/// RunStep with a text and no function call has no Decision (ADR 0091).
pub(crate) async fn tool_outputs(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    assembly: &CurrentPassageAssemblyRecord,
) -> Result<Vec<ToolOutput>, CompleteAgentRunError> {
    let Some(sources) = &assembly.operation_requirement.tool_results else {
        return Ok(Vec::new());
    };
    let prior_decisions: Vec<String> = client
        .query(
            "SELECT attempt.decision_id::text FROM storyos.model_attempts AS attempt
               JOIN storyos.agent_runs AS run USING (owner_user_id, project_id, run_id)
              WHERE attempt.owner_user_id = $1::text::uuid AND attempt.project_id = $2::text::uuid
                AND attempt.run_id = $3::text::uuid AND attempt.decision_id IS NOT NULL
                AND attempt.decision_position = run.active_decision_position - 1",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map_err(complete_database_error)?
        .iter()
        .map(|row| row.get(/*idx*/ 0))
        .collect();
    let selected = assembly
        .selected
        .iter()
        .filter(|item| item.source_class == ContextSourceClass::ToolResult);
    let pairs: Vec<_> = sources.iter().zip(selected).collect();
    if pairs.is_empty() {
        return Ok(Vec::new());
    }
    let (tool_call_ids, call_ids): (Vec<&str>, Vec<&str>) = pairs
        .iter()
        .map(|(source, _)| (source.tool_call_id.as_str(), source.call_id.as_str()))
        .unzip();
    let mut stored: std::collections::BTreeMap<i64, tokio_postgres::Row> = client
        .query(
            "SELECT input.ordinal, call.function_name, call.arguments::text
               FROM unnest($4::text[], $5::text[]) WITH ORDINALITY
                    AS input(tool_call_id, call_id, ordinal)
               JOIN storyos.tool_calls AS call
                 ON call.owner_user_id = $1::text::uuid AND call.project_id = $2::text::uuid
                AND call.run_id = $3::text::uuid AND call.tool_call_id = input.tool_call_id::uuid
                AND call.call_id = input.call_id",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &tool_call_ids,
                &call_ids,
            ],
        )
        .await
        .map_err(complete_database_error)?
        .into_iter()
        .map(|row| (row.get(/*idx*/ 0), row))
        .collect();
    let mut outputs = Vec::new();
    for (ordinal, (source, item)) in (1_i64..).zip(pairs) {
        if item.source_version != source.source_version() {
            return Err(damaged("The tool_result Context item is damaged"));
        }
        let row = stored
            .remove(&ordinal)
            .ok_or_else(|| damaged("The ToolCall of a tool_result is missing"))?;
        let arguments: serde_json::Value = serde_json::from_str(row.get(/*idx*/ 2))
            .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
        outputs.push(ToolOutput {
            call_id: source.call_id.clone(),
            name: row.get(/*idx*/ 1),
            arguments: storyos_core::canonical_json(&arguments),
            output: item.content.clone(),
            origin: if prior_decisions.contains(&source.decision_id) {
                ToolOutputOrigin::PriorResponse
            } else {
                ToolOutputOrigin::EarlierStep
            },
        });
    }
    Ok(outputs)
}

/// True when the active RunStep is a `tool_result_continuation` and the input that its request
/// sends, other than its Tool outputs, differs from the input of the function request Attempt
/// before it. The input is all that the model reads: the selected Context text, the declared
/// targets, the selection references, and the passage input. The prior response read the input
/// of that Attempt, so an incremental request cannot carry a change.
pub(crate) async fn continuation_input_changed(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<bool, CompleteAgentRunError> {
    let Some(row) = client
        .query_opt(
            "SELECT requirement.payload::text, run.author_message, run.chapter_id::text,
                    attempt.payload #>> '{wire,current_input_digest}'
               FROM storyos.agent_runs AS run
               JOIN storyos.operation_requirements AS requirement
                 ON (requirement.owner_user_id, requirement.project_id, requirement.run_id,
                     requirement.decision_position) =
                    (run.owner_user_id, run.project_id, run.run_id, run.active_decision_position)
                AND requirement.requirement_role = 'primary'
               LEFT JOIN storyos.model_attempts AS attempt
                 ON (attempt.owner_user_id, attempt.project_id, attempt.run_id) =
                    (run.owner_user_id, run.project_id, run.run_id)
                AND attempt.attempt_role = 'decision'
                AND attempt.decision_position = run.active_decision_position - 1
              WHERE run.owner_user_id = $1::text::uuid
                AND run.project_id = $2::text::uuid
                AND run.run_id = $3::text::uuid
                AND requirement.payload #> '{operation_requirement,tool_results}' IS NOT NULL",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map_err(complete_database_error)?
    else {
        return Ok(false);
    };
    let record: serde_json::Value = serde_json::from_str(row.get::<_, &str>(/*idx*/ 0))
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    let assembly = storyos_core::decode_assembly_record(&record)
        .ok_or_else(|| damaged("The continuation Context is invalid"))?;
    let author_message: String = row.get(/*idx*/ 1);
    let chapter_id: String = row.get(/*idx*/ 2);
    let declared_targets =
        crate::agent_run_create_dispatch::declared_targets(client, claim, &chapter_id).await?;
    let input = crate::agent_run_create_dispatch::record_input(&record, assembly, &author_message);
    let current = storyos_application::CurrentInput {
        author_message: &author_message,
        chapter_id: &chapter_id,
        passage_resolution: input.passage_resolution,
        passage_input: input.passage_input.as_ref(),
        context: &input.context,
        declared_targets: &declared_targets,
        selection_references: &input.selection_references,
    };
    let prior: Option<String> = row.get(/*idx*/ 3);
    Ok(prior.as_deref() != Some(current.digest().as_str()))
}

/// The lineage of a Proposal that the active RunStep opens, when that RunStep is a
/// `tool_result_continuation`: the Run, the RunStep (its position and Operation Requirement), the
/// Model Attempt, and the ToolCall identities. Otherwise, the lineage names the Research Artifact
/// whose response the request continues, if any (ADR 0087).
pub(crate) async fn proposal_source_lineage(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<Option<String>, tokio_postgres::Error> {
    let lineage = client
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
        .await?
        .map(|row| row.get(/*idx*/ 0));
    match lineage {
        Some(lineage) => Ok(Some(lineage)),
        None => crate::research_artifact::proposal_lineage(client, claim).await,
    }
}

#[cfg(test)]
#[path = "agent_run_tool_result_tests.rs"]
mod tests;
