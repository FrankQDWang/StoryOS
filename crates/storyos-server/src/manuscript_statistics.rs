use axum::extract::Query;
use serde::Deserialize;
use storyos_application::{
    GetManuscriptStatistics, ManuscriptStatisticsRequest, get_manuscript_statistics,
};

use super::*;

#[derive(Deserialize)]
pub(super) struct StatisticsQuery {
    required_watermark: Option<String>,
}

pub(super) async fn get_statistics(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    Query(query): Query<StatisticsQuery>,
    headers: HeaderMap,
) -> Result<Json<contracts::GetStatisticsResponse>, ApiError> {
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    let required_watermark = match query.required_watermark.as_deref() {
        None => None,
        Some(value) => Some(value.parse::<u64>().map_err(|_| invalid_request())?),
    };
    let reader = project_reader(&state)?;
    match get_manuscript_statistics(
        &reader,
        &scope,
        &ManuscriptStatisticsRequest { required_watermark },
    )
    .await
    .map_err(service_unavailable)?
    {
        GetManuscriptStatistics::Missing => Err(resource_unavailable()),
        GetManuscriptStatistics::SnapshotExpired => Err(problem(
            StatusCode::CONFLICT,
            "snapshot_expired",
            "The Snapshot is no longer available.",
        )),
        GetManuscriptStatistics::ProjectionNotReady { .. } => Err(problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "projection_not_ready",
            "The required statistics projection watermark is not ready.",
        )),
        GetManuscriptStatistics::Ready(page) => {
            let manuscript = contracts::ManuscriptTotals {
                chapter_count: page.manuscript.chapter_count.to_string(),
                word_count: page.manuscript.word_count.to_string(),
                character_count: page.manuscript.character_count.to_string(),
            };
            let current_chapter =
                page.current_chapter
                    .map(|chapter| contracts::ChapterStatistics {
                        chapter_id: chapter.chapter_id.as_ref().to_owned(),
                        word_count: chapter.word_count.to_string(),
                        character_count: chapter.character_count.to_string(),
                    });
            let page_bytes = serde_json::to_vec(&manuscript)
                .expect("statistics totals serialize")
                .len();
            let project_scope = contract_scope(&scope);
            Ok(Json(contracts::GetStatisticsResponse {
                schema_id: contracts::GET_STATISTICS_RESPONSE_SCHEMA_ID.to_owned(),
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
                completeness: contracts::ManuscriptStatisticsCompleteness::Complete,
                lag: page.lag.to_string(),
                counting_profile: page.counting_profile,
                current_chapter,
                manuscript,
                next_cursor: None,
                page_count: "1".to_owned(),
                page_bytes: page_bytes.to_string(),
                redaction_profile: page.source_snapshot.redaction_profile,
                limit_profile_revision:
                    storyos_application::MANUSCRIPT_STATISTICS_LIMIT_PROFILE_REVISION.to_owned(),
            }))
        }
    }
}
