//! The in-process execution of a `load_skill` call (ADR 0089 sections 3 and 7). It reads the
//! pinned catalog of the Run and the `SKILL.md` of the Snapshot that the catalog names, and it
//! settles the call with the exact text, or refuses it with the reason. It reads no Skill
//! directory and writes no Outbound Disclosure Event.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{SkillPackageReference, ToolCallState};

use crate::agent_run_tool_authority::{Resolution, ToolCallRow, damaged, resolve};
use crate::agent_run_work::complete_database_error;

/// Loads the Skill `skill` for the pending call `row`, and settles or refuses `row`.
pub(crate) async fn execute(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    skill: &str,
) -> Result<(), CompleteAgentRunError> {
    let scope = &claim.project_scope;
    let catalog: Vec<SkillPackageReference> =
        crate::skill_package_snapshot::run_skill_catalog(client, scope, &claim.run_id)
            .await
            .map_err(complete_database_error)?
            .into_iter()
            .map(|entry| entry.reference)
            .collect();
    let (state, refusal, result) = match storyos_core::admit_skill_load(&catalog, skill) {
        Ok(reference) => {
            let skill_md = crate::skill_package_snapshot::skill_md(client, scope, reference)
                .await
                .map_err(complete_database_error)?;
            let result = storyos_core::skill_load_result(skill_md)
                .ok_or_else(|| damaged("The SkillPackage Snapshot has no SKILL.md text"))?;
            (ToolCallState::Settled, None, result)
        }
        Err(refusal) => (
            ToolCallState::Refused,
            Some(refusal.label()),
            refusal.result(),
        ),
    };
    resolve(
        client,
        claim,
        row,
        Resolution {
            state,
            refusal,
            attempt: None,
            outcome: None,
            result: Some(&result),
        },
    )
    .await
}

#[cfg(test)]
#[path = "agent_run_load_skill_tests.rs"]
mod tests;
