use std::sync::Arc;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{Method, StatusCode, Uri, header};
use axum::response::Response;
use uuid::Uuid;

use crate::{ServerConfig, WebAssetSet, router_with_config};

const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; font-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'; worker-src 'none'; require-trusted-types-for 'script'; trusted-types 'none'";

/// Add the validated Web surface without replacing an existing API route or method gate.
pub fn router_with_web(config: ServerConfig, assets: WebAssetSet) -> Router {
    let web = Router::new()
        .fallback(web_response)
        .with_state(Arc::new(assets));
    router_with_config(config).fallback_service(web)
}

async fn web_response(
    State(assets): State<Arc<WebAssetSet>>,
    method: Method,
    uri: Uri,
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
    let (status, bytes) = match assets.resource(resource) {
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
    response
        .status(status)
        .body(Body::from(bytes))
        .expect("validated Web response headers")
}
