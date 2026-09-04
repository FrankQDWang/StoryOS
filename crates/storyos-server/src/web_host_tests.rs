use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use storyos_application::UserId;
use storyos_contracts::write_web_asset_manifest;
use tower::ServiceExt as _;
use uuid::Uuid;

use super::*;
use crate::{
    CLIENT_SESSION_BINDING_LIFETIME_SECS, ClientSessionBinding, RELEASE_1_SECURITY_POLICY_REVISION,
    ServerConfig, TrustedLocalSessionBootstrap,
};

const SCRIPT: &str = "assets/index-12345678.js";
const INDEX: &[u8] =
    b"<!doctype html><script type=\"module\" src=\"/assets/index-12345678.js\"></script>";
const USER_ID: &str = "018f0000-0000-7001-8000-000000000001";
const PROJECT_ID: &str = "018f0000-0000-7001-8000-000000000002";
const HOST: &str = "127.0.0.1:3000";
const ORIGIN: &str = "http://127.0.0.1:3000";
const HANDLE: &str = "session-a";
const CLIENT_CONTRACT: &str = "storyos.web-client.release-1.v1";

fn set_cookie_value() -> String {
    format!(
        "storyos_session={HANDLE}; HttpOnly; SameSite=Strict; Path=/; Max-Age={CLIENT_SESSION_BINDING_LIFETIME_SECS}"
    )
}

struct Fixture {
    root: PathBuf,
    digest: String,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("storyos-web-host-{}", Uuid::now_v7()));
        fs::create_dir_all(root.join("assets")).unwrap();
        fs::write(root.join("index.html"), INDEX).unwrap();
        fs::write(root.join(SCRIPT), b"document.title = 'StoryOS';").unwrap();
        let digest =
            write_web_asset_manifest(&root, &"a".repeat(/*n*/ 40), &"b".repeat(/*n*/ 40)).unwrap();
        Self { root, digest }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("test clock should be after the Unix epoch")
        .as_secs()
}

fn binding(issued_at_unix_seconds: u64, expires_at_unix_seconds: u64) -> ClientSessionBinding {
    ClientSessionBinding {
        owner_user_id: UserId::new(USER_ID),
        allowed_host: HOST.to_owned(),
        allowed_origin: ORIGIN.to_owned(),
        session_generation: 1,
        issued_at_unix_seconds,
        expires_at_unix_seconds,
        client_contract_revision: CLIENT_CONTRACT.to_owned(),
        security_policy_revision: RELEASE_1_SECURITY_POLICY_REVISION.to_owned(),
    }
}

fn config(bootstrap: TrustedLocalSessionBootstrap, expires_at_unix_seconds: u64) -> ServerConfig {
    let issued_at_unix_seconds = now_secs().saturating_sub(60);
    ServerConfig {
        session_bindings: HashMap::from([(
            HANDLE.to_owned(),
            binding(issued_at_unix_seconds, expires_at_unix_seconds),
        )]),
        current_session_generation: 1,
        accepted_client_contract_revision: Some(CLIENT_CONTRACT.to_owned()),
        accepted_security_policy_revision: Some(RELEASE_1_SECURITY_POLICY_REVISION.to_owned()),
        allowed_host: Some(HOST.to_owned()),
        allowed_origin: Some(ORIGIN.to_owned()),
        trusted_local_session_bootstrap: bootstrap,
        ..ServerConfig::default()
    }
}

async fn send(
    router: Router,
    uri: &str,
    host: &str,
    origin: Option<&str>,
    cookie: Option<&str>,
) -> axum::http::Response<Body> {
    let mut request = Request::builder().uri(uri).header("host", host);
    if let Some(origin) = origin {
        request = request.header("origin", origin);
    }
    if let Some(cookie) = cookie {
        request = request.header("cookie", cookie);
    }
    router
        .oneshot(
            request
                .body(Body::empty())
                .expect("test request should be valid"),
        )
        .await
        .expect("test request should complete")
}

