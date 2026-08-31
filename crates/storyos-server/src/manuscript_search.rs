use axum::body::to_bytes;
use storyos_application::{
    ManuscriptSearchRequest as ApplicationSearchRequest,
    ManuscriptSearchSelection as ApplicationSelection, SearchManuscript, search_manuscript,
};

use super::project_command_challenge::validate_json_content_type;
use super::*;

pub(super) async fn search_manuscript_query(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    request: Request,
) -> Result<Json<contracts::SearchManuscriptResponse>, ApiError> {
    let (parts, body_stream) = request.into_parts();
    let headers = parts.headers;
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    validate_json_content_type(&headers)?;
    let bytes = to_bytes(body_stream, contracts::AUTHOR_EDIT_MAX_WIRE_BODY_BYTES)
        .await
        .map_err(|_| payload_too_large())?;
    let body = serde_json::from_slice::<contracts::SearchManuscriptRequest>(&bytes)
        .map_err(|_| invalid_request_shape())?;
    if body.schema_id != contracts::SEARCH_MANUSCRIPT_REQUEST_SCHEMA_ID
        || body.query_text.is_empty()
    {
        return Err(invalid_request());
    }
    let required_watermark = match body.required_watermark.as_deref() {
        None => None,
        Some(value) => Some(value.parse::<u64>().map_err(|_| invalid_request())?),
    };
    let reader = project_reader(&state)?;
    match search_manuscript(
        &reader,
        &scope,
        &ApplicationSearchRequest {
            query_text: body.query_text,
            selection: match body.selection {
                contracts::ManuscriptSearchSelection::CurrentChapter => {
                    ApplicationSelection::CurrentChapter
                }
                contracts::ManuscriptSearchSelection::Manuscript => {
                    ApplicationSelection::Manuscript
                }
            },
            required_watermark,
        },
    )
    .await
    .map_err(service_unavailable)?
    {
        SearchManuscript::Missing => Err(resource_unavailable()),
        SearchManuscript::SnapshotExpired => Err(problem(
            StatusCode::CONFLICT,
            "snapshot_expired",
            "The Snapshot is no longer available.",
        )),
        SearchManuscript::ProjectionNotReady { .. } => Err(problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "projection_not_ready",
            "The required search projection watermark is not ready.",
        )),
        SearchManuscript::Ready(page) => {
            let items: Vec<contracts::ManuscriptSearchMatch> = page
                .items
                .into_iter()
                .map(|item| contracts::ManuscriptSearchMatch {
                    chapter_id: item.chapter_id.as_ref().to_owned(),
                    manuscript_block_id: item.manuscript_block_id,
                    start: item.start.to_string(),
                    end: item.end.to_string(),
                })
                .collect();
            let page_bytes = serde_json::to_vec(&items)
                .expect("search matches serialize")
                .len();
            let project_scope = contract_scope(&scope);
            Ok(Json(contracts::SearchManuscriptResponse {
                schema_id: contracts::SEARCH_MANUSCRIPT_RESPONSE_SCHEMA_ID.to_owned(),
                query_id: Uuid::now_v7().to_string(),
                correlation_id: Uuid::now_v7().to_string(),
                project_scope: project_scope.clone(),
                source_snapshot: contracts::SnapshotDescriptor {
                    snapshot_id: page.source_snapshot.snapshot_id,
                    project_scope,
                    snapshot_kind: contracts::SnapshotKind::Canonical,
                    project_activity_position: page
                        .source_snapshot
                        .project_activity_position
                        .to_string(),
                    source_watermarks: contracts::CanonicalSnapshotMaps {},
                    projection_generations: contracts::CanonicalSnapshotMaps {},
                    redaction_profile: page.source_snapshot.redaction_profile.clone(),
                    schema_profile: page.source_snapshot.schema_profile,
                    replay_generation: page.source_snapshot.replay_generation.to_string(),
                    created_at: page.source_snapshot.created_at,
                    expires_at: page.source_snapshot.expires_at,
                },
                projection_kind: page.projection_kind,
                projection_generation: page.projection_generation.to_string(),
                projection_watermark: page.projection_watermark.to_string(),
                required_watermark: page.required_watermark.map(|value| value.to_string()),
                completeness: match page.completeness {
                    storyos_application::ManuscriptSearchCompleteness::Complete => {
                        contracts::ManuscriptSearchCompleteness::Complete
                    }
                    storyos_application::ManuscriptSearchCompleteness::Truncated => {
                        contracts::ManuscriptSearchCompleteness::Truncated
                    }
                },
                lag: page.lag.to_string(),
                next_cursor: None,
                page_count: page.page_count.to_string(),
                page_bytes: page_bytes.to_string(),
                redaction_profile: page.source_snapshot.redaction_profile,
                limit_profile_revision:
                    storyos_application::MANUSCRIPT_SEARCH_LIMIT_PROFILE_REVISION.to_owned(),
                items,
            }))
        }
    }
}
