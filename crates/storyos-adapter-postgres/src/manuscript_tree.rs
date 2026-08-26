use uuid::Uuid;

use storyos_application::{
    CanonicalSnapshot, CanonicalTreeFacts, ManuscriptTreeReader, ProjectReadError, ProjectScope,
};

use super::{PostgresProjectReader, read_error, set_scope};

impl ManuscriptTreeReader for PostgresProjectReader {
    async fn read_canonical_tree_facts(
        &self,
        scope: &ProjectScope,
    ) -> Result<Option<CanonicalTreeFacts>, ProjectReadError> {
        let mut client = self.connect().await?;
        let transaction = client.transaction().await.map_err(read_error)?;
        set_scope(&transaction, scope).await?;
        let Some(project) = transaction
            .query_opt(
                "SELECT tree_revision::text,
                        COALESCE(counters.project_activity_position, 0)::text
                   FROM storyos.projects AS project
                   LEFT JOIN storyos.scope_counters AS counters
                     ON (counters.owner_user_id, counters.project_id) =
                        (project.owner_user_id, project.project_id)
                  WHERE project.owner_user_id = $1::text::uuid
                    AND project.project_id = $2::text::uuid",
                &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
            )
            .await
            .map_err(read_error)?
        else {
            transaction.commit().await.map_err(read_error)?;
            return Ok(None);
        };
        let tree_revision = project
            .get::<_, String>(0)
            .parse()
            .map_err(ProjectReadError::unavailable)?;
        let activity_position = project
            .get::<_, String>(1)
            .parse()
            .map_err(ProjectReadError::unavailable)?;
        let snapshot = match load_latest_canonical_snapshot(&transaction, scope).await? {
            Some(snapshot) => snapshot,
            None => issue_canonical_snapshot(&transaction, scope, activity_position).await?,
        };
        transaction.commit().await.map_err(read_error)?;
        Ok(Some(CanonicalTreeFacts {
            project_scope: scope.clone(),
            snapshot,
            tree_revision,
            volumes: Vec::new(),
        }))
    }
}

async fn load_latest_canonical_snapshot(
    transaction: &tokio_postgres::Transaction<'_>,
    scope: &ProjectScope,
) -> Result<Option<CanonicalSnapshot>, ProjectReadError> {
    let row = transaction
        .query_opt(
            "SELECT snapshot.snapshot_id::text,
                    snapshot.project_activity_position::text,
                    snapshot.replay_generation::text,
                    snapshot.redaction_profile,
                    snapshot.schema_profile,
                    to_char(snapshot.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                    CASE WHEN snapshot.expires_at IS NULL THEN NULL
                         ELSE to_char(snapshot.expires_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
                    END,
                    current_floor.floor_position::text
               FROM storyos.project_snapshots AS snapshot
               JOIN LATERAL (
                 SELECT replay_generation FROM storyos.replay_generations
                  WHERE owner_user_id = snapshot.owner_user_id AND project_id = snapshot.project_id
                  ORDER BY replay_generation DESC LIMIT 1
               ) AS current_generation ON true
               JOIN storyos.replay_floors AS current_floor
                 ON (current_floor.owner_user_id, current_floor.project_id,
                     current_floor.replay_generation) =
                    (snapshot.owner_user_id, snapshot.project_id, current_generation.replay_generation)
              WHERE snapshot.owner_user_id = $1::text::uuid
                AND snapshot.project_id = $2::text::uuid
                AND snapshot.snapshot_kind = 'canonical'
                AND (snapshot.expires_at IS NULL OR snapshot.expires_at > clock_timestamp())
              ORDER BY snapshot.project_activity_position DESC, snapshot.created_at DESC
              LIMIT 1",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(read_error)?;
    row.map(snapshot_from_row).transpose()
}

async fn issue_canonical_snapshot(
    transaction: &tokio_postgres::Transaction<'_>,
    scope: &ProjectScope,
    project_activity_position: u64,
) -> Result<CanonicalSnapshot, ProjectReadError> {
    let snapshot_id = Uuid::now_v7().to_string();
    transaction
        .execute(
            "INSERT INTO storyos.replay_generations
               (owner_user_id, project_id, replay_generation)
             VALUES ($1::text::uuid, $2::text::uuid, 1)
             ON CONFLICT DO NOTHING",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(read_error)?;
    transaction
        .execute(
            "INSERT INTO storyos.replay_floors
               (owner_user_id, project_id, replay_generation, floor_position)
             VALUES ($1::text::uuid, $2::text::uuid, 1, 0)
             ON CONFLICT DO NOTHING",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(read_error)?;
    let row = transaction
        .query_one(
            "INSERT INTO storyos.project_snapshots
               (owner_user_id, project_id, snapshot_id, replay_generation,
                project_activity_position, snapshot_kind, redaction_profile, schema_profile)
             SELECT $1::text::uuid, $2::text::uuid, $3::text::uuid, generation.replay_generation,
                    $4::text::numeric, 'canonical', 'storyos.author.v1', 'storyos.public.release.1'
               FROM storyos.replay_generations AS generation
              WHERE generation.owner_user_id = $1::text::uuid
                AND generation.project_id = $2::text::uuid
              ORDER BY generation.replay_generation DESC
              LIMIT 1
             RETURNING snapshot_id::text,
                       project_activity_position::text,
                       replay_generation::text,
                       redaction_profile,
                       schema_profile,
                       to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                       CASE WHEN expires_at IS NULL THEN NULL
                            ELSE to_char(expires_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
                       END",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &snapshot_id,
                &project_activity_position.to_string(),
            ],
        )
        .await
        .map_err(read_error)?;
    Ok(CanonicalSnapshot {
        snapshot_id: row.get(0),
        project_activity_position: row
            .get::<_, String>(1)
            .parse()
            .map_err(ProjectReadError::unavailable)?,
        replay_generation: row
            .get::<_, String>(2)
            .parse()
            .map_err(ProjectReadError::unavailable)?,
        floor_position: 0,
        redaction_profile: row.get(3),
        schema_profile: row.get(4),
        created_at: row.get(5),
        expires_at: row.get(6),
    })
}

fn snapshot_from_row(row: tokio_postgres::Row) -> Result<CanonicalSnapshot, ProjectReadError> {
    Ok(CanonicalSnapshot {
        snapshot_id: row.get(0),
        project_activity_position: row
            .get::<_, String>(1)
            .parse()
            .map_err(ProjectReadError::unavailable)?,
        replay_generation: row
            .get::<_, String>(2)
            .parse()
            .map_err(ProjectReadError::unavailable)?,
        floor_position: row
            .get::<_, String>(7)
            .parse()
            .map_err(ProjectReadError::unavailable)?,
        redaction_profile: row.get(3),
        schema_profile: row.get(4),
        created_at: row.get(5),
        expires_at: row.get(6),
    })
}

#[cfg(test)]
#[path = "manuscript_tree_tests.rs"]
mod tests;
