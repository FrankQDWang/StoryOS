use storyos_core::ModelRegistration;
use tokio_postgres::Client;

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
