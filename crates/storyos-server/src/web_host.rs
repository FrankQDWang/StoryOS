use std::sync::Arc;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header};
use axum::response::Response;
use uuid::Uuid;

use crate::{
    CLIENT_SESSION_BINDING_LIFETIME_SECS, PresentedSessionCookie, ServerConfig, ServerState,
    TrustedLocalSessionBootstrap, WebAssetSet, api_router, presented_session_cookie,
};

const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; font-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'; worker-src 'none'; require-trusted-types-for 'script'; trusted-types 'none'";

#[derive(Clone)]
struct WebHostState {
    assets: Arc<WebAssetSet>,
    server: Arc<ServerState>,
}

/// Add the validated Web surface without replacing an existing API route or method gate.
pub fn router_with_web(config: ServerConfig, assets: WebAssetSet) -> Router {
    let server = Arc::new(ServerState::new(config));
    let web = Router::new()
        .fallback(web_response)
        .with_state(WebHostState {
            assets: Arc::new(assets),
            server: server.clone(),
        });
    api_router(server).fallback_service(web)
}

async fn web_response(
    State(state): State<WebHostState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    let path = uri.path();
    let project_path = path.strip_suffix('/').unwrap_or(path);
    let is_project = project_path
        .get(..10)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("/projects/"))
        && project_path
            .get(10..)
            .is_some_and(|id| id.len() == 36 && Uuid::try_parse(id).is_ok());
    let resource = if path == "/" || is_project {
        "index.html"
    } else {
        path.strip_prefix('/').unwrap_or_default()
    };
    let mut response = Response::builder()
        .header("content-security-policy", CSP)
        .header("x-content-type-options", "nosniff")
        // Sensitive safe reads retain the existing same-origin Referer fallback.
        .header("referrer-policy", "same-origin")
        .header("cross-origin-resource-policy", "same-origin");
    let (status, bytes) = match state.assets.resource(resource) {
        None => (StatusCode::NOT_FOUND, Bytes::new()),
        Some(_) if method != Method::GET && method != Method::HEAD => {
            response = response.header(header::ALLOW, "GET, HEAD");
            (StatusCode::METHOD_NOT_ALLOWED, Bytes::new())
        }
        Some((record, bytes)) => {
            response = response
                .header(header::CONTENT_TYPE, &record.mime_type)
                .header(header::CONTENT_LENGTH, record.byte_length)
                .header(
                    header::CACHE_CONTROL,
                    if resource == "index.html" {
                        "no-store"
                    } else {
                        "public, max-age=31536000, immutable"
                    },
                );
            (
                StatusCode::OK,
                if method == Method::HEAD {
                    Bytes::new()
                } else {
                    bytes
                },
            )
        }
    };
    if status != StatusCode::OK {
        response = response.header(header::CACHE_CONTROL, "no-store");
    }
    if let Some(cookie) =
        trusted_local_session_cookie(&state, method, path == "/" || is_project, status, &headers)
    {
        response = response.header(header::SET_COOKIE, cookie);
    }
    response
        .status(status)
        .body(Body::from(bytes))
        .expect("validated Web response headers")
}

fn trusted_local_session_cookie(
    state: &WebHostState,
    method: Method,
    document_route: bool,
    status: StatusCode,
    headers: &HeaderMap,
) -> Option<HeaderValue> {
    if method != Method::GET
        || !document_route
        || status != StatusCode::OK
        || state.server.config.trusted_local_session_bootstrap
            != TrustedLocalSessionBootstrap::IssueTheSingleConfiguredHandle
    {
        return None;
    }
    let allowed_host = state.server.config.allowed_host.as_deref()?;
    let host = headers.get("host").and_then(|value| value.to_str().ok())?;
    if host != allowed_host {
        return None;
    }
    let handle = {
        if state.server.config.session_bindings.len() != 1 {
            return None;
        }
        state.server.config.session_bindings.keys().next()?.clone()
    };
    match presented_session_cookie(headers) {
        PresentedSessionCookie::Absent => Some(session_set_cookie(&handle)?),
        PresentedSessionCookie::Handle(existing) if existing == handle => {
            if state.server.refresh_client_session_binding(&handle) {
                session_set_cookie(&handle)
            } else {
                None
            }
        }
        PresentedSessionCookie::Handle(_) | PresentedSessionCookie::Invalid => None,
    }
}

fn session_set_cookie(handle: &str) -> Option<HeaderValue> {
    HeaderValue::from_str(&format!(
        "storyos_session={handle}; HttpOnly; SameSite=Strict; Path=/; Max-Age={CLIENT_SESSION_BINDING_LIFETIME_SECS}"
    ))
    .ok()
}

#[cfg(test)]
#[path = "web_host_tests.rs"]
mod tests;
