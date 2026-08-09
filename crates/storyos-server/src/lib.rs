//! StoryOS public HTTP boundary.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Path, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::{Json, Router, routing};
use storyos_adapter_postgres::PostgresProjectReader;
use storyos_application::{
    ChapterId, ProjectId, ProjectScope as ApplicationScope, UserId, open_current_chapter,
    open_project,
};
use storyos_contracts as contracts;
use uuid::Uuid;

/// The reviewed browser security policy accepted by this Server release.
pub const RELEASE_1_SECURITY_POLICY_REVISION: &str = "storyos.web-security-policy.release-1.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientSessionBinding {
    pub owner_user_id: UserId,
    pub allowed_host: String,
    pub allowed_origin: String,
    pub session_generation: u64,
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
}

#[derive(Clone, Debug, Default)]
pub struct ServerConfig {
    pub database_url: Option<String>,
    pub session_bindings: HashMap<String, ClientSessionBinding>,
    pub current_session_generation: u64,
    pub accepted_client_contract_revision: Option<String>,
    pub accepted_security_policy_revision: Option<String>,
    pub allowed_host: Option<String>,
    pub allowed_origin: Option<String>,
}

#[derive(Clone)]
struct ServerState {
    config: ServerConfig,
}

type ApiError = (StatusCode, Json<contracts::StoryOSProblem>);

/// Build the protocol-only router used when no controlled Project store is configured.
pub fn router() -> Router {
    router_with_config(ServerConfig::default())
}

/// Build the one StoryOS Server router used by production and boundary tests.
pub fn router_with_config(config: ServerConfig) -> Router {
    let protocol_method = method_filter(contracts::GET_PROTOCOL_PROFILE_METHOD);
    let project_method = method_filter(contracts::GET_PROJECT_METHOD);
    let chapter_method = method_filter(contracts::GET_CHAPTER_METHOD);
    Router::new()
        .route(
            contracts::GET_PROTOCOL_PROFILE_PATH,
            routing::on(protocol_method, get_protocol_profile),
        )
        .route(
            contracts::GET_PROJECT_PATH,
            routing::on(project_method, get_project),
        )
        .route(
            contracts::GET_CHAPTER_PATH,
            routing::on(chapter_method, get_chapter),
        )
        .with_state(Arc::new(ServerState { config }))
}

fn method_filter(method: &str) -> routing::MethodFilter {
    let method = Method::from_bytes(method.as_bytes()).expect("contract method must be valid HTTP");
    routing::MethodFilter::try_from(method).expect("contract method must be routable")
}

async fn get_protocol_profile() -> Json<contracts::Release1ProtocolProfile> {
    Json(contracts::release1_protocol_profile())
}

