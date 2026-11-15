//! The Skills that a Run used, for the Run read (ADR 0089 section 8). This module reads the
//! Skill selection of the first RunStep and the stored `load_skill` calls of the Run, and the
//! Core rule `skill_uses` decides the entries.

use storyos_application::{CreateAgentRunError, ProjectScope};
use storyos_core::{SkillRequirement, SkillUse, StoredSkillLoad, ToolCallState};

use crate::create_agent_run::{agent_run_database_error, agent_run_parse_error};

/// The Skills that the Run `run_id` used.
pub(crate) async fn load(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<Vec<SkillUse>, CreateAgentRunError> {
    let (owner_user_id, project_id) = (scope.owner_user_id.as_ref(), scope.project_id.as_ref());
    // Each later RunStep keeps the Skill selection of the first RunStep.
    let selection = client
        .query_opt(
            "SELECT (payload #> '{operation_requirement,skill_requirements}')::text
               FROM storyos.context_assembly_manifests
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid AND manifest_role = 'decision'
              ORDER BY decision_position LIMIT 1",
            &[&owner_user_id, &project_id, &run_id],
        )
        .await
        .map_err(agent_run_database_error)?
        .and_then(|row| row.get::<_, Option<String>>(/*idx*/ 0));
    let selected: Vec<SkillRequirement> = match selection {
        Some(text) => serde_json::from_str(&text).map_err(agent_run_parse_error)?,
        None => Vec::new(),
    };
    let rows = client
        .query(
            "SELECT state, arguments::text FROM storyos.tool_calls
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid AND tool_spec_revision = $4
              ORDER BY decision_position, call_position",
            &[
                &owner_user_id,
                &project_id,
                &run_id,
                &storyos_core::LOAD_SKILL_TOOL_SPEC.revision,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    let mut loads = Vec::with_capacity(rows.len());
    for row in &rows {
        let state = ToolCallState::parse(row.get(/*idx*/ 0)).ok_or_else(|| {
            CreateAgentRunError::Unavailable(Box::new(std::io::Error::other(
                "The ToolCall state is unknown",
            )))
        })?;
        let arguments: serde_json::Value =
            serde_json::from_str(row.get(/*idx*/ 1)).map_err(agent_run_parse_error)?;
        loads.push((state, arguments));
    }
    let loads: Vec<StoredSkillLoad<'_>> = loads
        .iter()
        .map(|(state, arguments)| StoredSkillLoad {
            state: *state,
            arguments,
        })
        .collect();
    Ok(storyos_core::skill_uses(&selected, &loads))
}

#[cfg(test)]
#[path = "agent_run_skill_uses_tests.rs"]
mod tests;
