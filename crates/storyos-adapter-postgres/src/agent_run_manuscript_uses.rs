//! The manuscript Tool uses of a Run, for the Run read (ADR 0095 section 1). This module reads
//! the stored `read_chapter` and `search_manuscript` calls of the Run, and the Core rule
//! `manuscript_uses` decides the entries.

use storyos_application::{CreateAgentRunError, ProjectScope};
use storyos_core::{
    ManuscriptUse, READ_CHAPTER_TOOL_SPEC, SEARCH_MANUSCRIPT_TOOL_SPEC, StoredManuscriptCall,
    ToolCallState,
};

use crate::create_agent_run::{agent_run_database_error, agent_run_parse_error};

/// The manuscript Tool uses of the Run `run_id`.
pub(crate) async fn load(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<Vec<ManuscriptUse>, CreateAgentRunError> {
    let (owner_user_id, project_id) = (scope.owner_user_id.as_ref(), scope.project_id.as_ref());
    let rows = client
        .query(
            "SELECT tool_spec_revision, state, arguments::text, result_text
               FROM storyos.tool_calls
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid AND tool_spec_revision IN ($4, $5)
              ORDER BY decision_position, call_position",
            &[
                &owner_user_id,
                &project_id,
                &run_id,
                &READ_CHAPTER_TOOL_SPEC.revision,
                &SEARCH_MANUSCRIPT_TOOL_SPEC.revision,
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
        let arguments: serde_json::Value =
            serde_json::from_str(row.get(/*idx*/ 2)).map_err(agent_run_parse_error)?;
        stored.push((
            row.get::<_, String>(/*idx*/ 0),
            state,
            arguments,
            row.get::<_, Option<String>>(/*idx*/ 3),
        ));
    }
    let calls: Vec<StoredManuscriptCall<'_>> = stored
        .iter()
        .map(
            |(revision, state, arguments, result)| StoredManuscriptCall {
                tool_spec_revision: revision,
                state: *state,
                arguments,
                result_text: result.as_deref(),
            },
        )
        .collect();
    Ok(storyos_core::manuscript_uses(&calls))
}

#[cfg(test)]
#[path = "agent_run_manuscript_uses_tests.rs"]
mod tests;
