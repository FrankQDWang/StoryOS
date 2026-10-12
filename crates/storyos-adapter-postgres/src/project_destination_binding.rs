use storyos_application::{ProjectCommandEnvelope, ProjectScope};
use storyos_core::{
    AGENT_PLAN_ACCOUNT_BOUNDARY, AGENT_PLAN_ELIGIBILITY_EVIDENCE, AGENT_PLAN_ENDPOINT,
    DeploymentDestination, GrantScope,
};
use tokio_postgres::Client;
use uuid::Uuid;

/// The kind and evidence columns of the binding records of one deployment destination.
struct BindingShape {
    evidence_kind: &'static str,
    endpoint: Option<&'static str>,
    account_boundary: Option<&'static str>,
    eligibility_evidence: Option<&'static str>,
    grant_kind: &'static str,
    credential_source: &'static str,
    decision_kind: &'static str,
}

fn binding_shape(destination: &DeploymentDestination) -> BindingShape {
    match destination {
        DeploymentDestination::HostFake => BindingShape {
            evidence_kind: "host_fake_boundary",
            endpoint: None,
            account_boundary: None,
            eligibility_evidence: None,
            grant_kind: "host_fake_use",
            credential_source: "none",
            decision_kind: "host_fake_compatible",
        },
        DeploymentDestination::VolcengineAgentPlan { .. } => BindingShape {
            evidence_kind: "volcengine_agent_plan_boundary",
            endpoint: Some(AGENT_PLAN_ENDPOINT),
            account_boundary: Some(AGENT_PLAN_ACCOUNT_BOUNDARY),
            eligibility_evidence: Some(AGENT_PLAN_ELIGIBILITY_EVIDENCE),
            grant_kind: "volcengine_agent_plan_use",
            credential_source: "operator_quota",
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
                  grant_kind, grant_scope)
               VALUES ($1::text::uuid, $2::text::uuid, $4::text::uuid, $3::text::uuid, $12,
                       $20)
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
                project_model_use_binding_revision, decision_kind, runtime_qualification,
                qualification_authorization)
             VALUES ($1::text::uuid, $2::text::uuid, $6::text::uuid, $5::text::uuid, $17, $18,
                     $19)",
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
                &destination.credential_reference(),
                &destination.budget_bounds().as_str(),
                &shape.decision_kind,
                &destination.runtime_qualification().as_str(),
                &destination.qualification_authorization(),
                &GrantScope::CURRENT.as_str(),
            ],
        )
        .await?;
    Ok(decision)
}

/// Whether the current binding of the Project has the records that `destination` writes. A
/// changed destination, Registration, Credential Reference, bound, qualification, or grant scope
/// needs new binding records (ADR 0048, ADR 0049, ADR 0051).
pub(crate) async fn current_binding_offers(
    client: &Client,
    scope: &ProjectScope,
    destination: &DeploymentDestination,
) -> Result<bool, tokio_postgres::Error> {
    let Some(row) = client
        .query_opt(
            "SELECT identity.destination_kind, binding.model_registration_revision::text,
                    binding.credential_reference, binding.budget_bounds,
                    decision.runtime_qualification, decision.qualification_authorization,
                    destination_grant.grant_scope
               FROM storyos.project_policy_revisions AS policy
               JOIN storyos.external_contract_compatibility_decisions AS decision
                 ON (decision.owner_user_id, decision.project_id,
                     decision.external_compatibility_decision) =
                    (policy.owner_user_id, policy.project_id,
                     policy.external_compatibility_decision)
               JOIN storyos.project_external_use_binding_revisions AS binding
                 ON (binding.owner_user_id, binding.project_id,
                     binding.project_model_use_binding_revision) =
                    (decision.owner_user_id, decision.project_id,
                     decision.project_model_use_binding_revision)
               JOIN storyos.project_destination_grants AS destination_grant
                 ON (destination_grant.owner_user_id, destination_grant.project_id,
                     destination_grant.grant_id) =
                    (binding.owner_user_id, binding.project_id, binding.grant_id)
                AND destination_grant.processing_destination_identity =
                    binding.processing_destination_identity
               JOIN storyos.processing_destination_identities AS identity
                 ON (identity.owner_user_id, identity.project_id,
                     identity.processing_destination_identity) =
                    (binding.owner_user_id, binding.project_id,
                     binding.processing_destination_identity)
              WHERE policy.owner_user_id = $1::text::uuid
                AND policy.project_id = $2::text::uuid
              ORDER BY policy.policy_revision DESC
              LIMIT 1",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await?
    else {
        return Ok(false);
    };
    Ok((
        row.get::<_, &str>(/*idx*/ 0),
        row.get::<_, &str>(/*idx*/ 1),
        row.get::<_, Option<&str>>(/*idx*/ 2),
        row.get::<_, &str>(/*idx*/ 3),
        row.get::<_, &str>(/*idx*/ 4),
        row.get::<_, Option<&str>>(/*idx*/ 5),
        row.get::<_, Option<&str>>(/*idx*/ 6),
    ) == (
        destination.kind().as_str(),
        destination.registration().revision,
        destination.credential_reference(),
        destination.budget_bounds().as_str(),
        destination.runtime_qualification().as_str(),
        destination.qualification_authorization(),
        Some(GrantScope::CURRENT.as_str()),
    ))
}

#[cfg(test)]
#[path = "project_destination_binding_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "runtime_qualification_tests.rs"]
mod qualification_tests;

#[cfg(test)]
#[path = "grant_scope_tests.rs"]
mod grant_scope_tests;
