//! The Tool Exposure of one RunStep (ADR 0084, ADR 0090). The Worker reads the current research
//! Decision of the Project with its pinned Registration revision and the lifecycle of that
//! revision, and the Core computes the disposable projection.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{
    BoundTool, ExposureFacts, ToolCompatibilityDecision, ToolExposure, ToolRegistrationLifecycle,
};

use crate::agent_run_work::{RunPhaseRow, complete_unavailable};

/// The Exposure of `run` at this RunStep. A Registration revision without a lifecycle row is the
/// active constant; a row with an unknown lifecycle is damaged.
pub(crate) async fn tool_exposure(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
) -> Result<ToolExposure, CompleteAgentRunError> {
    let rows = client
        .query(
            "SELECT decision.decision_kind, binding.tool_registration_revision::text,
                    registration.lifecycle
               FROM (SELECT * FROM storyos.external_contract_compatibility_decisions
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                        AND decision_kind = 'tool_registration_compatible'
                      ORDER BY external_compatibility_decision DESC LIMIT 1) AS decision
               JOIN storyos.project_external_use_binding_revisions AS binding
                 ON (binding.owner_user_id, binding.project_id,
                     binding.project_model_use_binding_revision) =
                    (decision.owner_user_id, decision.project_id,
                     decision.project_model_use_binding_revision)
               LEFT JOIN storyos.tool_registration_revisions AS registration
                 ON registration.tool_registration_revision = binding.tool_registration_revision",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(complete_unavailable)?;
    let mut bound = Vec::new();
    for row in rows {
        let Some(registration_revision) = row.get::<_, Option<String>>(/*idx*/ 1) else {
            continue;
        };
        let lifecycle = match row.get::<_, Option<&str>>(/*idx*/ 2) {
            None => ToolRegistrationLifecycle::Active,
            Some(lifecycle) => ToolRegistrationLifecycle::parse(lifecycle).ok_or_else(|| {
                CompleteAgentRunError::Unavailable(Box::new(std::io::Error::other(
                    "Tool Registration is damaged",
                )))
            })?,
        };
        bound.push(BoundTool {
            registration_revision,
            lifecycle,
            compatibility: ToolCompatibilityDecision::parse(row.get(/*idx*/ 0)),
        });
    }
    // The pinned catalog of the Run decides the Skill Tool (ADR 0089 section 3).
    let skill_catalog: bool = client
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM storyos.agent_run_skill_catalog_entries
                             WHERE owner_user_id = $1::text::uuid
                               AND project_id = $2::text::uuid AND run_id = $3::text::uuid)",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map_err(complete_unavailable)?
        .get(/*idx*/ 0);
    // The captured Conversation Memory Settings of the Run decide the Memory read (ADR 0094
    // section 4). A background Run has no settings, and a Subrun uses the settings of its parent
    // Run (ADR 0103 section 3A).
    let memory_use: bool = client
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM storyos.agent_runs AS run
                              JOIN storyos.conversation_memory_settings AS settings
                                ON (settings.owner_user_id, settings.project_id,
                                    settings.conversation_id, settings.memory_settings_revision) =
                                   (run.owner_user_id, run.project_id, run.conversation_id,
                                    run.memory_settings_revision)
                             WHERE run.owner_user_id = $1::text::uuid
                               AND run.project_id = $2::text::uuid
                               AND run.run_id = COALESCE(
                                     (SELECT request.parent_run_id
                                        FROM storyos.subrun_requests AS request
                                       WHERE request.owner_user_id = $1::text::uuid
                                         AND request.project_id = $2::text::uuid
                                         AND request.subrun_run_id = $3::text::uuid),
                                     $3::text::uuid)
                               AND settings.use_enabled)",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map_err(complete_unavailable)?
        .get(/*idx*/ 0);
    let hybrid_search = crate::agent_run_hybrid_search::possible(client, claim).await?;
    Ok(ToolExposure::compute(&ExposureFacts {
        bound: &bound,
        model_registration_revision: &run.model_registration_revision,
        skill_catalog,
        memory_use,
        hybrid_search,
        run_kind: run.kind,
    }))
}

#[cfg(test)]
#[path = "agent_run_tool_exposure_tests.rs"]
pub(crate) mod tests;
