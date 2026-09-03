use storyos_application::{
    ActivityAggregateRef, CanonicalSnapshot, ProjectActivityEvent, ProjectActivityKind,
    ProjectReadError, ProjectScope, SnapshotLookup, SnapshotReadError, SnapshotStore,
};

use super::*;

pub(crate) async fn persist_canonical_snapshot(
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

pub(crate) async fn load_latest_canonical_snapshot(
    client: &impl tokio_postgres::GenericClient,
    scope: &ProjectScope,
) -> Result<Option<CanonicalSnapshot>, ProjectReadError> {
    let row = client
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
    row.map(canonical_snapshot_from_row).transpose()
}

pub(crate) async fn load_canonical_snapshot_by_id(
    client: &impl tokio_postgres::GenericClient,
    scope: &ProjectScope,
    snapshot_id: &str,
) -> Result<Option<CanonicalSnapshot>, ProjectReadError> {
    let row = client
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
                AND snapshot.snapshot_id = $3::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &snapshot_id,
            ],
        )
        .await
        .map_err(read_error)?;
    row.map(canonical_snapshot_from_row).transpose()
}

fn canonical_snapshot_from_row(
    row: tokio_postgres::Row,
) -> Result<CanonicalSnapshot, ProjectReadError> {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CanonicalSnapshotQuery {
    Available(CanonicalSnapshot),
    Expired,
    Missing,
}

pub(super) async fn load_latest_canonical_snapshot_query(
    transaction: &tokio_postgres::Transaction<'_>,
    scope: &ProjectScope,
) -> Result<CanonicalSnapshotQuery, ProjectReadError> {
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
                    current_floor.floor_position::text,
                    snapshot.expires_at IS NOT NULL AND snapshot.expires_at <= clock_timestamp()
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
              ORDER BY snapshot.project_activity_position DESC, snapshot.created_at DESC
              LIMIT 1",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(read_error)?;
    let Some(row) = row else {
        return Ok(CanonicalSnapshotQuery::Missing);
    };
    if row.get::<_, bool>(8) {
        return Ok(CanonicalSnapshotQuery::Expired);
    }
    Ok(CanonicalSnapshotQuery::Available(
        canonical_snapshot_from_row(row)?,
    ))
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

    async fn list_project_activity(
        &self,
        lookup: &SnapshotLookup,
        after_position: u64,
    ) -> Result<Vec<ProjectActivityEvent>, SnapshotReadError> {
        list_project_activity(self, lookup, after_position).await
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

async fn list_project_activity(
    store: &PostgresProjectReader,
    lookup: &SnapshotLookup,
    after_position: u64,
) -> Result<Vec<ProjectActivityEvent>, SnapshotReadError> {
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
            "SELECT stream.event_id,
                    stream.project_activity_position,
                    stream.event_kind,
                    stream.command_id,
                    stream.correlation_id,
                    stream.receipt_id,
                    stream.occurred_at,
                    stream.payload_json
               FROM (
                 SELECT activity.project_activity_event_id::text AS event_id,
                        activity.project_activity_position::text AS project_activity_position,
                        activity.event_kind,
                        receipt.command_id::text AS command_id,
                        admission.correlation_id::text AS correlation_id,
                        activity.receipt_id::text AS receipt_id,
                        to_char(activity.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"') AS occurred_at,
                        jsonb_build_object(
                          'chapter_id', admission.chapter_object_id::text,
                          'authoritative_revision_id', activity.resulting_revision_id::text,
                          'authoritative_commit_id', activity.authoritative_commit_id::text,
                          'author_action_sequence', activity.author_action_sequence::text
                        )::text AS payload_json
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
                 UNION ALL
                 SELECT payload.project_activity_event_id::text,
                        payload.project_activity_position::text,
                        payload.event_kind,
                        receipt.command_id::text,
                        admission.correlation_id::text,
                        payload.receipt_id::text,
                        to_char(payload.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                        payload.payload::text
                   FROM storyos.project_activity_event_payloads AS payload
                   JOIN storyos.domain_receipts AS receipt
                     ON (receipt.owner_user_id, receipt.project_id, receipt.receipt_id) =
                        (payload.owner_user_id, payload.project_id, payload.receipt_id)
                   JOIN storyos.author_command_admissions AS admission
                     ON (admission.owner_user_id, admission.project_id, admission.command_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.command_id)
                  WHERE payload.owner_user_id = $1::text::uuid
                    AND payload.project_id = $2::text::uuid
                    AND payload.project_activity_position > $3::text::numeric
               ) AS stream
              ORDER BY stream.project_activity_position::numeric",
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
            let kind = ProjectActivityKind::from_persisted(row.get::<_, String>(2).as_str())
                .ok_or_else(|| {
                    SnapshotReadError::Unavailable("persisted Activity kind is unprojected".into())
                })?;
            let payload_json: String = row.get(7);
            let payload: serde_json::Value = serde_json::from_str(&payload_json)
                .map_err(|error| SnapshotReadError::Unavailable(Box::new(error)))?;
            Ok(ProjectActivityEvent {
                event_id: row.get(0),
                kind,
                project_sequence,
                stream_sequence: project_sequence,
                command_id: row.get(3),
                correlation_id: row.get(4),
                receipt_id: row.get(5),
                occurred_at: row.get(6),
                aggregate: payload_aggregate(
                    kind,
                    &payload,
                    lookup.project_scope.project_id.as_ref(),
                )?,
                payload_json,
            })
        })
        .collect()
}

fn payload_aggregate(
    kind: ProjectActivityKind,
    payload: &serde_json::Value,
    project_id: &str,
) -> Result<ActivityAggregateRef, SnapshotReadError> {
    let field = |key: &str| {
        payload
            .get(key)
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| SnapshotReadError::Unavailable("activity payload is incomplete".into()))
    };
    Ok(match kind {
        ProjectActivityKind::AuthoritativeAuthorEditApplied
        | ProjectActivityKind::ChapterCreated
        | ProjectActivityKind::ChapterUpdated
        | ProjectActivityKind::ChapterDeleted => ActivityAggregateRef {
            kind: "chapter".to_owned(),
            id: field("chapter_id")?,
        },
        ProjectActivityKind::CurrentChapterSet => ActivityAggregateRef {
            kind: "chapter".to_owned(),
            id: field("current_chapter_id")?,
        },
        ProjectActivityKind::VolumeCreated
        | ProjectActivityKind::VolumeUpdated
        | ProjectActivityKind::VolumeDeleted => ActivityAggregateRef {
            kind: "volume".to_owned(),
            id: field("volume_id")?,
        },
        ProjectActivityKind::ProjectCreated
        | ProjectActivityKind::ProjectUpdated
        | ProjectActivityKind::ProjectArchivalChanged => ActivityAggregateRef {
            kind: "project".to_owned(),
            id: project_id.to_owned(),
        },
        ProjectActivityKind::WriterTakeoverApplied => ActivityAggregateRef {
            kind: "editor_session".to_owned(),
            id: field("resulting_editor_session_id")?,
        },
        ProjectActivityKind::WriterTakeoverCompareFailed => ActivityAggregateRef {
            kind: "project".to_owned(),
            id: project_id.to_owned(),
        },
        ProjectActivityKind::HumanReadableManuscriptExportSettled
        | ProjectActivityKind::ProjectExportSettled => ActivityAggregateRef {
            kind: "export".to_owned(),
            id: field("export_id")?,
        },
    })
}
