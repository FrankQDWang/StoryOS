//! The Tool Exposure of one RunStep (ADR 0084). The Worker reads the latest enablement of each
//! Tool Registration of the Project with its compatibility Decision, its pinned Registration
//! revision, and the lifecycle of that revision, and the Core computes the disposable projection.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{
    EnabledTool, ExposureFacts, ToolCompatibilityDecision, ToolExposure, ToolRegistrationLifecycle,
};

use crate::agent_run_work::{RunPhaseRow, complete_database_error};

/// The Exposure of `run` at this RunStep. A Registration revision without a lifecycle row is the
/// active constant; a row with an unknown lifecycle is damaged.
pub(crate) async fn tool_exposure(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
) -> Result<ToolExposure, CompleteAgentRunError> {
    let rows = client
        .query(
            "SELECT DISTINCT ON (enablement.registration_id)
                    enablement.enablement, decision.decision_kind,
                    binding.tool_registration_revision::text, registration.lifecycle
               FROM storyos.project_tool_enablements AS enablement
               LEFT JOIN storyos.external_contract_compatibility_decisions AS decision
                 ON (decision.owner_user_id, decision.project_id,
                     decision.external_compatibility_decision) =
                    (enablement.owner_user_id, enablement.project_id,
                     enablement.external_compatibility_decision)
               LEFT JOIN storyos.project_external_use_binding_revisions AS binding
                 ON (binding.owner_user_id, binding.project_id,
                     binding.project_model_use_binding_revision) =
                    (decision.owner_user_id, decision.project_id,
                     decision.project_model_use_binding_revision)
               LEFT JOIN storyos.tool_registration_revisions AS registration
                 ON registration.tool_registration_revision = binding.tool_registration_revision
              WHERE enablement.owner_user_id = $1::text::uuid
                AND enablement.project_id = $2::text::uuid
              ORDER BY enablement.registration_id, enablement.enablement_revision DESC",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(complete_database_error)?;
    let mut enabled = Vec::new();
    for row in rows {
        let enablement: &str = row.get(/*idx*/ 0);
        let revision: Option<String> = row.get(/*idx*/ 2);
        let (Some(storyos_core::ToolEnablement::Enabled), Some(registration_revision)) =
            (storyos_core::ToolEnablement::parse(enablement), revision)
        else {
            continue;
        };
        let lifecycle = match row.get::<_, Option<&str>>(/*idx*/ 3) {
            None => ToolRegistrationLifecycle::Active,
            Some(lifecycle) => ToolRegistrationLifecycle::parse(lifecycle).ok_or_else(|| {
                CompleteAgentRunError::Unavailable(Box::new(std::io::Error::other(
                    "Tool Registration is damaged",
                )))
            })?,
        };
        enabled.push(EnabledTool {
            registration_revision,
            lifecycle,
            compatibility: row
                .get::<_, Option<&str>>(/*idx*/ 1)
                .and_then(ToolCompatibilityDecision::parse),
        });
    }
    Ok(ToolExposure::compute(&ExposureFacts {
        enabled: &enabled,
        model_registration_revision: &run.model_registration_revision,
    }))
}

#[cfg(test)]
#[path = "agent_run_tool_exposure_tests.rs"]
pub(crate) mod tests;
