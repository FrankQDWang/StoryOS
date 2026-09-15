use storyos_application::ProjectScope;

const VOLUME_KEY_BATCH_SIZE: usize = 1_000_000;

pub(crate) async fn persist_volume_storage_order(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    ordered_ids: &[String],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut original_keys = Vec::with_capacity(ordered_ids.len());
    for ids in ordered_ids.chunks(VOLUME_KEY_BATCH_SIZE) {
        let volume_ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        let moved = client
            .query(
                "WITH prior AS MATERIALIZED (
                   SELECT volume.manuscript_object_id, volume.tree_order,
                          row_number() OVER (ORDER BY volume.tree_order) AS ordinal
                     FROM storyos.manuscript_objects AS volume
                    WHERE volume.owner_user_id = $1::text::uuid
                      AND volume.project_id = $2::text::uuid AND volume.object_kind = 'volume'
                      AND volume.manuscript_object_id = ANY($3::text[]::uuid[])
                      AND NOT EXISTS (
                        SELECT 1 FROM storyos.volume_removal_decisions AS removal
                         WHERE removal.owner_user_id = volume.owner_user_id
                           AND removal.project_id = volume.project_id
                           AND removal.volume_id = volume.manuscript_object_id
                      )
                 ), ceiling AS (
                   SELECT max(tree_order) AS last_key FROM storyos.manuscript_objects
                    WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                      AND object_kind = 'volume'
                 )
                 UPDATE storyos.manuscript_objects AS volume
                    SET tree_order = ceiling.last_key + prior.ordinal
                   FROM prior, ceiling
                  WHERE volume.owner_user_id = $1::text::uuid
                    AND volume.project_id = $2::text::uuid AND volume.object_kind = 'volume'
                    AND volume.manuscript_object_id = prior.manuscript_object_id
                 RETURNING prior.tree_order",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &volume_ids,
                ],
            )
            .await?;
        if moved.len() != ids.len() {
            return Err(std::io::Error::other("live Volume set changed under FOR UPDATE").into());
        }
        original_keys.extend(moved.iter().map(|row| row.get::<_, i64>(0)));
    }
    // Keep removed slots in place so later Delete Volume Compensation restores their position.
    original_keys.sort_unstable();
    for (ids, keys) in ordered_ids
        .chunks(VOLUME_KEY_BATCH_SIZE)
        .zip(original_keys.chunks(VOLUME_KEY_BATCH_SIZE))
    {
        let volume_ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        let updated = client
            .execute(
                "UPDATE storyos.manuscript_objects AS volume
                    SET tree_order = ranked.storage_key
                   FROM unnest($3::text[], $4::bigint[]) AS ranked(volume_id, storage_key)
                  WHERE volume.owner_user_id = $1::text::uuid AND volume.project_id = $2::text::uuid
                    AND volume.object_kind = 'volume'
                    AND volume.manuscript_object_id = ranked.volume_id::uuid",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &volume_ids,
                    &keys,
                ],
            )
            .await?;
        if updated != ids.len() as u64 {
            return Err(std::io::Error::other("live Volume set changed under FOR UPDATE").into());
        }
    }
    Ok(())
}
