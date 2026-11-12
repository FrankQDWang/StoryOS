//! The Tool Exposure of one RunStep (ADR 0084, ADR 0090). The Worker reads the current research
//! Decision of the Project with its pinned Registration revision and the lifecycle of that
//! revision, and the Core computes the disposable projection.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{
    BoundTool, ExposureFacts, ToolCompatibilityDecision, ToolExposure, ToolRegistrationLifecycle,
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
        .map_err(complete_database_error)?;
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
    Ok(ToolExposure::compute(&ExposureFacts {
        bound: &bound,
        model_registration_revision: &run.model_registration_revision,
    }))
}

#[cfg(test)]
#[path = "agent_run_tool_exposure_tests.rs"]
pub(crate) mod tests;
