//! The reads of the Subruns of a Run of the author (ADR 0103 section 3A, "The reads"). The
//! records come from the Subrun Requests, the Subrun rows, their Model Attempts, and their stored
//! ToolCalls. The Core rules `subrun_state` and `subrun_steps` decide the state and the entries.

use crate::statement_cache::Client;
use storyos_application::{CreateAgentRunError, ProjectScope, SubrunEntry, SubrunRecord};
use storyos_core::{
    HYBRID_SEARCH_MANUSCRIPT_TOOL_SPEC, READ_CHAPTER_TOOL_SPEC, READ_MEMORY_ENTRY_TOOL_SPEC,
    SEARCH_MANUSCRIPT_TOOL_SPEC, SEARCH_MEMORY_TOOL_SPEC, StoredManuscriptCall,
};

use crate::create_agent_run::{agent_run_database_error, agent_run_parse_error};
use crate::subrun::{subrun_end, subrun_end_columns};

/// The Subrun Requests of the Run `run_id` in the order of their calls, or `None` when the Run
/// has none. A refused `start_subrun` call has no Subrun Request.
pub(crate) async fn load_entries(
    client: &Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<Option<Vec<SubrunEntry>>, CreateAgentRunError> {
    let rows = client
        .query(
            concat!(
                "SELECT request.subrun_run_id::text, request.title, ",
                subrun_end_columns!(),
                " FROM storyos.subrun_requests AS request
                  JOIN storyos.tool_calls AS call
                    ON (call.owner_user_id, call.project_id, call.tool_call_id) =
                       (request.owner_user_id, request.project_id, request.parent_tool_call_id)
                  JOIN storyos.agent_runs AS subrun
                    ON (subrun.owner_user_id, subrun.project_id, subrun.run_id) =
                       (request.owner_user_id, request.project_id, request.subrun_run_id)
                 WHERE request.owner_user_id = $1::text::uuid
                   AND request.project_id = $2::text::uuid
                   AND request.parent_run_id = $3::text::uuid
                 ORDER BY call.decision_position, call.call_position"
            ),
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    let mut entries = Vec::with_capacity(rows.len());
    for row in &rows {
        entries.push(SubrunEntry {
            subrun_id: row.get(/*idx*/ 0),
            title: row.get(/*idx*/ 1),
            state: storyos_core::subrun_state(subrun_end(row, 2).ok_or_else(damaged)?),
        });
    }
    Ok((!entries.is_empty()).then_some(entries))
}

/// The record of the Subrun `subrun_id`, or `None` when the Project has no Subrun Request with
/// that Subrun.
pub(crate) async fn read(
    client: &Client,
    scope: &ProjectScope,
    subrun_id: &str,
) -> Result<Option<SubrunRecord>, CreateAgentRunError> {
    let ids: [&(dyn tokio_postgres::types::ToSql + Sync); 3] = [
        &scope.owner_user_id.as_ref(),
        &scope.project_id.as_ref(),
        &subrun_id,
    ];
    let Some(row) = client
        .query_opt(
            concat!(
                "SELECT request.title, request.brief, ",
                subrun_end_columns!(),
                ", (SELECT count(DISTINCT attempt.decision_position)
                     FROM storyos.model_attempts AS attempt
                    WHERE (attempt.owner_user_id, attempt.project_id, attempt.run_id) =
                          (subrun.owner_user_id, subrun.project_id, subrun.run_id)
                      AND attempt.attempt_role IN ('decision', 'successor')
                      AND attempt.dispatch_state = 'settled'
                      -- A request that was not submitted or that was rejected has no response.
                      AND NOT attempt.payload ?| array['submission', 'rejection'])
                  FROM storyos.subrun_requests AS request
                  JOIN storyos.agent_runs AS subrun
                    ON (subrun.owner_user_id, subrun.project_id, subrun.run_id) =
                       (request.owner_user_id, request.project_id, request.subrun_run_id)
                 WHERE request.owner_user_id = $1::text::uuid
                   AND request.project_id = $2::text::uuid
                   AND request.subrun_run_id = $3::text::uuid"
            ),
            &ids,
        )
        .await
        .map_err(agent_run_database_error)?
    else {
        return Ok(None);
    };
    let end = subrun_end(&row, 2).ok_or_else(damaged)?;
    // A text of a response with function calls, with the call position of the first ToolCall
    // after it. The final answer is the text of an `advisory` Decision, also when its response
    // has a function call of the last RunStep. The place of a text comes from the stored items,
    // which keep the order in which the stream gave each item for the first time. The read proves
    // no order for a stream that gives its items in an order different from the terminal response.
    let texts = client
        .query(
            "SELECT attempt.decision_position::text, item->>'text',
                    (SELECT COALESCE(min(call.call_position), 2147483647)
                       FROM jsonb_array_elements(attempt.payload->'items')
                            WITH ORDINALITY AS later(other, place)
                       JOIN storyos.tool_calls AS call
                         ON (call.owner_user_id, call.project_id, call.model_attempt_id, call.call_id)
                          = (attempt.owner_user_id, attempt.project_id, attempt.model_attempt_id,
                             later.other->>'call_id')
                      WHERE later.place > entry.place)::text
               FROM storyos.model_attempts AS attempt,
                    jsonb_array_elements(attempt.payload->'items') WITH ORDINALITY AS entry(item, place)
              WHERE attempt.owner_user_id = $1::text::uuid AND attempt.project_id = $2::text::uuid
                AND attempt.run_id = $3::text::uuid
                AND attempt.attempt_role IN ('decision', 'successor')
                AND attempt.dispatch_state = 'settled'
                AND attempt.payload->'items' @> '[{\"native_type\": \"function_call\"}]'
                AND attempt.payload #>> '{decision,kind}' IS DISTINCT FROM 'advisory'
                AND item->>'role' = 'assistant' AND item->>'text' <> ''
              ORDER BY attempt.decision_position, attempt.model_attempt_id, entry.place",
            &ids,
        )
        .await
        .map_err(agent_run_database_error)?;
    let texts = (texts.iter())
        .map(|row| {
            let before = position(row.get(/*idx*/ 2))?;
            Ok((position(row.get(/*idx*/ 0))?, before, row.get(/*idx*/ 1)))
        })
        .collect::<Result<Vec<(u64, u64, String)>, CreateAgentRunError>>()?;
    let revisions = [
        READ_CHAPTER_TOOL_SPEC.revision,
        SEARCH_MANUSCRIPT_TOOL_SPEC.revision,
        HYBRID_SEARCH_MANUSCRIPT_TOOL_SPEC.revision,
        READ_MEMORY_ENTRY_TOOL_SPEC.revision,
        SEARCH_MEMORY_TOOL_SPEC.revision,
    ];
    let stored =
        crate::agent_run_manuscript_uses::stored_calls(client, scope, subrun_id, &revisions)
            .await?;
    let calls: Vec<(u64, u64, StoredManuscriptCall<'_>)> = (stored.iter())
        .map(|call| {
            let stored = StoredManuscriptCall {
                tool_spec_revision: &call.tool_spec_revision,
                state: call.state,
                arguments: &call.arguments,
                result_text: call.result_text.as_deref(),
            };
            (call.decision_position, call.call_position, stored)
        })
        .collect();
    let state = storyos_core::subrun_state(end);
    Ok(Some(SubrunRecord {
        title: row.get(/*idx*/ 0),
        brief: row.get(/*idx*/ 1),
        state,
        step_count: u32::try_from(row.get::<_, i64>(/*idx*/ 5)).map_err(agent_run_parse_error)?,
        steps: storyos_core::subrun_steps(&texts, &calls),
        // Only a `completed` Subrun has its final answer.
        answer: (state == storyos_core::SubrunState::Completed)
            .then(|| end.final_answer.map(str::to_owned))
            .flatten(),
    }))
}

fn position(text: &str) -> Result<u64, CreateAgentRunError> {
    text.parse().map_err(agent_run_parse_error)
}

fn damaged() -> CreateAgentRunError {
    CreateAgentRunError::Unavailable("The status of a Subrun is damaged".into())
}