fn set_cookie(response: &axum::http::Response<Body>) -> Option<&str> {
    let mut values = response.headers().get_all(header::SET_COOKIE).iter();
    let value = values.next()?.to_str().ok();
    if values.next().is_some() {
        panic!("Trusted Local Session Bootstrap issues exactly one Set-Cookie");
    }
    value
}

#[tokio::test]
async fn html_document_get_issues_the_configured_session_cookie() {
    let fixture = Fixture::new();
    let assets = WebAssetSet::load_bound(&fixture.root, &fixture.digest).unwrap();
    let router = router_with_web(
        config(
            TrustedLocalSessionBootstrap::IssueTheSingleConfiguredHandle,
            now_secs() + CLIENT_SESSION_BINDING_LIFETIME_SECS,
        ),
        assets,
    );

    for uri in ["/", &format!("/projects/{PROJECT_ID}")] {
        let response = send(
            router.clone(),
            uri,
            HOST,
            /*origin*/ None,
            /*cookie*/ None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let cookie = set_cookie(&response).expect("document GET issues one cookie");
        assert_eq!(cookie, set_cookie_value());
        assert!(!cookie.contains("Secure"));
        assert!(!cookie.to_ascii_lowercase().contains("domain="));
    }
}

#[tokio::test]
async fn static_index_html_path_does_not_issue_a_session() {
    let fixture = Fixture::new();
    let assets = WebAssetSet::load_bound(&fixture.root, &fixture.digest).unwrap();
    let router = router_with_web(
        config(
            TrustedLocalSessionBootstrap::IssueTheSingleConfiguredHandle,
            now_secs() + CLIENT_SESSION_BINDING_LIFETIME_SECS,
        ),
        assets,
    );
    let static_index = send(
        router.clone(),
        "/index.html",
        HOST,
        /*origin*/ None,
        /*cookie*/ None,
    )
    .await;
    assert_eq!(static_index.status(), StatusCode::OK);
    assert_eq!(set_cookie(&static_index), None);

    let document = send(
        router, "/", HOST, /*origin*/ None, /*cookie*/ None,
    )
    .await;
    assert_eq!(set_cookie(&document), Some(set_cookie_value().as_str()));
}

#[tokio::test]
async fn protocol_assets_and_failures_do_not_issue_a_session() {
    let fixture = Fixture::new();
    let assets = WebAssetSet::load_bound(&fixture.root, &fixture.digest).unwrap();
    let router = router_with_web(
        config(
            TrustedLocalSessionBootstrap::IssueTheSingleConfiguredHandle,
            now_secs() + CLIENT_SESSION_BINDING_LIFETIME_SECS,
        ),
        assets,
    );

    let protocol = send(
        router.clone(),
        "/api/v1/protocol",
        HOST,
        /*origin*/ None,
        /*cookie*/ None,
    )
    .await;
    assert_eq!(protocol.status(), StatusCode::OK);
    assert_eq!(set_cookie(&protocol), None);

    let mismatched_protocol = send(
        router.clone(),
        "/api/v1/protocol",
        HOST,
        /*origin*/ None,
        Some("storyos_session=other"),
    )
    .await;
    assert_eq!(mismatched_protocol.status(), StatusCode::OK);
    assert_eq!(set_cookie(&mismatched_protocol), None);

    let asset = send(
        router.clone(),
        &format!("/{SCRIPT}"),
        HOST,
        /*origin*/ None,
        /*cookie*/ None,
    )
    .await;
    assert_eq!(asset.status(), StatusCode::OK);
    assert_eq!(set_cookie(&asset), None);

    let missing = send(
        router.clone(),
        "/missing",
        HOST,
        /*origin*/ None,
        /*cookie*/ None,
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(set_cookie(&missing), None);

    let project_api = send(
        router,
        &format!("/api/v1/projects/{PROJECT_ID}"),
        HOST,
        Some(ORIGIN),
        /*cookie*/ None,
    )
    .await;
    assert_eq!(project_api.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(set_cookie(&project_api), None);
}

#[tokio::test]
async fn issuance_requires_the_printed_host_and_does_not_need_origin() {
    let fixture = Fixture::new();
    let assets = WebAssetSet::load_bound(&fixture.root, &fixture.digest).unwrap();
    let router = router_with_web(
        config(
            TrustedLocalSessionBootstrap::IssueTheSingleConfiguredHandle,
            now_secs() + CLIENT_SESSION_BINDING_LIFETIME_SECS,
        ),
        assets,
    );

    let foreign = send(
        router.clone(),
        "/",
        "example.com",
        /*origin*/ None,
        /*cookie*/ None,
    )
    .await;
    assert_eq!(foreign.status(), StatusCode::OK);
    assert_eq!(set_cookie(&foreign), None);

    let issued = send(
        router, "/", HOST, /*origin*/ None, /*cookie*/ None,
    )
    .await;
    assert_eq!(set_cookie(&issued), Some(set_cookie_value().as_str()));
}

#[tokio::test]
async fn matching_cookie_refreshes_an_expired_binding() {
    let fixture = Fixture::new();
    let assets = WebAssetSet::load_bound(&fixture.root, &fixture.digest).unwrap();
    let router = router_with_web(
        config(
            TrustedLocalSessionBootstrap::IssueTheSingleConfiguredHandle,
            now_secs().saturating_sub(1),
        ),
        assets,
    );

    let expired = send(
        router.clone(),
        &format!("/api/v1/projects/{PROJECT_ID}"),
        HOST,
        Some(ORIGIN),
        Some("storyos_session=session-a"),
    )
    .await;
    assert_eq!(expired.status(), StatusCode::UNAUTHORIZED);

    let html = send(
        router.clone(),
        "/",
        HOST,
        /*origin*/ None,
        Some("storyos_session=session-a"),
    )
    .await;
    assert_eq!(html.status(), StatusCode::OK);
    assert_eq!(set_cookie(&html), Some(set_cookie_value().as_str()));

    let refreshed = send(
        router,
        &format!("/api/v1/projects/{PROJECT_ID}"),
        HOST,
        Some(ORIGIN),
        Some("storyos_session=session-a"),
    )
    .await;
    assert_eq!(refreshed.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(set_cookie(&refreshed), None);
}

#[tokio::test]
async fn mismatched_cookie_is_not_overwritten_and_does_not_authorize() {
    let fixture = Fixture::new();
    let assets = WebAssetSet::load_bound(&fixture.root, &fixture.digest).unwrap();
    let router = router_with_web(
        config(
            TrustedLocalSessionBootstrap::IssueTheSingleConfiguredHandle,
            now_secs() + CLIENT_SESSION_BINDING_LIFETIME_SECS,
        ),
        assets,
    );

    for cookie in [
        "storyos_session=other",
        "storyos_session=",
        "storyos_session=session-a; storyos_session=other",
    ] {
        let html = send(
            router.clone(),
            "/",
            HOST,
            /*origin*/ None,
            Some(cookie),
        )
        .await;
        assert_eq!(html.status(), StatusCode::OK, "{cookie}");
        assert_eq!(set_cookie(&html), None, "{cookie}");
        let api = send(
            router.clone(),
            &format!("/api/v1/projects/{PROJECT_ID}"),
            HOST,
            Some(ORIGIN),
            Some(cookie),
        )
        .await;
        assert_eq!(api.status(), StatusCode::UNAUTHORIZED, "{cookie}");
    }
}

#[tokio::test]
async fn disabled_bootstrap_does_not_issue_a_cookie() {
    let fixture = Fixture::new();
    let assets = WebAssetSet::load_bound(&fixture.root, &fixture.digest).unwrap();
    let router = router_with_web(
        config(
            TrustedLocalSessionBootstrap::Disabled,
            now_secs() + CLIENT_SESSION_BINDING_LIFETIME_SECS,
        ),
        assets,
    );
    let response = send(
        router, "/", HOST, /*origin*/ None, /*cookie*/ None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(set_cookie(&response), None);
}
