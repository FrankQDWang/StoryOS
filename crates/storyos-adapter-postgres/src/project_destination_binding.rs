use crate::statement_cache::Client;
use storyos_application::{ProjectCommandEnvelope, ProjectScope};
use storyos_core::{
    AGENT_PLAN_ACCOUNT_BOUNDARY, AGENT_PLAN_ELIGIBILITY_EVIDENCE, AGENT_PLAN_ENDPOINT,
    DeploymentDestination, DestinationKind, GrantScope,
};
use uuid::Uuid;

/// The kind and evidence columns of the binding records of one destination.
pub(crate) struct BindingShape {
    pub(crate) control_classification: &'static str,
    pub(crate) evidence_kind: &'static str,
    pub(crate) endpoint: Option<&'static str>,
    pub(crate) account_boundary: Option<&'static str>,
    pub(crate) eligibility_evidence: Option<&'static str>,
    grant_kind: &'static str,
    pub(crate) credential_source: &'static str,
    decision_kind: &'static str,
}

pub(crate) fn binding_shape(destination: DestinationKind) -> BindingShape {
    match destination {
        DestinationKind::HostFake => BindingShape {
            control_classification: "host_internal",
            evidence_kind: "host_fake_boundary",
            endpoint: None,
            account_boundary: None,
            eligibility_evidence: None,
            grant_kind: "host_fake_use",
            credential_source: "none",
            decision_kind: "host_fake_compatible",
        },
        DestinationKind::VolcengineAgentPlan => BindingShape {
            control_classification: "external",
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

/// The Registration that a use binding names. Each kind has its own column.
#[derive(Clone, Copy)]
pub(crate) enum BoundRegistration<'a> {
    Model(&'a str),
    Tool(&'a str),
    Embedding(&'a str),
}

/// The compatibility Decision of a new use binding.
pub(crate) struct BindingDecision<'a> {
    pub(crate) decision_kind: &'a str,
    pub(crate) runtime_qualification: Option<&'a str>,
    pub(crate) qualification_authorization: Option<&'a str>,
}

/// The values of the records of one new use binding.
pub(crate) struct BindingRecord<'a> {
    pub(crate) destination_kind: &'a str,
    pub(crate) control_classification: &'a str,
    pub(crate) evidence_kind: &'a str,
    pub(crate) endpoint: Option<&'a str>,
    pub(crate) account_boundary: Option<&'a str>,
    pub(crate) eligibility_evidence: Option<&'a str>,
    pub(crate) grant_kind: &'a str,
    pub(crate) grant_scope: GrantScope,
    pub(crate) registration: BoundRegistration<'a>,
    pub(crate) credential_source: &'a str,
    pub(crate) credential_reference: Option<&'a str>,
    pub(crate) budget_bounds: &'a str,
    pub(crate) decision: Option<BindingDecision<'a>>,
}

/// Writes a new Processing Destination Identity with its evidence, a Project Destination Grant,
/// a `ProjectExternalUseBindingRevision`, and, when `record` has one, the compatibility Decision.
/// Returns the identity of the Decision. Earlier binding records stay unchanged.
pub(crate) async fn insert_binding(
    client: &Client,
    scope: &ProjectScope,
    record: &BindingRecord<'_>,
) -> Result<Option<String>, tokio_postgres::Error> {
    let [identity, grant, binding, decision] = [(); 4].map(|()| Uuid::now_v7().to_string());
    let decision = record.decision.as_ref().map(|_| decision);
    let (model, tool, embedding) = match record.registration {
        BoundRegistration::Model(revision) => (Some(revision), None, None),
        BoundRegistration::Tool(revision) => (None, Some(revision), None),
        BoundRegistration::Embedding(revision) => (None, None, Some(revision)),
    };
    let kind = record.decision.as_ref().map(|made| made.decision_kind);
    let qualification = record
        .decision
        .as_ref()
        .and_then(|made| made.runtime_qualification);
    let authorization = record
        .decision
        .as_ref()
        .and_then(|made| made.qualification_authorization);
    client
        .execute(
            "WITH identity AS (
               INSERT INTO storyos.processing_destination_identities
                 (owner_user_id, project_id, processing_destination_identity, destination_kind,
                  control_classification)
               VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $7, $8)
             ), evidence AS (
               INSERT INTO storyos.processing_destination_identity_evidence_revisions
                 (owner_user_id, project_id, processing_destination_identity, evidence_revision,
                  evidence_kind, endpoint, account_boundary, eligibility_evidence)
               VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 1, $9, $10, $11, $12)
             ), destination_grant AS (
               INSERT INTO storyos.project_destination_grants
                 (owner_user_id, project_id, grant_id, processing_destination_identity,
                  grant_kind, grant_scope)
               VALUES ($1::text::uuid, $2::text::uuid, $4::text::uuid, $3::text::uuid, $13,
                       $14)
             ), binding AS (
               INSERT INTO storyos.project_external_use_binding_revisions
                 (owner_user_id, project_id, project_model_use_binding_revision,
                  processing_destination_identity, evidence_revision, grant_id,
                  model_registration_revision, tool_registration_revision,
                  embedding_registration_revision, credential_source, credential_reference,
                  budget_bounds)
               VALUES ($1::text::uuid, $2::text::uuid, $5::text::uuid, $3::text::uuid, 1,
                       $4::text::uuid, $15::text::uuid, $16::text::uuid, $17::text::uuid, $18,
                       $19, $20)
             )
             INSERT INTO storyos.external_contract_compatibility_decisions
               (owner_user_id, project_id, external_compatibility_decision,
                project_model_use_binding_revision, decision_kind, runtime_qualification,
                qualification_authorization)
             SELECT $1::text::uuid, $2::text::uuid, $6::text::uuid, $5::text::uuid, $21::text,
                    $22::text, $23::text
              WHERE $6::text IS NOT NULL",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &identity,
                &grant,
                &binding,
                &decision,
                &record.destination_kind,
                &record.control_classification,
                &record.evidence_kind,
                &record.endpoint,
                &record.account_boundary,
                &record.eligibility_evidence,
                &record.grant_kind,
                &record.grant_scope.as_str(),
                &model,
                &tool,
                &embedding,
                &record.credential_source,
                &record.credential_reference,
                &record.budget_bounds,
                &kind,
                &qualification,
                &authorization,
            ],
        )
        .await?;
    Ok(decision)
}

