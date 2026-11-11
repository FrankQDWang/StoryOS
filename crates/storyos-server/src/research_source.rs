use storyos_application::ResearchSourceReadStore;

use super::*;
use crate::create_agent_run::agent_run_read_error;

/// The Source Snapshots and Source Gaps of one AgentRun of the authenticated Project Scope.
pub(super) async fn list_research_sources(
    State(state): State<Arc<ServerState>>,
    Path((project_id, run_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<contracts::ListResearchSourcesResponse>, ApiError> {
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    valid_uuid(&run_id)?;
    let reader = project_reader(&state).await?;
    let Some(sources) = reader
        .list_research_sources(&scope, &run_id)
        .await
        .map_err(agent_run_read_error)?
    else {
        return Err(resource_unavailable());
    };
    Ok(Json(contracts::ListResearchSourcesResponse {
        schema_id: contracts::LIST_RESEARCH_SOURCES_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: Uuid::now_v7().to_string(),
        project_scope: contract_scope(&scope),
        run_id,
        snapshots: sources.snapshots,
        gaps: sources.gaps,
        unknown_calls: sources.unknown_calls,
    }))
}

/// One Source Snapshot revision of the authenticated Project Scope, with its text.
pub(super) async fn get_research_source(
    State(state): State<Arc<ServerState>>,
    Path((project_id, source_snapshot_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<contracts::GetResearchSourceResponse>, ApiError> {
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    valid_uuid(&source_snapshot_id)?;
    let reader = project_reader(&state).await?;
    let Some(source) = reader
        .get_research_source(&scope, &source_snapshot_id)
        .await
        .map_err(agent_run_read_error)?
    else {
        return Err(resource_unavailable());
    };
    Ok(Json(contracts::GetResearchSourceResponse {
        schema_id: contracts::GET_RESEARCH_SOURCE_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: Uuid::now_v7().to_string(),
        project_scope: contract_scope(&scope),
        snapshot: source.snapshot,
        text: source.text,
    }))
}
