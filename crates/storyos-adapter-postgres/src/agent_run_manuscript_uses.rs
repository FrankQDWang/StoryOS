//! The manuscript Tool uses of a Run, for the Run read (ADR 0095 section 1). This module reads
//! the stored `read_chapter`, `search_manuscript`, and `hybrid_search_manuscript` calls of the
//! Run, and the Core rule `manuscript_uses` decides the entries. It also reads the stored
//! `update_plan` calls, and the Core rule `run_plan` decides the plan (ADR 0103 section 2).

use storyos_application::{CreateAgentRunError, ProjectScope};
use storyos_core::{
    HYBRID_SEARCH_MANUSCRIPT_TOOL_SPEC, ManuscriptUse, READ_CHAPTER_TOOL_SPEC,
    SEARCH_MANUSCRIPT_TOOL_SPEC, StoredManuscriptCall, ToolCallState,
};

use crate::create_agent_run::{agent_run_database_error, agent_run_parse_error};

/// The manuscript Tool uses of the Run `run_id`.
pub(crate) async fn load(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<Vec<ManuscriptUse>, CreateAgentRunError> {
    let stored = stored_calls(
        client,
        scope,
        run_id,
        &[
            READ_CHAPTER_TOOL_SPEC.revision,
            SEARCH_MANUSCRIPT_TOOL_SPEC.revision,
            HYBRID_SEARCH_MANUSCRIPT_TOOL_SPEC.revision,
        ],
    )
    .await?;
    let calls: Vec<StoredManuscriptCall<'_>> = stored
        .iter()
        .map(|call| StoredManuscriptCall {
            tool_spec_revision: &call.tool_spec_revision,
            state: call.state,
            arguments: &call.arguments,
            result_text: call.result_text.as_deref(),
        })
        .collect();
    Ok(storyos_core::manuscript_uses(&calls))
}

/// The plan of the Run `run_id` from its stored `update_plan` calls (ADR 0103 section 2).
pub(crate) async fn load_plan(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<Option<Vec<storyos_core::PlanStep>>, CreateAgentRunError> {
    let revisions = [storyos_core::UPDATE_PLAN_TOOL_SPEC.revision];
    let stored = stored_calls(client, scope, run_id, &revisions).await?;
    let calls: Vec<storyos_core::StoredPlanCall<'_>> = (stored.iter())
        .map(|call| storyos_core::StoredPlanCall {
            state: call.state,
            arguments: &call.arguments,
        })
        .collect();
    Ok(storyos_core::run_plan(&calls))
}

/// One stored ToolCall of a Run, for a read of the Tool uses of the Run.
pub(crate) struct StoredCall {
    pub(crate) tool_spec_revision: String,
    pub(crate) state: ToolCallState,
    pub(crate) arguments: serde_json::Value,
    pub(crate) result_text: Option<String>,
}

/// The stored ToolCalls of the Run `run_id` with a ToolSpec revision in `revisions`, in the
/// order of their RunSteps and calls.
pub(crate) async fn stored_calls(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    run_id: &str,
    revisions: &[&str],
) -> Result<Vec<StoredCall>, CreateAgentRunError> {
    let rows = client
        .query(
            "SELECT tool_spec_revision, state, arguments::text, result_text
               FROM storyos.tool_calls
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid AND tool_spec_revision = ANY($4::text[])
              ORDER BY decision_position, call_position",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
                &revisions,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    let mut stored = Vec::with_capacity(rows.len());
    for row in &rows {
        let state = ToolCallState::parse(row.get(/*idx*/ 1)).ok_or_else(|| {
            CreateAgentRunError::Unavailable(Box::new(std::io::Error::other(
                "The ToolCall state is unknown",
            )))
        })?;
        stored.push(StoredCall {
            tool_spec_revision: row.get(/*idx*/ 0),
            state,
            arguments: serde_json::from_str(row.get(/*idx*/ 2)).map_err(agent_run_parse_error)?,
            result_text: row.get(/*idx*/ 3),
        });
    }
    Ok(stored)
}

#[cfg(test)]
#[path = "agent_run_manuscript_uses_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "agent_run_plan_tests.rs"]
mod plan_tests;
