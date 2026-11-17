//! The embedding grant and use binding that `createAgentRun` makes current for the embedding
//! configuration of the deployment (ADR 0095 section 3): a Processing Destination Identity with
//! its evidence, the Project Destination Grant `manuscript_embedding_v1`, and the use binding of
//! the Embedding Registration. The configuration by the operator is the consent.

use storyos_application::{ProjectCommandEnvelope, ProjectCommandError, ProjectScope};
use storyos_core::{
    EmbeddingBindingFacts, EmbeddingRegistration, GrantScope, embedding_binding_to_make,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::command_sequence::unavailable;
use crate::project_destination_binding::binding_shape;

/// The latest embedding use binding of the Project, its grant, and the endpoint of its evidence.
/// Columns: 0 binding revision, 1 Embedding Registration revision, 2 grant, 3 grant scope,
/// 4 destination identity, 5 evidence revision, 6 destination kind, 7 Credential Reference,
/// 8 endpoint.
pub(crate) const CURRENT_EMBEDDING_BINDING: &str = "
    SELECT binding.project_model_use_binding_revision::text,
           binding.embedding_registration_revision::text, grant_row.grant_id::text,
           grant_row.grant_scope, binding.processing_destination_identity::text,
           binding.evidence_revision::text, identity.destination_kind,
           binding.credential_reference, evidence.endpoint
      FROM storyos.project_external_use_binding_revisions AS binding
      JOIN storyos.project_destination_grants AS grant_row
        ON (grant_row.owner_user_id, grant_row.project_id, grant_row.grant_id) =
           (binding.owner_user_id, binding.project_id, binding.grant_id)
      JOIN storyos.processing_destination_identities AS identity
        ON (identity.owner_user_id, identity.project_id,
            identity.processing_destination_identity) =
           (binding.owner_user_id, binding.project_id, binding.processing_destination_identity)
      JOIN storyos.processing_destination_identity_evidence_revisions AS evidence
        ON (evidence.owner_user_id, evidence.project_id, evidence.processing_destination_identity,
            evidence.evidence_revision) =
           (binding.owner_user_id, binding.project_id, binding.processing_destination_identity,
            binding.evidence_revision)
     WHERE binding.owner_user_id = $1::text::uuid AND binding.project_id = $2::text::uuid
       AND binding.embedding_registration_revision IS NOT NULL
     ORDER BY binding.project_model_use_binding_revision DESC
     LIMIT 1";

/// The embedding Registration that `createAgentRun` binds (`embedding_binding_to_make`).
pub(crate) async fn embedding_plan(
    client: &Client,
    scope: &ProjectScope,
    registration: Option<&'static EmbeddingRegistration>,
) -> Result<Option<&'static EmbeddingRegistration>, ProjectCommandError> {
    let Some(registration) = registration else {
        return Ok(None);
    };
    let row = client
        .query_opt(
            CURRENT_EMBEDDING_BINDING,
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(unavailable)?;
    Ok(embedding_binding_to_make(
        Some(registration),
        row.as_ref().map(embedding_binding_facts),
    ))
}

/// The facts of one row of `CURRENT_EMBEDDING_BINDING`.
pub(crate) fn embedding_binding_facts(row: &tokio_postgres::Row) -> EmbeddingBindingFacts<'_> {
    EmbeddingBindingFacts {
        registration_revision: row.get(/*idx*/ 1),
        grant_scope: row.get(/*idx*/ 3),
        destination_kind: row.get(/*idx*/ 6),
        credential_reference: row.get(/*idx*/ 7),
    }
}

/// Writes a new Processing Destination Identity with its evidence, the Project Destination Grant
/// `manuscript_embedding_v1`, and the use binding of `registration`. Earlier records stay as
/// history.
pub(crate) async fn insert_embedding_binding(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    registration: &EmbeddingRegistration,
) -> Result<(), tokio_postgres::Error> {
    let shape = binding_shape(registration.destination);
    let [identity, grant, binding] = [(); 3].map(|()| Uuid::now_v7().to_string());
    client
        .execute(
            "WITH identity AS (
               INSERT INTO storyos.processing_destination_identities
                 (owner_user_id, project_id, processing_destination_identity, destination_kind,
                  control_classification)
               VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $6, $7)
             ), evidence AS (
               INSERT INTO storyos.processing_destination_identity_evidence_revisions
                 (owner_user_id, project_id, processing_destination_identity, evidence_revision,
                  evidence_kind, endpoint, account_boundary, eligibility_evidence)
               VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 1, $8, $9, $10, $11)
             ), destination_grant AS (
               INSERT INTO storyos.project_destination_grants
                 (owner_user_id, project_id, grant_id, processing_destination_identity,
                  grant_kind, grant_scope)
               VALUES ($1::text::uuid, $2::text::uuid, $4::text::uuid, $3::text::uuid,
                       'manuscript_embedding_use', $12)
             )
             INSERT INTO storyos.project_external_use_binding_revisions
               (owner_user_id, project_id, project_model_use_binding_revision,
                processing_destination_identity, evidence_revision, grant_id,
                embedding_registration_revision, credential_source, credential_reference,
                budget_bounds)
             VALUES ($1::text::uuid, $2::text::uuid, $5::text::uuid, $3::text::uuid, 1,
                     $4::text::uuid, $13::text::uuid, $14, $15, $16)",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &identity,
                &grant,
                &binding,
                &registration.destination.as_str(),
                &shape.control_classification,
                &shape.evidence_kind,
                &shape.endpoint,
                &shape.account_boundary,
                &shape.eligibility_evidence,
                &GrantScope::ManuscriptEmbeddingV1.as_str(),
                &registration.revision,
                &shape.credential_source,
                &registration.destination.credential_reference(),
                &registration.request_bounds,
            ],
        )
        .await?;
    // Each live Chapter gets index work for the new binding (ADR 0095 section 4).
    crate::passage_index_work::mark_passage_index_work(
        client,
        &envelope.project_scope,
        crate::passage_index_work::MarkedChapters::Live,
    )
    .await
}

#[cfg(test)]
#[path = "project_embedding_binding_tests.rs"]
pub(crate) mod tests;