async fn get_project(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<contracts::GetProjectResponse>, ApiError> {
    let scope = authenticate_scope(&state, &headers, &project_id)?;
    let reader = project_reader(&state)?;
    let Some(project) = open_project(&reader, &scope)
        .await
        .map_err(service_unavailable)?
    else {
        return Err(resource_unavailable());
    };
    Ok(Json(contracts::GetProjectResponse {
        schema_id: "storyos.query.project.response.v1".to_owned(),
        correlation_id: Uuid::now_v7().to_string(),
        project_scope: contract_scope(&scope),
        project: contracts::ControlledProject {
            project_id: project.project_id.as_ref().to_owned(),
            title: project.title,
            current_chapter_id: project.current_chapter_id.as_ref().to_owned(),
        },
    }))
}

async fn get_chapter(
    State(state): State<Arc<ServerState>>,
    Path((project_id, chapter_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<contracts::GetChapterResponse>, ApiError> {
    let scope = authenticate_scope(&state, &headers, &project_id)?;
    valid_uuid(&chapter_id)?;
    let chapter_id = ChapterId::new(chapter_id);
    let reader = project_reader(&state)?;
    let Some(chapter) = open_current_chapter(&reader, &scope, &chapter_id)
        .await
        .map_err(service_unavailable)?
    else {
        return Err(resource_unavailable());
    };
    Ok(Json(contracts::GetChapterResponse {
        schema_id: "storyos.query.chapter.response.v1".to_owned(),
        correlation_id: Uuid::now_v7().to_string(),
        project_scope: contract_scope(&scope),
        chapter: contracts::CurrentChapter {
            chapter_id: chapter.chapter_id.as_ref().to_owned(),
            title: chapter.title,
            current_revision: contracts::AuthoritativeChapterRevision {
                revision_id: chapter.revision_id.as_ref().to_owned(),
                body: chapter.body,
            },
        },
    }))
}

fn authenticate_scope(
    state: &ServerState,
    headers: &HeaderMap,
    project_id: &str,
) -> Result<ApplicationScope, ApiError> {
    valid_uuid(project_id)?;
    validate_request_site(&state.config, headers)?;
    let session_handle = session_cookie(headers).ok_or_else(authentication_required)?;
    let binding = state
        .config
        .session_bindings
        .get(session_handle)
        .ok_or_else(authentication_required)?;
    validate_session_binding(&state.config, binding, headers)?;
    Ok(ApplicationScope::new(
        binding.owner_user_id.clone(),
        ProjectId::new(project_id),
    ))
}

fn validate_request_site(config: &ServerConfig, headers: &HeaderMap) -> Result<(), ApiError> {
    let host = headers.get("host").and_then(|value| value.to_str().ok());
    if let Some(expected) = &config.allowed_host
        && host != Some(expected.as_str())
    {
        return Err(request_site_refused());
    }
    let origin = request_origin(headers);
    if let Some(expected) = &config.allowed_origin
        && origin != Some(expected.as_str())
    {
        return Err(request_site_refused());
    }
    Ok(())
}

fn request_origin(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("origin")
        .and_then(|value| value.to_str().ok())
        .or_else(|| {
            let referer = headers.get("referer")?.to_str().ok()?;
            let scheme_end = referer.find("://")? + 3;
            let path_start = referer[scheme_end..]
                .find('/')
                .map_or(referer.len(), |index| scheme_end + index);
            Some(&referer[..path_start])
        })
}

fn session_cookie(headers: &HeaderMap) -> Option<&str> {
    let mut cookie_headers = headers.get_all("cookie").iter();
    let cookie = cookie_headers.next()?.to_str().ok()?;
    if cookie_headers.next().is_some() {
        return None;
    }

    let mut session_handle = None;
    for part in cookie.split(';') {
        if let Some(value) = part.trim().strip_prefix("storyos_session=")
            && (value.is_empty() || session_handle.replace(value).is_some())
        {
            return None;
        }
    }
    session_handle
}

fn validate_session_binding(
    config: &ServerConfig,
    binding: &ClientSessionBinding,
    headers: &HeaderMap,
) -> Result<(), ApiError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| authentication_required())?
        .as_secs();
    let host = headers.get("host").and_then(|value| value.to_str().ok());
    let origin = request_origin(headers);
    let valid = host == Some(binding.allowed_host.as_str())
        && origin == Some(binding.allowed_origin.as_str())
        && binding.session_generation == config.current_session_generation
        && binding.issued_at_unix_seconds <= now
        && now < binding.expires_at_unix_seconds
        && config.accepted_client_contract_revision.as_deref()
            == Some(binding.client_contract_revision.as_str())
        && config.accepted_security_policy_revision.as_deref()
            == Some(binding.security_policy_revision.as_str());
    if valid {
        Ok(())
    } else {
        Err(authentication_required())
    }
}

fn valid_uuid(value: &str) -> Result<(), ApiError> {
    Uuid::parse_str(value).map(|_| ()).map_err(|_| {
        problem(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "The request identity is invalid.",
        )
    })
}

fn project_reader(state: &ServerState) -> Result<PostgresProjectReader, ApiError> {
    state
        .config
        .database_url
        .as_ref()
        .map(PostgresProjectReader::new)
        .ok_or_else(|| {
            problem(
                StatusCode::SERVICE_UNAVAILABLE,
                "project_store_unavailable",
                "The Project store is unavailable.",
            )
        })
}

fn contract_scope(scope: &ApplicationScope) -> contracts::ProjectScope {
    contracts::ProjectScope {
        owner_user_id: scope.owner_user_id.as_ref().to_owned(),
        project_id: scope.project_id.as_ref().to_owned(),
    }
}

fn service_unavailable(_error: storyos_application::ProjectReadError) -> ApiError {
    problem(
        StatusCode::SERVICE_UNAVAILABLE,
        "project_store_unavailable",
        "The Project store is unavailable.",
    )
}

fn authentication_required() -> ApiError {
    problem(
        StatusCode::UNAUTHORIZED,
        "authentication_required",
        "Authentication is required.",
    )
}

fn request_site_refused() -> ApiError {
    problem(
        StatusCode::FORBIDDEN,
        "request_site_refused",
        "The request site is not allowed.",
    )
}

fn resource_unavailable() -> ApiError {
    problem(
        StatusCode::NOT_FOUND,
        "resource_unavailable",
        "The requested resource is unavailable.",
    )
}

fn problem(status: StatusCode, code: &str, message: &str) -> ApiError {
    (
        status,
        Json(contracts::StoryOSProblem {
            schema_id: "storyos.problem.v1".to_owned(),
            code: code.to_owned(),
            message: message.to_owned(),
        }),
    )
}

#[cfg(test)]
#[path = "client_session_binding_tests.rs"]
mod tests;
