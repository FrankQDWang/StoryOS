use storyos_application::{
    ClaimedAgentRun, CompleteAgentRunError, CredentialReference, RequestRoute,
};
use storyos_core::{ModelAdapter, ModelRegistration};
use tokio_postgres::Client;

use crate::agent_run_work::complete_database_error;

/// The adapter of the Registration that the claimed AgentRun pinned, and the Credential
/// Reference of its use binding.
pub(crate) async fn request_route(
    client: &Client,
    claim: &ClaimedAgentRun,
) -> Result<RequestRoute, CompleteAgentRunError> {
    let row = client
        .query_one(
            "SELECT registration.model_kind, binding.credential_reference
               FROM storyos.agent_runs AS run
               JOIN storyos.model_registration_revisions AS registration
                 ON registration.model_registration_revision = run.model_registration_revision
               JOIN storyos.project_external_use_binding_revisions AS binding
                 ON (binding.owner_user_id, binding.project_id,
                     binding.project_model_use_binding_revision) =
                    (run.owner_user_id, run.project_id, run.project_model_use_binding_revision)
              WHERE run.owner_user_id = $1::text::uuid
                AND run.project_id = $2::text::uuid
                AND run.run_id = $3::text::uuid",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    Ok(RequestRoute {
        adapter: ModelAdapter::parse(row.get(/*idx*/ 0)).ok_or_else(|| {
            CompleteAgentRunError::Unavailable(Box::new(std::io::Error::other(
                "The Model Registration adapter is damaged",
            )))
        })?,
        credential_reference: row
            .get::<_, Option<String>>(/*idx*/ 1)
            .map(CredentialReference),
    })
}

/// Inserts one global Model Registration with its Capability Profile and head, if absent.
pub(crate) async fn insert_model_registration(
    client: &Client,
    registration: &ModelRegistration,
) -> Result<(), tokio_postgres::Error> {
    let profile = registration.capability_profile;
    let profile_json = serde_json::to_value(profile).expect("a static profile serializes");
    client
        .execute(
            "INSERT INTO storyos.model_capability_profiles (capability_profile_revision, profile)
             VALUES ($1, $2::text::jsonb)
             ON CONFLICT (capability_profile_revision) DO NOTHING",
            &[&profile.revision, &profile_json.to_string()],
        )
        .await?;
    client
        .execute(
            "INSERT INTO storyos.model_registration_revisions
               (model_registration_revision, model_kind, api_surface, provider_model_id,
                capability_profile_revision)
             VALUES ($1::text::uuid, $2, $3, $4, $5)
             ON CONFLICT (model_registration_revision) DO NOTHING",
            &[
                &registration.revision,
                &registration.adapter.kind(),
                &registration.api_surface,
                &registration.provider_model_id,
                &profile.revision,
            ],
        )
        .await?;
    client
        .execute(
            "INSERT INTO storyos.model_registration_heads (model_kind, model_registration_revision)
             VALUES ($1, $2::text::uuid)
             ON CONFLICT (model_kind) DO NOTHING",
            &[&registration.adapter.kind(), &registration.revision],
        )
        .await?;
    Ok(())
}

#[cfg(test)]
#[path = "model_registration_tests.rs"]
pub(crate) mod tests;
