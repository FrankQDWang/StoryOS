use storyos_application::{
    AppliedAuthorEditActivity, CanonicalSnapshot, ProjectScope, SnapshotLookup, SnapshotReadError,
    SnapshotStore,
};

use super::*;

pub(super) async fn persist_canonical_snapshot(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    snapshot_id: &str,
    project_activity_position: u64,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "INSERT INTO storyos.replay_generations
               (owner_user_id, project_id, replay_generation)
             VALUES ($1::text::uuid, $2::text::uuid, 1)
             ON CONFLICT DO NOTHING",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await?;
    client
        .execute(
            "INSERT INTO storyos.replay_floors
               (owner_user_id, project_id, replay_generation, floor_position)
             VALUES ($1::text::uuid, $2::text::uuid, 1, 0)
             ON CONFLICT DO NOTHING",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await?;
    client
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
             RETURNING snapshot_id",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &snapshot_id,
                &project_activity_position.to_string(),
            ],
        )
        .await?;
    Ok(())
}

impl SnapshotStore for PostgresProjectReader {
    async fn read_snapshot(
        &self,
        lookup: &SnapshotLookup,
    ) -> Result<Option<CanonicalSnapshot>, SnapshotReadError> {
        load_snapshot(self, lookup, SnapshotEligibility::CanonicalQuery).await
    }

    async fn activity_resume(
        &self,
        lookup: &SnapshotLookup,
    ) -> Result<CanonicalSnapshot, SnapshotReadError> {
        load_snapshot(self, lookup, SnapshotEligibility::ActivityResume)
            .await?
            .ok_or_else(|| {
                SnapshotReadError::Unavailable("canonical snapshot is unavailable".into())
            })
    }

    async fn list_applied_author_edit_activity(
        &self,
        lookup: &SnapshotLookup,
        after_position: u64,
    ) -> Result<Vec<AppliedAuthorEditActivity>, SnapshotReadError> {
        list_applied_author_edit_activity(self, lookup, after_position).await
    }
}

enum SnapshotEligibility {
    CanonicalQuery,
    ActivityResume,
}

async fn load_snapshot(
    store: &PostgresProjectReader,
    lookup: &SnapshotLookup,
    eligibility: SnapshotEligibility,
) -> Result<Option<CanonicalSnapshot>, SnapshotReadError> {
    let mut client = store
        .connect()
        .await
        .map_err(|error| SnapshotReadError::Unavailable(Box::new(error)))?;
    let transaction = client.transaction().await.map_err(snapshot_unavailable)?;
    set_scope(&transaction, &lookup.project_scope)
        .await
        .map_err(|error| SnapshotReadError::Unavailable(Box::new(error)))?;
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
                    snapshot.expires_at IS NOT NULL AND snapshot.expires_at <= clock_timestamp(),
                    snapshot.replay_generation < current_generation.replay_generation
                      OR snapshot.project_activity_position < current_floor.floor_position,
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
              AND snapshot.snapshot_id = $3::text::uuid",
            &[
                &lookup.project_scope.owner_user_id.as_ref(),
                &lookup.project_scope.project_id.as_ref(),
                &lookup.snapshot_id,
            ],
        )
        .await
        .map_err(snapshot_unavailable)?;
    transaction.commit().await.map_err(snapshot_unavailable)?;
    let Some(row) = row else {
        return Ok(None);
    };
    if row.get::<_, bool>(7) {
        return Err(SnapshotReadError::Expired);
    }
    if matches!(eligibility, SnapshotEligibility::ActivityResume) && row.get::<_, bool>(8) {
        return Err(SnapshotReadError::CursorTooOld);
    }
    Ok(Some(CanonicalSnapshot {
        snapshot_id: row.get(0),
        project_activity_position: row
            .get::<_, String>(1)
            .parse()
            .map_err(|error| SnapshotReadError::Unavailable(Box::new(error)))?,
        replay_generation: row
            .get::<_, String>(2)
            .parse()
            .map_err(|error| SnapshotReadError::Unavailable(Box::new(error)))?,
        floor_position: row
            .get::<_, String>(9)
            .parse()
            .map_err(|error| SnapshotReadError::Unavailable(Box::new(error)))?,
        redaction_profile: row.get(3),
        schema_profile: row.get(4),
        created_at: row.get(5),
        expires_at: row.get(6),
    }))
}

fn snapshot_unavailable(source: tokio_postgres::Error) -> SnapshotReadError {
    SnapshotReadError::Unavailable(Box::new(source))
}

async fn list_applied_author_edit_activity(
    store: &PostgresProjectReader,
    lookup: &SnapshotLookup,
    after_position: u64,
) -> Result<Vec<AppliedAuthorEditActivity>, SnapshotReadError> {
    let mut client = store
        .connect()
        .await
        .map_err(|error| SnapshotReadError::Unavailable(Box::new(error)))?;
    let transaction = client.transaction().await.map_err(snapshot_unavailable)?;
    set_scope(&transaction, &lookup.project_scope)
        .await
        .map_err(|error| SnapshotReadError::Unavailable(Box::new(error)))?;
    let after = after_position.to_string();
    let rows = transaction
        .query(
            "SELECT activity.project_activity_event_id::text,
                    activity.project_activity_position::text,
                    receipt.command_id::text,
                    admission.correlation_id::text,
                    activity.receipt_id::text,
                    admission.chapter_object_id::text,
                    activity.resulting_revision_id::text,
                    activity.authoritative_commit_id::text,
                    activity.author_action_sequence::text,
                    to_char(activity.created_at AT TIME ZONE 'UTC',
                            'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
               FROM storyos.project_activity_events AS activity
               JOIN storyos.domain_receipts AS receipt
                 ON (receipt.owner_user_id, receipt.project_id, receipt.receipt_id) =
                    (activity.owner_user_id, activity.project_id, activity.receipt_id)
               JOIN storyos.author_command_admissions AS admission
                 ON (admission.owner_user_id, admission.project_id, admission.command_id) =
                    (receipt.owner_user_id, receipt.project_id, receipt.command_id)
              WHERE activity.owner_user_id = $1::text::uuid
                AND activity.project_id = $2::text::uuid
                AND activity.project_activity_position > $3::text::numeric
                AND activity.event_kind = 'authoritative_author_edit_applied'
              ORDER BY activity.project_activity_position",
            &[
                &lookup.project_scope.owner_user_id.as_ref(),
                &lookup.project_scope.project_id.as_ref(),
                &after,
            ],
        )
        .await
        .map_err(snapshot_unavailable)?;
    transaction.commit().await.map_err(snapshot_unavailable)?;
    rows.into_iter()
        .map(|row| {
            let project_sequence = row
                .get::<_, String>(1)
                .parse()
                .map_err(|error| SnapshotReadError::Unavailable(Box::new(error)))?;
            Ok(AppliedAuthorEditActivity {
                event_id: row.get(0),
                project_sequence,
                stream_sequence: project_sequence,
                command_id: row.get(2),
                correlation_id: row.get(3),
                receipt_id: row.get(4),
                chapter_id: row.get(5),
                authoritative_revision_id: row.get(6),
                authoritative_commit_id: row.get(7),
                author_action_sequence: row.get(8),
                occurred_at: row.get(9),
            })
        })
        .collect()
}
