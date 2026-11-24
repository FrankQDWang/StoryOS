//! The Skills that a Run used, for the Run read (ADR 0089 section 8). This module reads the
//! Skill selection of the first RunStep and the stored `load_skill` calls of the Run, and the
//! Core rule `skill_uses` decides the entries.

use storyos_application::{CreateAgentRunError, ProjectScope};
use storyos_core::{SkillRequirement, SkillUse, StoredSkillLoad};

use crate::create_agent_run::{agent_run_database_error, agent_run_parse_error};

/// The Skills that the Run `run_id` used.
pub(crate) async fn load(
    client: &crate::statement_cache::Client,
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
    let stored = crate::agent_run_manuscript_uses::stored_calls(
        client,
        scope,
        run_id,
        &[storyos_core::LOAD_SKILL_TOOL_SPEC.revision],
    )
    .await?;
    let loads: Vec<StoredSkillLoad<'_>> = stored
        .iter()
        .map(|call| StoredSkillLoad {
            state: call.state,
            arguments: &call.arguments,
        })
        .collect();
    Ok(storyos_core::skill_uses(&selected, &loads))
}

#[cfg(test)]
#[path = "agent_run_skill_uses_tests.rs"]
mod tests;
