use storyos_application::{ChapterId, OpenChapter, open_chapter};

use super::*;

pub(super) async fn get_chapter(
    State(state): State<Arc<ServerState>>,
    Path((project_id, chapter_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<contracts::GetChapterResponse>, ApiError> {
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    valid_uuid(&chapter_id)?;
    let chapter_id = ChapterId::new(chapter_id);
    let reader = project_reader(&state)?;
    match open_chapter(&reader, &scope, &chapter_id)
        .await
        .map_err(service_unavailable)?
    {
        OpenChapter::Missing => Err(resource_unavailable()),
        OpenChapter::SnapshotExpired => Err(problem(
            StatusCode::CONFLICT,
            "snapshot_expired",
            "The Snapshot is no longer available.",
        )),
        OpenChapter::Found(facts) => Ok(Json(contracts::GetChapterResponse {
            schema_id: "storyos.query.chapter.response.v1".to_owned(),
            correlation_id: Uuid::now_v7().to_string(),
            project_scope: contract_scope(&scope),
            project_activity_position: facts.chapter.project_activity_position.to_string(),
            chapter: contracts::CurrentChapter {
                chapter_id: facts.chapter.chapter_id.as_ref().to_owned(),
                title: facts.chapter.title,
                current_revision: contract_chapter_revision(
                    facts.chapter.revision_id.as_ref().to_owned(),
                    facts.chapter.body,
                    &facts.chapter.blocks,
                ),
            },
        })),
    }
}