/// Writes the Model Registration of `destination` and a new use binding with its compatibility
/// Decision, and returns the Decision.
pub(crate) async fn insert_destination_binding(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    destination: &DeploymentDestination,
) -> Result<String, tokio_postgres::Error> {
    let registration = destination.registration();
    crate::model_registration::insert_model_registration(client, registration).await?;
    let shape = binding_shape(destination.kind());
    let record = BindingRecord {
        destination_kind: destination.kind().as_str(),
        control_classification: shape.control_classification,
        evidence_kind: shape.evidence_kind,
        endpoint: shape.endpoint,
        account_boundary: shape.account_boundary,
        eligibility_evidence: shape.eligibility_evidence,
        grant_kind: shape.grant_kind,
        grant_scope: GrantScope::CURRENT,
        registration: BoundRegistration::Model(registration.revision),
        credential_source: shape.credential_source,
        credential_reference: destination.credential_reference(),
        budget_bounds: destination.budget_bounds().as_str(),
        decision: Some(BindingDecision {
            decision_kind: shape.decision_kind,
            runtime_qualification: Some(destination.runtime_qualification().as_str()),
            qualification_authorization: destination.qualification_authorization(),
        }),
    };
    let decision = insert_binding(client, &envelope.project_scope, &record).await?;
    Ok(decision.expect("a binding with a Decision returns it"))
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
pub(crate) mod grant_scope_tests;
