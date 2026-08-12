//! StoryOS public HTTP boundary.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing};
use storyos_adapter_postgres::PostgresProjectReader;
use storyos_application::{
    ChapterId, ProjectCommandChallengeError, ProjectId, ProjectScope as ApplicationScope, UserId,
    open_current_chapter, open_project,
};
use storyos_contracts as contracts;
use uuid::Uuid;

mod project_command_challenge;
mod request_origin;

use project_command_challenge::create_project_command_challenge;
use request_origin::{RequestOriginPolicy, TupleOrigin, request_origin};

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
    pub project_command_challenge_secret: Option<Vec<u8>>,
}

#[derive(Clone)]
struct ServerState {
    config: ServerConfig,
    allowed_origin: Option<TupleOrigin>,
    session_allowed_origins: HashMap<String, TupleOrigin>,
}

impl ServerState {
    fn new(config: ServerConfig) -> Self {
        let allowed_origin = config
            .allowed_origin
            .as_deref()
            .and_then(TupleOrigin::from_allowed_origin);
        let session_allowed_origins = config
            .session_bindings
            .iter()
            .filter_map(|(handle, binding)| {
                TupleOrigin::from_allowed_origin(&binding.allowed_origin)
                    .map(|origin| (handle.clone(), origin))
            })
            .collect();
        Self {
            config,
            allowed_origin,
            session_allowed_origins,
        }
    }
}

struct ApiError(
    StatusCode,
    Box<(HeaderMap, Json<contracts::StoryOSProblem>)>,
);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (headers, problem) = *self.1;
        (self.0, headers, problem).into_response()
    }
}

/// Build the protocol-only router used when no controlled Project store is configured.
pub fn router() -> Router {
    router_with_config(ServerConfig::default())
}

/// Build the one StoryOS Server router used by production and boundary tests.
pub fn router_with_config(config: ServerConfig) -> Router {
    let protocol_method = method_filter(contracts::GET_PROTOCOL_PROFILE_METHOD);
    let project_method = method_filter(contracts::GET_PROJECT_METHOD);
    let chapter_method = method_filter(contracts::GET_CHAPTER_METHOD);
    let challenge_method = method_filter(contracts::CREATE_PROJECT_COMMAND_CHALLENGE_METHOD);
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
        .route(
            contracts::CREATE_PROJECT_COMMAND_CHALLENGE_PATH,
            routing::on(challenge_method, create_project_command_challenge),
        )
        .with_state(Arc::new(ServerState::new(config)))
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
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
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
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
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
    origin_policy: RequestOriginPolicy,
) -> Result<ApplicationScope, ApiError> {
    valid_uuid(project_id)?;
    let request_origin = validate_request_site(state, headers, origin_policy)?;
    let session_handle = session_cookie(headers).ok_or_else(authentication_required)?;
    let binding = state
        .config
        .session_bindings
        .get(session_handle)
        .ok_or_else(authentication_required)?;
    validate_session_binding(state, session_handle, binding, headers, &request_origin)?;
    Ok(ApplicationScope::new(
        binding.owner_user_id.clone(),
        ProjectId::new(project_id),
    ))
}

fn validate_request_site(
    state: &ServerState,
    headers: &HeaderMap,
    origin_policy: RequestOriginPolicy,
) -> Result<TupleOrigin, ApiError> {
    let config = &state.config;
    let host = headers.get("host").and_then(|value| value.to_str().ok());
    if let Some(expected) = &config.allowed_host
        && host != Some(expected.as_str())
    {
        return Err(request_site_refused());
    }
    let origin = request_origin(headers, origin_policy).ok_or_else(request_site_refused)?;
    if config.allowed_origin.is_some() && state.allowed_origin.as_ref() != Some(&origin) {
        return Err(request_site_refused());
    }
    Ok(origin)
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
    state: &ServerState,
    session_handle: &str,
    binding: &ClientSessionBinding,
    headers: &HeaderMap,
    request_origin: &TupleOrigin,
) -> Result<(), ApiError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| authentication_required())?
        .as_secs();
    let host = headers.get("host").and_then(|value| value.to_str().ok());
    let valid = host == Some(binding.allowed_host.as_str())
        && state.session_allowed_origins.get(session_handle) == Some(request_origin)
        && binding.session_generation == state.config.current_session_generation
        && binding.issued_at_unix_seconds <= now
        && now < binding.expires_at_unix_seconds
        && state.config.accepted_client_contract_revision.as_deref()
            == Some(binding.client_contract_revision.as_str())
        && state.config.accepted_security_policy_revision.as_deref()
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

