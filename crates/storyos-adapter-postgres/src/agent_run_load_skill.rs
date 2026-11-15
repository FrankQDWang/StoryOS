//! The in-process execution of a `load_skill` call (ADR 0089 sections 3, 7, and 9). It reads the
//! pinned catalog of the Run, the selected Skills, the settled loads, and the `SKILL.md` of the
//! Snapshot that the catalog names, and it settles the call with the exact text, or refuses it
//! with the reason. It reads no Skill directory and writes no Outbound Disclosure Event.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{LOAD_SKILL_TOOL_SPEC, SkillLoadFacts, SkillPackageReference, ToolCallState};

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
    let (selected, loaded) = held_skills(client, claim).await?;
    let facts = SkillLoadFacts {
        catalog: &catalog,
        selected: &selected,
        loaded: &loaded,
    };
    let (state, refusal, result) = match storyos_core::admit_skill_load(&facts, skill) {
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

/// The name of each Skill that the author selected for the claimed Run, from the Operation
/// Requirement of its first RunStep, and the Skill name of each settled `load_skill` call of the
/// Run. The Tool Gateway settles the calls of one response in call order in one transaction, so
/// each settled call counts for the calls after it.
async fn held_skills(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<(Vec<String>, Vec<String>), CompleteAgentRunError> {
    let row = client
        .query_one(
            "SELECT ARRAY(SELECT requirement ->> 'skill_name'
                            FROM storyos.context_assembly_manifests AS manifest,
                                 jsonb_array_elements(manifest.payload
                                   #> '{operation_requirement,skill_requirements}') AS requirement
                           WHERE manifest.owner_user_id = $1::text::uuid
                             AND manifest.project_id = $2::text::uuid
                             AND manifest.run_id = $3::text::uuid
                             AND manifest.manifest_role = 'decision'
                             AND manifest.decision_position = 0),
                    ARRAY(SELECT call.arguments ->> 'skill' FROM storyos.tool_calls AS call
                           WHERE call.owner_user_id = $1::text::uuid
                             AND call.project_id = $2::text::uuid
                             AND call.run_id = $3::text::uuid
                             AND call.tool_spec_revision = $4 AND call.state = 'settled')",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &LOAD_SKILL_TOOL_SPEC.revision,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    Ok((row.get(/*idx*/ 0), row.get(/*idx*/ 1)))
}

#[cfg(test)]
#[path = "agent_run_load_skill_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "agent_run_load_skill_limit_tests.rs"]
mod limit_tests;
