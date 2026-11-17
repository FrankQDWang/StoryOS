use super::*;
use axum::body::Body;
use axum::http::Request;
use tower::ServiceExt as _;

const USER_ID: &str = "018f0000-0000-7001-8000-000000000001";
const PROJECT_ID: &str = "018f0000-0000-7001-8000-000000000002";
const HOST: &str = "127.0.0.1:3000";
const CLIENT_CONTRACT: &str = "storyos.web-client.release-1.v1";

async fn public_read_status(
    configured_origin: &str,
    binding_origin: &str,
    header_origin: &str,
) -> StatusCode {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("test clock should be after the Unix epoch")
        .as_secs();
    let binding = ClientSessionBinding {
        owner_user_id: UserId::new(USER_ID),
        allowed_host: HOST.to_owned(),
        allowed_origin: binding_origin.to_owned(),
        session_generation: 7,
        issued_at_unix_seconds: now.saturating_sub(60),
        expires_at_unix_seconds: now + 60,
        client_contract_revision: CLIENT_CONTRACT.to_owned(),
        security_policy_revision: RELEASE_1_SECURITY_POLICY_REVISION.to_owned(),
    };
    let config = ServerConfig {
        session_bindings: HashMap::from([("opaque-a".to_owned(), binding)]),
        current_session_generation: 7,
        accepted_client_contract_revision: Some(CLIENT_CONTRACT.to_owned()),
        accepted_security_policy_revision: Some(RELEASE_1_SECURITY_POLICY_REVISION.to_owned()),
        allowed_host: Some(HOST.to_owned()),
        allowed_origin: Some(configured_origin.to_owned()),
        ..ServerConfig::default()
    };
    let request = Request::builder()
        .uri(format!("/api/v1/projects/{PROJECT_ID}"))
        .header("host", HOST)
        .header("origin", header_origin)
        .header("cookie", "storyos_session=opaque-a")
        .body(Body::empty())
        .expect("public request should be valid");

    router_with_config(config)
        .oneshot(request)
        .await
        .expect("public request should complete")
        .status()
}

#[tokio::test]
async fn public_read_admission_uses_normalized_config_binding_and_header_origins() {
    for (configured, binding, presented) in [
        (
            "https://example.com",
            "https://example.com:443",
            "HTTPS://EXAMPLE.COM",
        ),
        (
            "https://bücher.example",
            "https://xn--bcher-kva.example",
            "https://xn--bcher-kva.example:443",
        ),
        (
            "http://127.0.0.1",
            "http://127.0.0.1:80",
            "http://127.0.0.1",
        ),
        (
            "http://[0:0:0:0:0:0:0:1]",
            "http://[::1]:80",
            "http://[::1]",
        ),
    ] {
        assert_eq!(
            public_read_status(configured, binding, presented).await,
            StatusCode::SERVICE_UNAVAILABLE,
            "{presented}",
        );
    }
}
