//! The research records that `createAgentRun` makes current for the research destination of the
//! deployment (ADR 0083, ADR 0090): the Processing Destination Identity of the server, the Project
//! Destination Grant `research_tool_v1`, the use binding with the Credential Reference, and the
//! compatibility Decision from the pinned active Registration revision. The Server performs no MCP
//! exchange; the Worker checks the pinned digests before each dispatch. The current research
//! Decision of a Project is its latest `tool_registration_compatible` Decision.

use storyos_application::{ProjectCommandEnvelope, ProjectCommandError, ProjectScope};
use storyos_core::{GrantScope, ResearchToolOffer, ToolRegistrationLifecycle};
use tokio_postgres::Client;

use crate::command_sequence::unavailable;
use crate::project_destination_binding::{
    BindingDecision, BindingRecord, BoundRegistration, insert_binding,
};

/// What one `createAgentRun` writes for the research destination of the deployment.
#[derive(Clone, Copy, Debug)]
pub(crate) enum ResearchPlan {
    /// The deployment offers no research Registration, the offered revision is not active, or the
    /// current research records of the Project match the offer.
    Keep,
    /// The Project has no research records, or its current records differ from the offer.
    Bind(&'static ResearchToolOffer),
}

/// The plan for `offer` in the Project of `scope`. A revision without a lifecycle row is the
/// active constant, because the row appears at the first binding of the deployment.
pub(crate) async fn research_plan(
    client: &Client,
    scope: &ProjectScope,
    offer: Option<&'static ResearchToolOffer>,
) -> Result<ResearchPlan, ProjectCommandError> {
    let Some(offer) = offer else {
        return Ok(ResearchPlan::Keep);
    };
    let row = client
        .query_one(
            "SELECT (SELECT lifecycle FROM storyos.tool_registration_revisions
                      WHERE tool_registration_revision = $3::text::uuid),
                    EXISTS (
                      SELECT 1
                        FROM (SELECT * FROM storyos.external_contract_compatibility_decisions
                               WHERE owner_user_id = $1::text::uuid
                                 AND project_id = $2::text::uuid
                                 AND decision_kind = 'tool_registration_compatible'
                               ORDER BY external_compatibility_decision DESC LIMIT 1) AS decision
                        JOIN storyos.project_external_use_binding_revisions AS binding
                          ON (binding.owner_user_id, binding.project_id,
                              binding.project_model_use_binding_revision) =
                             (decision.owner_user_id, decision.project_id,
                              decision.project_model_use_binding_revision)
                        JOIN storyos.processing_destination_identity_evidence_revisions AS evidence
                          ON (evidence.owner_user_id, evidence.project_id,
                              evidence.processing_destination_identity,
                              evidence.evidence_revision) =
                             (binding.owner_user_id, binding.project_id,
                              binding.processing_destination_identity, binding.evidence_revision)
                        JOIN storyos.project_destination_grants AS grant_row
                          ON (grant_row.owner_user_id, grant_row.project_id, grant_row.grant_id) =
                             (binding.owner_user_id, binding.project_id, binding.grant_id)
                       WHERE binding.tool_registration_revision = $3::text::uuid
                         AND binding.credential_reference = $4
                         AND evidence.endpoint = $5
                         AND evidence.account_boundary = $6
                         AND grant_row.grant_scope = $7)",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &offer.registration.revision,
                &offer.credential_reference,
                &offer.registration.server_url,
                &offer.account_boundary,
                &GrantScope::ResearchToolV1.as_str(),
            ],
        )
        .await
        .map_err(unavailable)?;
    let lifecycle = match row.get::<_, Option<&str>>(/*idx*/ 0) {
        None => ToolRegistrationLifecycle::Active,
        Some(lifecycle) => ToolRegistrationLifecycle::parse(lifecycle)
            .ok_or_else(|| unavailable("Tool Registration is damaged"))?,
    };
    let current: bool = row.get(/*idx*/ 1);
    Ok(match lifecycle {
        ToolRegistrationLifecycle::Active if !current => ResearchPlan::Bind(offer),
        ToolRegistrationLifecycle::Active
        | ToolRegistrationLifecycle::Quarantined
        | ToolRegistrationLifecycle::Retired => ResearchPlan::Keep,
    })
}

/// Writes the Registration rows of `offer`, a new Processing Destination Identity with its
/// evidence, the Project Destination Grant `research_tool_v1`, the use binding with the
/// Credential Reference, and the compatibility Decision. Earlier records stay as history.
pub(crate) async fn insert_research_binding(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    offer: &ResearchToolOffer,
) -> Result<(), tokio_postgres::Error> {
    crate::tool_registration::insert_registration_rows(client, offer.registration).await?;
    let record = BindingRecord {
        destination_kind: "mcp_remote",
        control_classification: "external",
        evidence_kind: "mcp_remote_boundary",
        endpoint: Some(offer.registration.server_url),
        account_boundary: Some(offer.account_boundary),
        eligibility_evidence: None,
        grant_kind: "research_tool_use",
        grant_scope: GrantScope::ResearchToolV1,
        registration: BoundRegistration::Tool(offer.registration.revision),
        credential_source: "operator_quota",
        credential_reference: Some(offer.credential_reference),
        budget_bounds: "not_required",
        decision: Some(BindingDecision {
            decision_kind: "tool_registration_compatible",
            runtime_qualification: None,
            qualification_authorization: None,
        }),
    };
    insert_binding(client, &envelope.project_scope, &record).await?;
    Ok(())
}

#[cfg(test)]
#[path = "project_research_binding_tests.rs"]
pub(crate) mod tests;
