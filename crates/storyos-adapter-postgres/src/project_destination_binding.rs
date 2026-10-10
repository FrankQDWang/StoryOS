use storyos_application::ProjectCommandEnvelope;
use storyos_core::{
    AGENT_PLAN_ACCOUNT_BOUNDARY, AGENT_PLAN_ELIGIBILITY_EVIDENCE, AGENT_PLAN_ENDPOINT,
    CREATE_REQUIREMENT, DeploymentDestination, RuntimeQualification,
};
use tokio_postgres::Client;
use uuid::Uuid;

/// The kind and evidence columns of the binding records of one deployment destination.
struct BindingShape<'a> {
    evidence_kind: &'static str,
    endpoint: Option<&'static str>,
    account_boundary: Option<&'static str>,
    eligibility_evidence: Option<&'static str>,
    grant_kind: &'static str,
    credential_source: &'static str,
    credential_reference: Option<&'a str>,
    budget_bounds: &'static str,
    decision_kind: &'static str,
}

fn binding_shape(destination: &DeploymentDestination) -> BindingShape<'_> {
    match destination {
        DeploymentDestination::HostFake => BindingShape {
            evidence_kind: "host_fake_boundary",
            endpoint: None,
            account_boundary: None,
            eligibility_evidence: None,
            grant_kind: "host_fake_use",
            credential_source: "none",
            credential_reference: None,
            budget_bounds: "not_required",
            decision_kind: "host_fake_compatible",
        },
        DeploymentDestination::VolcengineAgentPlan {
            credential_reference,
        } => BindingShape {
            evidence_kind: "volcengine_agent_plan_boundary",
            endpoint: Some(AGENT_PLAN_ENDPOINT),
            account_boundary: Some(AGENT_PLAN_ACCOUNT_BOUNDARY),
            eligibility_evidence: Some(AGENT_PLAN_ELIGIBILITY_EVIDENCE),
            grant_kind: "volcengine_agent_plan_use",
            credential_source: "operator_quota",
            credential_reference: Some(credential_reference),
            budget_bounds: "unqualified",
            decision_kind: "volcengine_agent_plan_compatible",
        },
    }
}

/// Writes a new Processing Destination Identity with its evidence, Project Destination Grant,
/// `ProjectExternalUseBindingRevision`, and compatibility Decision for `destination`, and
/// returns the Decision. Earlier binding records stay unchanged.
pub(crate) async fn insert_destination_binding(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    destination: &DeploymentDestination,
) -> Result<String, tokio_postgres::Error> {
    let registration = destination.registration();
    crate::model_registration::insert_model_registration(client, registration).await?;
    let shape = binding_shape(destination);
    let qualification = if registration
        .capability_profile
        .qualifies(CREATE_REQUIREMENT)
    {
        RuntimeQualification::Qualified
    } else {
        RuntimeQualification::Pending
    };
    let [identity, grant, binding, decision] = [(); 4].map(|()| Uuid::now_v7().to_string());
    client
        .execute(
            "WITH identity AS (
               INSERT INTO storyos.processing_destination_identities
                 (owner_user_id, project_id, processing_destination_identity, destination_kind)
               VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $7)
             ), evidence AS (
               INSERT INTO storyos.processing_destination_identity_evidence_revisions
                 (owner_user_id, project_id, processing_destination_identity, evidence_revision,
                  evidence_kind, endpoint, account_boundary, eligibility_evidence)
               VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 1, $8, $9, $10, $11)
             ), destination_grant AS (
               INSERT INTO storyos.project_destination_grants
                 (owner_user_id, project_id, grant_id, processing_destination_identity,
                  grant_kind)
               VALUES ($1::text::uuid, $2::text::uuid, $4::text::uuid, $3::text::uuid, $12)
             ), binding AS (
               INSERT INTO storyos.project_external_use_binding_revisions
                 (owner_user_id, project_id, project_model_use_binding_revision,
                  processing_destination_identity, evidence_revision, grant_id,
                  model_registration_revision, credential_source, credential_reference,
                  budget_bounds)
               VALUES ($1::text::uuid, $2::text::uuid, $5::text::uuid, $3::text::uuid, 1,
                       $4::text::uuid, $13::text::uuid, $14, $15, $16)
             )
             INSERT INTO storyos.external_contract_compatibility_decisions
               (owner_user_id, project_id, external_compatibility_decision,
                project_model_use_binding_revision, decision_kind, runtime_qualification)
             VALUES ($1::text::uuid, $2::text::uuid, $6::text::uuid, $5::text::uuid, $17, $18)",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &identity,
                &grant,
                &binding,
                &decision,
                &destination.kind().as_str(),
                &shape.evidence_kind,
                &shape.endpoint,
                &shape.account_boundary,
                &shape.eligibility_evidence,
                &shape.grant_kind,
                &registration.revision,
                &shape.credential_source,
                &shape.credential_reference,
                &shape.budget_bounds,
                &shape.decision_kind,
                &qualification.as_str(),
            ],
        )
        .await?;
    Ok(decision)
}

#[cfg(test)]
#[path = "project_destination_binding_tests.rs"]
mod tests;
