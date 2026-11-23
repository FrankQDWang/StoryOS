use storyos_application::{
    list_project_conversations as list_conversations,
    read_project_conversation as read_conversation, wire_agent_run_status,
};

use super::*;

pub(super) async fn list_project_conversations(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<contracts::ListProjectConversationsResponse>, ApiError> {
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    let reader = project_reader(&state).await?;
    let Some(list) = list_conversations(&reader, &scope)
        .await
        .map_err(service_unavailable)?
    else {
        return Err(resource_unavailable());
    };
    Ok(Json(contracts::ListProjectConversationsResponse {
        schema_id: contracts::LIST_PROJECT_CONVERSATIONS_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: Uuid::now_v7().to_string(),
        project_scope: contract_scope(&scope),
        completeness: list.completeness,
        conversations: list
            .items
            .into_iter()
            .map(|item| contracts::ProjectConversationEntry {
                conversation_id: item.conversation_id,
                label: item.label,
                first_run_created_at: item.first_run_created_at,
                newest_run_created_at: item.newest_run_created_at,
                newest_run_status: wire_agent_run_status(item.newest_run_status),
            })
            .collect(),
    }))
}

pub(super) async fn get_project_conversation(
    State(state): State<Arc<ServerState>>,
    Path((project_id, conversation_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<contracts::GetProjectConversationResponse>, ApiError> {
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    valid_uuid(&conversation_id)?;
    let reader = project_reader(&state).await?;
    let Some(read) = read_conversation(&reader, &scope, &conversation_id)
        .await
        .map_err(service_unavailable)?
    else {
        return Err(resource_unavailable());
    };
    Ok(Json(contracts::GetProjectConversationResponse {
        schema_id: contracts::GET_PROJECT_CONVERSATION_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: Uuid::now_v7().to_string(),
        project_scope: contract_scope(&scope),
        conversation_id,
        completeness: read.completeness,
        runs: read
            .runs
            .into_iter()
            .map(|run| contracts::ProjectConversationRun {
                run_id: run.run_id,
                author_message: run.author_message,
                chapter_id: run.chapter_id,
                status: wire_agent_run_status(run.status),
                created_at: run.created_at,
            })
            .collect(),
    }))
}
