use storyos_application::{ProjectLifecycle, list_owned_projects};

use super::*;

pub(super) async fn list_projects(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
) -> Result<Json<contracts::ListProjectsResponse>, ApiError> {
    let request_origin = validate_request_site(
        &state,
        &headers,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    let session_handle = session_cookie(&headers).ok_or_else(authentication_required)?;
    let session = state
        .config
        .session_bindings
        .get(session_handle)
        .ok_or_else(authentication_required)?;
    validate_session_binding(&state, session_handle, session, &headers, &request_origin)?;
    let reader = project_reader(&state)?;
    let projects = list_owned_projects(&reader, &session.owner_user_id)
        .await
        .map_err(service_unavailable)?;
    Ok(Json(contracts::ListProjectsResponse {
        schema_id: contracts::LIST_PROJECTS_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: Uuid::now_v7().to_string(),
        owner_user_id: session.owner_user_id.as_ref().to_owned(),
        projects: projects
            .into_iter()
            .map(|item| contracts::ProjectListItem {
                project_scope: contract_scope(&item.project_scope),
                title: item.title,
                lifecycle: match item.lifecycle {
                    ProjectLifecycle::Active => contracts::ProjectLifecycleState::Active,
                    ProjectLifecycle::Archived => contracts::ProjectLifecycleState::Archived,
                },
                revision: item.revision.to_string(),
                open: match item.current_chapter_id {
                    Some(chapter_id) => contracts::ProjectOpenState::CurrentChapter {
                        current_chapter_id: chapter_id.as_ref().to_owned(),
                    },
                    None => contracts::ProjectOpenState::Empty,
                },
            })
            .collect(),
    }))
}
