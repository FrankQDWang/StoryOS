//! The in-process execution of a `read_skill_resource` call (ADR 0089 sections 3, 7, and 9). It
//! reads the pinned catalog of the Run, the Skills that the Run has, the count of its settled
//! reads, and the file of the Snapshot with the exact path, and it settles the call with the exact
//! text of the file, or refuses it with the reason. It reads only Snapshot rows, never a Skill
//! directory, and it writes no Outbound Disclosure Event.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{READ_SKILL_RESOURCE_TOOL_SPEC, SkillResourceReadFacts, ToolCallState};

use crate::agent_run_load_skill::{CountedLoads, held_skills};
use crate::agent_run_tool_authority::{Resolution, ToolCallRow, resolve};
use crate::agent_run_work::complete_unavailable;

/// Reads the file `path` of the Skill `skill` for the pending call `row`, and settles or refuses
/// `row`. A load of the response of `row` does not count, because its output is not in the
/// conversation of the model yet.
pub(crate) async fn execute(
    client: &crate::statement_cache::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    skill: &str,
    path: &str,
) -> Result<(), CompleteAgentRunError> {
    let scope = &claim.project_scope;
    let catalog: Vec<_> =
        crate::skill_package_snapshot::run_skill_catalog(client, scope, &claim.run_id)
            .await
            .map_err(complete_unavailable)?
            .into_iter()
            .map(|entry| entry.reference)
            .collect();
    let (selected, loaded) = held_skills(
        client,
        claim,
        CountedLoads::BeforeResponseOf(&row.tool_call_id),
    )
    .await?;
    let stored = client
        .query_one(
            "SELECT (SELECT count(*) FROM storyos.tool_calls AS call
                      WHERE call.owner_user_id = $1::text::uuid
                        AND call.project_id = $2::text::uuid AND call.run_id = $3::text::uuid
                        AND call.tool_spec_revision = $4 AND call.state = 'settled'),
                    (SELECT file.bytes
                       FROM storyos.agent_run_skill_catalog_entries AS entry
                       JOIN storyos.skill_package_snapshot_files AS file
                         ON (file.owner_user_id, file.project_id, file.package_digest) =
                            (entry.owner_user_id, entry.project_id, entry.package_digest)
                      WHERE entry.owner_user_id = $1::text::uuid
                        AND entry.project_id = $2::text::uuid AND entry.run_id = $3::text::uuid
                        AND entry.skill_name = $5 AND file.path = $6)",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &claim.run_id,
                &READ_SKILL_RESOURCE_TOOL_SPEC.revision,
                &skill,
                &path,
            ],
        )
        .await
        .map_err(complete_unavailable)?;
    let settled_reads: i64 = stored.get(/*idx*/ 0);
    let file: Option<Vec<u8>> = stored.get(/*idx*/ 1);
    let facts = SkillResourceReadFacts {
        catalog: &catalog,
        selected: &selected,
        loaded: &loaded,
        settled_reads: usize::try_from(settled_reads).unwrap_or(usize::MAX),
        file: file.as_deref(),
    };
    let (state, refusal, result) =
        match storyos_core::admit_skill_resource_read(&facts, skill, path) {
            Ok(result) => (ToolCallState::Settled, None, result),
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
        Resolution::host(state, refusal, Some(&result)),
    )
    .await
}

#[cfg(test)]
#[path = "agent_run_read_skill_resource_tests.rs"]
mod tests;
