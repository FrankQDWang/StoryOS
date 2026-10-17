use storyos_application::list_open_proposals;

use super::*;

pub(super) async fn list_chapter_open_proposals(
    State(state): State<Arc<ServerState>>,
    Path((project_id, chapter_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<contracts::ListChapterOpenProposalsResponse>, ApiError> {
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    valid_uuid(&chapter_id)?;
    let reader = project_reader(&state).await?;
    let list = list_open_proposals(&reader, &scope, &chapter_id)
        .await
        .map_err(service_unavailable)?;
    Ok(Json(contracts::ListChapterOpenProposalsResponse {
        schema_id: contracts::LIST_CHAPTER_OPEN_PROPOSALS_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: Uuid::now_v7().to_string(),
        project_scope: contract_scope(&scope),
        chapter_id,
        completeness: list.completeness,
        proposals: list
            .items
            .into_iter()
            .map(|item| contracts::OpenProposalEntry {
                proposal_id: item.proposal_id,
                revision_id: item.revision_id,
                kind: item.kind,
                source: item.source,
            })
            .collect(),
    }))
}
