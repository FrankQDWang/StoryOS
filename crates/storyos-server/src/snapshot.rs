use axum::extract::Query;
use axum::http::HeaderValue;
use serde::Deserialize;
use storyos_application::{SnapshotLookup, SnapshotReadError};

use super::*;

#[derive(Deserialize)]
pub(super) struct ActivityStreamQuery {
    snapshot_id: String,
    protocol_release: String,
}

pub(super) async fn get_snapshot(
    State(state): State<Arc<ServerState>>,
    Path((project_id, snapshot_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<contracts::GetSnapshotResponse>, ApiError> {
    let (scope, lookup, store) =
        authorized_snapshot_lookup(&state, &headers, &project_id, &snapshot_id).await?;
    let snapshot = storyos_application::get_snapshot(&store, &lookup)
        .await
        .map_err(snapshot_read_error)?
        .ok_or_else(resource_unavailable)?;
    let project_scope = contract_scope(&scope);
    Ok(Json(contracts::GetSnapshotResponse {
        schema_id: contracts::GET_SNAPSHOT_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: Uuid::now_v7().to_string(),
        project_scope: project_scope.clone(),
        snapshot: contracts::SnapshotDescriptor {
            snapshot_id: snapshot.snapshot_id,
            project_scope,
            snapshot_kind: contracts::SnapshotKind::Canonical,
            project_activity_position: snapshot.project_activity_position.to_string(),
            source_watermarks: contracts::CanonicalSnapshotMaps {},
            projection_generations: contracts::CanonicalSnapshotMaps {},
            redaction_profile: snapshot.redaction_profile,
            schema_profile: snapshot.schema_profile,
            replay_generation: snapshot.replay_generation.to_string(),
            created_at: snapshot.created_at,
            expires_at: snapshot.expires_at,
        },
    }))
}

pub(super) async fn activity_stream(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    Query(query): Query<ActivityStreamQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if query.protocol_release != contracts::PUBLIC_PROTOCOL_RELEASE {
        return Err(invalid_request());
    }
    let (_, lookup, store) =
        authorized_snapshot_lookup(&state, &headers, &project_id, &query.snapshot_id).await?;
    storyos_application::activity_stream_resume(&store, &lookup)
        .await
        .map_err(snapshot_read_error)?;
    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    response_headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok((StatusCode::OK, response_headers, String::new()).into_response())
}

pub(super) async fn snapshot_method_not_allowed() -> ApiError {
    problem(
        StatusCode::METHOD_NOT_ALLOWED,
        "method_not_allowed",
        "The request method is not allowed.",
    )
}

async fn authorized_snapshot_lookup(
    state: &ServerState,
    headers: &HeaderMap,
    project_id: &str,
    snapshot_id: &str,
) -> Result<(ApplicationScope, SnapshotLookup, PostgresProjectReader), ApiError> {
    let scope = authenticate_scope(
        state,
        headers,
        project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    valid_uuid(snapshot_id)?;
    let store = project_reader(state)?;
    let lookup = SnapshotLookup {
        project_scope: scope.clone(),
        snapshot_id: snapshot_id.to_owned(),
    };
    Ok((scope, lookup, store))
}

fn snapshot_read_error(error: SnapshotReadError) -> ApiError {
    match error {
        SnapshotReadError::Expired => problem(
            StatusCode::CONFLICT,
            "snapshot_expired",
            "The Snapshot is no longer available.",
        ),
        SnapshotReadError::CursorTooOld => problem(
            StatusCode::CONFLICT,
            "activity_cursor_too_old",
            "The Activity cursor is below the replay floor.",
        ),
        SnapshotReadError::Unavailable(_) => resource_unavailable(),
    }
}
