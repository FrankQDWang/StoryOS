//! The Context Assembly gates of ADR 0005 for the Tool destination. The dispatch claim commits
//! them in its transaction before the Destination Attempt. Gate one: the Operation Requirement is
//! the ToolCall. Gate two: the candidate is the validated arguments as data. Gate three: the
//! eligibility is the Project Scope and the grant scope `research_tool_v1`. Gate four: the one
//! candidate is the selection, with no ranking. Gate five: the Bounded Projection is the typed
//! intake. Gate six: the Context Assembly Manifest, the Destination Context Manifest, and the
//! Outbound Disclosure Manifest commit with the preparation step, before the dispatch claim.
//! Gate seven is the claim itself, in a later transaction.

use serde_json::{Value, json};
use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{GrantScope, ToolCallPlan};
use uuid::Uuid;

use crate::agent_run_tool_authority::{ToolCallRow, damaged};
use crate::agent_run_work::complete_database_error;

/// The manifest identities that the Destination Attempt of the ToolCall refers to.
pub(crate) struct ToolContextManifests {
    pub assembly: String,
    pub destination: String,
    pub outbound: String,
}

impl ToolContextManifests {
    pub fn record(&self) -> Value {
        json!({
            "context_assembly_manifest_id": self.assembly,
            "destination_context_manifest_id": self.destination,
            "outbound_disclosure_manifest_id": self.outbound,
        })
    }
}

pub(crate) struct ToolContext<'a> {
    pub row: &'a ToolCallRow,
    pub plan: &'a ToolCallPlan,
    pub endpoint: &'a str,
    /// The exact authority evidence of the admission.
    pub authority: &'a Value,
}

/// Commits gates one to six for the ToolCall and returns the Context Assembly Manifest identity.
/// A missing Run requirement is a damaged Run.
pub(crate) async fn commit_tool_context(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    context: ToolContext<'_>,
) -> Result<String, CompleteAgentRunError> {
    let ToolContext {
        row,
        plan,
        endpoint,
        authority,
    } = context;
    let requirement_id = Uuid::now_v7().to_string();
    let manifests = ToolContextManifests {
        assembly: Uuid::now_v7().to_string(),
        destination: Uuid::now_v7().to_string(),
        outbound: Uuid::now_v7().to_string(),
    };
    let requirement = json!({
        "kind": "tool_call",
        "tool_call_id": row.tool_call_id,
        "call_id": row.call_id,
        "registration_revision": row.registration_revision,
        "spec_revision": row.spec_revision,
    });
    let manifest = json!({
        "candidate": {"arguments": row.arguments},
        "eligibility": {
            "project_scope": {
                "owner_user_id": claim.project_scope.owner_user_id.as_ref(),
                "project_id": claim.project_scope.project_id.as_ref(),
            },
            "grant_scope": GrantScope::ResearchToolV1.as_str(),
        },
        "selection": "one_candidate",
        "bounded_projection": {"intake": "typed_arguments", "effect_request": plan.effect.record()},
        "destination_context": {
            "destination": "tool",
            "endpoint": endpoint,
            "registration_revision": row.registration_revision,
            "spec_revision": row.spec_revision,
            "authority": authority,
        },
        "outbound_disclosure": {
            "source_class": plan.effect.outbound_disclosure.source_class(),
            "disclosed": plan.effect.disclosed,
        },
    });
    let inserted = client
        .execute(
            "WITH position AS (
               SELECT count(*)::numeric AS next FROM storyos.operation_requirements
                WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                  AND run_id = $3::text::uuid AND requirement_role = 'tool_call'),
             requirement AS (
               INSERT INTO storyos.operation_requirements
                 (owner_user_id, project_id, operation_requirement_id, run_id,
                  input_snapshot_id, receipt_id, payload, requirement_role, decision_position)
               SELECT primary_requirement.owner_user_id, primary_requirement.project_id,
                      $4::text::uuid, primary_requirement.run_id,
                      primary_requirement.input_snapshot_id, primary_requirement.receipt_id,
                      $5::text::jsonb, 'tool_call', position.next
                 FROM storyos.operation_requirements AS primary_requirement, position
                WHERE primary_requirement.owner_user_id = $1::text::uuid
                  AND primary_requirement.project_id = $2::text::uuid
                  AND primary_requirement.run_id = $3::text::uuid
                  AND primary_requirement.requirement_role = 'primary'
                  AND primary_requirement.decision_position = 0
               RETURNING owner_user_id, project_id, operation_requirement_id, run_id,
                         receipt_id, decision_position)
             INSERT INTO storyos.context_assembly_manifests
               (owner_user_id, project_id, context_assembly_manifest_id, operation_requirement_id,
                run_id, sufficiency, destination_context_manifest_id,
                outbound_disclosure_manifest_id, payload, receipt_id, manifest_role,
                decision_position, disclosure_grant_id, disclosure_grant_scope)
             SELECT owner_user_id, project_id, $6::text::uuid, operation_requirement_id, run_id,
                    'complete', $7::text::uuid, $8::text::uuid, $9::text::jsonb, receipt_id,
                    'tool_call', decision_position, $10::text::uuid, $11
               FROM requirement",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &requirement_id,
                &requirement.to_string(),
                &manifests.assembly,
                &manifests.destination,
                &manifests.outbound,
                &manifest.to_string(),
                &authority["grant_id"].as_str(),
                &GrantScope::ResearchToolV1.as_str(),
            ],
        )
        .await
        .map_err(complete_database_error)?;
    if inserted != 1 {
        return Err(damaged(
            "The primary Operation Requirement of the Run is missing",
        ));
    }
    Ok(manifests.assembly)
}

/// The committed manifests of `manifest_id` when they belong to the ToolCall and record the same
/// authority evidence, and `None` otherwise.
pub(crate) async fn committed_tool_context(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    manifest_id: &str,
    authority: &Value,
) -> Result<Option<ToolContextManifests>, CompleteAgentRunError> {
    let found = client
        .query_opt(
            "SELECT manifest.destination_context_manifest_id::text,
                    manifest.outbound_disclosure_manifest_id::text
               FROM storyos.context_assembly_manifests AS manifest
               JOIN storyos.operation_requirements AS requirement
                 USING (owner_user_id, project_id, operation_requirement_id)
              WHERE manifest.owner_user_id = $1::text::uuid AND manifest.project_id = $2::text::uuid
                AND manifest.run_id = $3::text::uuid
                AND manifest.context_assembly_manifest_id = $4::text::uuid
                AND manifest.manifest_role = 'tool_call' AND manifest.sufficiency = 'complete'
                AND requirement.payload ->> 'tool_call_id' = $5
                AND manifest.payload #> '{destination_context,authority}' = $6::text::jsonb",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &manifest_id,
                &row.tool_call_id,
                &authority.to_string(),
            ],
        )
        .await
        .map_err(complete_database_error)?;
    Ok(found.map(|found| ToolContextManifests {
        assembly: manifest_id.to_owned(),
        destination: found.get(0),
        outbound: found.get(1),
    }))
}