fn invalid_request() -> ApiError {
    problem(
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "The request is invalid.",
    )
}

fn validate_json_content_type(headers: &HeaderMap) -> Result<(), ApiError> {
    let mut values = headers.get_all(header::CONTENT_TYPE).iter();
    let value = values
        .next()
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim);
    if values.next().is_some()
        || !value.is_some_and(|value| {
            value.split_once('/').is_some_and(|(type_name, subtype)| {
                type_name.eq_ignore_ascii_case("application")
                    && (subtype.eq_ignore_ascii_case("json")
                        || subtype.rsplit_once('+').is_some_and(|(name, suffix)| {
                            !name.is_empty() && suffix.eq_ignore_ascii_case("json")
                        }))
            })
        })
    {
        return Err(problem(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_content_type",
            "The request must use one JSON content type.",
        ));
    }
    Ok(())
}

fn payload_too_large() -> ApiError {
    problem(
        StatusCode::PAYLOAD_TOO_LARGE,
        "payload_too_large",
        "The request exceeds the active Protocol Limit Profile.",
    )
}

fn invalid_request_shape() -> ApiError {
    problem(
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_request_shape",
        "The request does not match its closed schema.",
    )
}

fn command_target_refused() -> ApiError {
    problem(
        StatusCode::UNPROCESSABLE_ENTITY,
        "command_target_refused",
        "The command target is not in the closed Release 1 contract.",
    )
}

fn challenge_store_unavailable() -> ApiError {
    problem(
        StatusCode::SERVICE_UNAVAILABLE,
        "challenge_store_unavailable",
        "The command challenge store is unavailable.",
    )
}

fn challenge_error(error: ProjectCommandChallengeError) -> ApiError {
    match error {
        ProjectCommandChallengeError::BindingConflict => problem(
            StatusCode::CONFLICT,
            "idempotency_binding_conflict",
            "The idempotency key is already bound to another command.",
        ),
        ProjectCommandChallengeError::InvalidOrExpired => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "challenge_invalid",
            "The command challenge is invalid or expired.",
        ),
        ProjectCommandChallengeError::RateLimited {
            retry_after_seconds,
        } => {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::RETRY_AFTER,
                retry_after_seconds
                    .to_string()
                    .parse()
                    .expect("retry seconds must be a valid header value"),
            );
            ApiError(
                StatusCode::TOO_MANY_REQUESTS,
                Box::new((
                    headers,
                    Json(contracts::StoryOSProblem {
                        schema_id: "storyos.problem.v1".to_owned(),
                        code: "challenge_rate_limited".to_owned(),
                        message: "The command challenge rate limit is exceeded.".to_owned(),
                    }),
                )),
            )
        }
        ProjectCommandChallengeError::Unavailable(_) => challenge_store_unavailable(),
    }
}

fn problem(status: StatusCode, code: &str, message: &str) -> ApiError {
    ApiError(
        status,
        Box::new((
            HeaderMap::new(),
            Json(contracts::StoryOSProblem {
                schema_id: "storyos.problem.v1".to_owned(),
                code: code.to_owned(),
                message: message.to_owned(),
            }),
        )),
    )
}

#[cfg(test)]
#[path = "client_session_binding_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "project_command_challenge_tests.rs"]
mod project_command_challenge_tests;
