use axum::extract::Query;
use axum::http::HeaderValue;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use storyos_application::{
    CanonicalSnapshot, ProjectActivityEvent, SnapshotLookup, SnapshotReadError, SnapshotStore,
};

use super::project_command_challenge::{hex_bytes, secret_digest};
use super::*;

const ACTIVITY_CURSOR_HMAC_PROFILE: &str = "storyos.activity-stream.cursor.hmac-sha256.v1";
const ACTIVITY_PROFILE: &str = "storyos.project-activity.v1";
const COMPATIBILITY_PROFILE: &str = "storyos.public.same-release.v1";

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
    let (scope, lookup, store) =
        authorized_snapshot_lookup(&state, &headers, &project_id, &query.snapshot_id).await?;
    let secret = state
        .config
        .project_command_challenge_secret
        .as_deref()
        .filter(|secret| secret.len() >= 32)
        .ok_or_else(challenge_store_unavailable)?;
    let snapshot = storyos_application::activity_stream_resume(&store, &lookup)
        .await
        .map_err(snapshot_read_error)?;
    let after_position = match headers
        .get("last-event-id")
        .map(|value| value.to_str().map_err(|_| resource_unavailable()))
        .transpose()?
    {
        Some(last_event_id) => decode_activity_cursor(secret, last_event_id, &scope, &snapshot)?,
        None => snapshot.project_activity_position,
    };
    let events = store
        .list_project_activity(&lookup, after_position)
        .await
        .map_err(snapshot_read_error)?;
    let mut body = String::new();
    for event in events {
        let frame = format_activity_frame(secret, &scope, &snapshot, &event)?;
        body.push_str(&frame);
    }
    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    response_headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok((StatusCode::OK, response_headers, body).into_response())
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

fn format_activity_frame(
    secret: &[u8],
    scope: &ApplicationScope,
    snapshot: &CanonicalSnapshot,
    event: &ProjectActivityEvent,
) -> Result<String, ApiError> {
    let payload: serde_json::Value =
        serde_json::from_str(&event.payload_json).map_err(|_| resource_unavailable())?;
    let payload_bytes = serde_json::to_vec(&payload).map_err(|_| resource_unavailable())?;
    let payload_digest = hex_bytes(&Sha256::digest(payload_bytes));
    let data = serde_json::json!({
        "envelope_version": 1,
        "activity_profile": ACTIVITY_PROFILE,
        "event_id": event.event_id,
        "event_schema": event.kind.event_schema(),
        "event_kind": event.kind.as_str(),
        "project_scope": {
            "owner_user_id": scope.owner_user_id.as_ref(),
            "project_id": scope.project_id.as_ref(),
        },
        "requester_user_id": scope.owner_user_id.as_ref(),
        "actor": {
            "kind": "author",
            "id": scope.owner_user_id.as_ref(),
        },
        "project_sequence": event.project_sequence.to_string(),
        "stream_sequence": event.stream_sequence.to_string(),
        "agent_run_id": serde_json::Value::Null,
        "run_step_id": serde_json::Value::Null,
        "run_sequence": serde_json::Value::Null,
        "aggregate_ref": {
            "kind": event.aggregate.kind.as_str(),
            "id": event.aggregate.id.as_str(),
        },
        "correlation_id": event.correlation_id,
        "causation": {
            "kind": "command",
            "id": event.command_id.as_str(),
        },
        "command_id": event.command_id.as_str(),
        "receipt_ref": {
            "kind": "domain_receipt",
            "id": event.receipt_id,
        },
        "occurred_at": event.occurred_at,
        "recorded_at": event.occurred_at,
        "payload": payload,
        "payload_digest": {
            "algorithm": "sha256",
            "profile": "storyos.event-payload.jcs.v1",
            "value_hex_lowercase": payload_digest,
        },
        "application_wire_record_ref": event.event_id,
        "limit_profile_revision": contracts::LIMIT_PROFILE_REVISION,
    });
    let data_bytes = serde_json::to_string(&data).map_err(|_| resource_unavailable())?;
    let cursor = encode_activity_cursor(secret, scope, snapshot, event.project_sequence);
    Ok(format!(
        "id: {cursor}\nevent: storyos.project-activity\ndata: {data_bytes}\n\n"
    ))
}

fn encode_activity_cursor(
    secret: &[u8],
    scope: &ApplicationScope,
    snapshot: &CanonicalSnapshot,
    sequence_position: u64,
) -> String {
    let replay_generation = snapshot.replay_generation.to_string();
    let sequence = sequence_position.to_string();
    let mac = secret_digest(
        secret,
        ACTIVITY_CURSOR_HMAC_PROFILE,
        &activity_cursor_parts(
            scope,
            &replay_generation,
            &sequence,
            &snapshot.redaction_profile,
        ),
    );
    format!("v1.{replay_generation}.{sequence}.{mac}")
}

fn decode_activity_cursor(
    secret: &[u8],
    last_event_id: &str,
    scope: &ApplicationScope,
    snapshot: &CanonicalSnapshot,
) -> Result<u64, ApiError> {
    let mut parts = last_event_id.split('.');
    let version = parts.next();
    let replay_generation = parts.next();
    let sequence = parts.next();
    let mac = parts.next();
    if parts.next().is_some() || version != Some("v1") {
        return Err(resource_unavailable());
    }
    let (Some(replay_generation), Some(sequence), Some(mac)) = (replay_generation, sequence, mac)
    else {
        return Err(resource_unavailable());
    };
    let expected = secret_digest(
        secret,
        ACTIVITY_CURSOR_HMAC_PROFILE,
        &activity_cursor_parts(
            scope,
            replay_generation,
            sequence,
            &snapshot.redaction_profile,
        ),
    );
    if expected != mac {
        return Err(resource_unavailable());
    }
    let replay_generation = replay_generation
        .parse::<u64>()
        .map_err(|_| resource_unavailable())?;
    let sequence = sequence
        .parse::<u64>()
        .map_err(|_| resource_unavailable())?;
    if replay_generation != snapshot.replay_generation || sequence < snapshot.floor_position {
        return Err(snapshot_read_error(SnapshotReadError::CursorTooOld));
    }
    Ok(sequence)
}

fn activity_cursor_parts(
    scope: &ApplicationScope,
    replay_generation: &str,
    sequence_position: &str,
    redaction_profile: &str,
) -> [Vec<u8>; 9] {
    let filter_digest = hex_bytes(&Sha256::digest(b"{}"));
    [
        scope.owner_user_id.as_ref().as_bytes().to_vec(),
        scope.project_id.as_ref().as_bytes().to_vec(),
        scope.owner_user_id.as_ref().as_bytes().to_vec(),
        replay_generation.as_bytes().to_vec(),
        sequence_position.as_bytes().to_vec(),
        ACTIVITY_PROFILE.as_bytes().to_vec(),
        redaction_profile.as_bytes().to_vec(),
        COMPATIBILITY_PROFILE.as_bytes().to_vec(),
        filter_digest.as_bytes().to_vec(),
    ]
}
