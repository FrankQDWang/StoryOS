//! The research records that `createAgentRun` makes current for the research destination of the
//! deployment (ADR 0083, ADR 0090): the Processing Destination Identity of the server, the Project
//! Destination Grant `research_tool_v1`, the use binding with the Credential Reference, and the
//! compatibility Decision from the pinned active Registration revision. The Server performs no MCP
//! exchange; the Worker checks the pinned digests before each dispatch. The current research
//! Decision of a Project is its latest `tool_registration_compatible` Decision.

use storyos_application::{ProjectCommandEnvelope, ProjectCommandError, ProjectScope};
use storyos_core::{GrantScope, ResearchToolOffer, ToolRegistrationLifecycle};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::command_sequence::unavailable;

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
    let [identity, grant, binding, decision] = [(); 4].map(|()| Uuid::now_v7().to_string());
    client
        .execute(
            "WITH identity AS (
               INSERT INTO storyos.processing_destination_identities
                 (owner_user_id, project_id, processing_destination_identity, destination_kind,
                  control_classification)
               VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 'mcp_remote', 'external')
             ), evidence AS (
               INSERT INTO storyos.processing_destination_identity_evidence_revisions
                 (owner_user_id, project_id, processing_destination_identity, evidence_revision,
                  evidence_kind, endpoint, account_boundary)
               VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 1, 'mcp_remote_boundary',
                       $7, $8)
             ), destination_grant AS (
               INSERT INTO storyos.project_destination_grants
                 (owner_user_id, project_id, grant_id, processing_destination_identity,
                  grant_kind, grant_scope)
               VALUES ($1::text::uuid, $2::text::uuid, $4::text::uuid, $3::text::uuid,
                       'research_tool_use', $11)
             ), binding AS (
               INSERT INTO storyos.project_external_use_binding_revisions
                 (owner_user_id, project_id, project_model_use_binding_revision,
                  processing_destination_identity, evidence_revision, grant_id,
                  tool_registration_revision, credential_source, credential_reference,
                  budget_bounds)
               VALUES ($1::text::uuid, $2::text::uuid, $5::text::uuid, $3::text::uuid, 1,
                       $4::text::uuid, $9::text::uuid, 'operator_quota', $10, 'not_required')
             )
             INSERT INTO storyos.external_contract_compatibility_decisions
               (owner_user_id, project_id, external_compatibility_decision,
                project_model_use_binding_revision, decision_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $6::text::uuid, $5::text::uuid,
                     'tool_registration_compatible')",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &identity,
                &grant,
                &binding,
                &decision,
                &offer.registration.server_url,
                &offer.account_boundary,
                &offer.registration.revision,
                &offer.credential_reference,
                &GrantScope::ResearchToolV1.as_str(),
            ],
        )
        .await?;
    Ok(())
}

#[cfg(test)]
#[path = "project_research_binding_tests.rs"]
mod tests;
