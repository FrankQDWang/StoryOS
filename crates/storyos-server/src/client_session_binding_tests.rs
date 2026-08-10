use super::*;
use axum::body::Body;
use axum::http::Request;
use tower::ServiceExt as _;

const USER_ID: &str = "018f0000-0000-7001-8000-000000000001";
const PROJECT_ID: &str = "018f0000-0000-7001-8000-000000000002";
const HOST: &str = "127.0.0.1:3000";
const ORIGIN: &str = "http://127.0.0.1:3000";
const CLIENT_CONTRACT: &str = "storyos.web-client.release-1.v1";

fn current_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("test clock should be after the Unix epoch")
        .as_secs()
}

fn binding_fixture() -> (ServerConfig, ClientSessionBinding, HeaderMap) {
    let now = current_time();
    let binding = ClientSessionBinding {
        owner_user_id: UserId::new(USER_ID),
        allowed_host: HOST.to_owned(),
        allowed_origin: ORIGIN.to_owned(),
        session_generation: 7,
        issued_at_unix_seconds: now.saturating_sub(60),
        expires_at_unix_seconds: now + 60,
        client_contract_revision: CLIENT_CONTRACT.to_owned(),
        security_policy_revision: RELEASE_1_SECURITY_POLICY_REVISION.to_owned(),
    };
    let config = ServerConfig {
        session_bindings: HashMap::from([("opaque-a".to_owned(), binding.clone())]),
        current_session_generation: 7,
        accepted_client_contract_revision: Some(CLIENT_CONTRACT.to_owned()),
        accepted_security_policy_revision: Some(RELEASE_1_SECURITY_POLICY_REVISION.to_owned()),
        allowed_host: Some(HOST.to_owned()),
        allowed_origin: Some(ORIGIN.to_owned()),
        ..ServerConfig::default()
    };
    let mut headers = HeaderMap::new();
    headers.insert("host", HOST.parse().expect("test Host should be valid"));
    headers.insert(
        "origin",
        ORIGIN.parse().expect("test Origin should be valid"),
    );
    headers.insert(
        "cookie",
        "storyos_session=opaque-a"
            .parse()
            .expect("test Cookie should be valid"),
    );
    (config, binding, headers)
}

fn validate_fixture_binding(
    state: &ServerState,
    binding: &ClientSessionBinding,
    headers: &HeaderMap,
) -> Result<(), ApiError> {
    let request_origin = validate_request_site(
        state,
        headers,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    validate_session_binding(state, "opaque-a", binding, headers, &request_origin)
}

async fn public_read_status(
    configured_origin: &str,
    binding_origin: &str,
    header_origin: &str,
) -> StatusCode {
    let (mut config, mut binding, _headers) = binding_fixture();
    config.allowed_origin = Some(configured_origin.to_owned());
    binding.allowed_origin = binding_origin.to_owned();
    config
        .session_bindings
        .insert("opaque-a".to_owned(), binding);
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

#[test]
fn current_client_session_binding_accepts_its_exact_request_site_and_identities() {
    let (config, binding, headers) = binding_fixture();
    let state = ServerState::new(config);

    assert!(
        validate_request_site(
            &state,
            &headers,
            RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
        )
        .is_ok()
    );
    assert_eq!(session_cookie(&headers), Some("opaque-a"));
    assert!(validate_fixture_binding(&state, &binding, &headers).is_ok());
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

#[test]
fn stale_or_identity_mismatched_client_session_bindings_fail_closed() {
    let (config, binding, headers) = binding_fixture();
    let state = ServerState::new(config);

    let mut stale = binding.clone();
    stale.session_generation -= 1;
    assert_eq!(
        validate_fixture_binding(&state, &stale, &headers)
            .expect_err("stale generation must fail")
            .0,
        StatusCode::UNAUTHORIZED,
    );

    let mut expired = binding.clone();
    expired.expires_at_unix_seconds = current_time();
    assert_eq!(
        validate_fixture_binding(&state, &expired, &headers)
            .expect_err("expired binding must fail")
            .0,
        StatusCode::UNAUTHORIZED,
    );

    let mut wrong_client = binding.clone();
    wrong_client.client_contract_revision = "storyos.web-client.release-0.v1".to_owned();
    assert_eq!(
        validate_fixture_binding(&state, &wrong_client, &headers)
            .expect_err("wrong client contract must fail")
            .0,
        StatusCode::UNAUTHORIZED,
    );

    let mut wrong_policy = binding;
    wrong_policy.security_policy_revision = "storyos.web-security-policy.release-0.v1".to_owned();
    assert_eq!(
        validate_fixture_binding(&state, &wrong_policy, &headers)
            .expect_err("wrong security policy must fail")
            .0,
        StatusCode::UNAUTHORIZED,
    );
}

#[test]
fn missing_origin_and_ambiguous_cookie_fail_closed() {
    let (config, _binding, mut headers) = binding_fixture();
    let state = ServerState::new(config);
    headers.remove("origin");
    assert_eq!(
        validate_request_site(
            &state,
            &headers,
            RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
        )
        .expect_err("missing Origin must fail")
        .0,
        StatusCode::FORBIDDEN,
    );

    headers.insert(
        "origin",
        ORIGIN.parse().expect("test Origin should be valid"),
    );
    headers.insert(
        "cookie",
        "storyos_session=opaque-a; storyos_session=opaque-b"
            .parse()
            .expect("test Cookie should be valid"),
    );
    assert_eq!(session_cookie(&headers), None);
}

#[test]
fn invalid_allowed_origin_configuration_admits_no_protected_request() {
    let (mut config, _binding, headers) = binding_fixture();
    config.allowed_origin = Some(format!("{ORIGIN}/project"));
    let state = ServerState::new(config);

    assert_eq!(
        validate_request_site(
            &state,
            &headers,
            RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
        )
        .expect_err("invalid allowed Origin must fail")
        .0,
        StatusCode::FORBIDDEN,
    );
}

#[test]
fn invalid_binding_origin_configuration_fails_authentication() {
    let (mut config, mut binding, headers) = binding_fixture();
    binding.allowed_origin = format!("{ORIGIN}/project");
    config
        .session_bindings
        .insert("opaque-a".to_owned(), binding.clone());
    let state = ServerState::new(config);

    assert_eq!(
        validate_fixture_binding(&state, &binding, &headers)
            .expect_err("invalid binding Origin must fail")
            .0,
        StatusCode::UNAUTHORIZED,
    );
}

#[test]
fn multiple_cookie_headers_fail_closed() {
    let (_config, _binding, mut headers) = binding_fixture();
    headers.append(
        "cookie",
        "unrelated=value"
            .parse()
            .expect("test Cookie should be valid"),
    );

    assert_eq!(session_cookie(&headers), None);
}

#[test]
fn empty_or_mixed_empty_session_cookie_values_fail_closed() {
    let (_config, _binding, mut headers) = binding_fixture();
    headers.insert(
        "cookie",
        "storyos_session="
            .parse()
            .expect("test Cookie should be valid"),
    );
    assert_eq!(session_cookie(&headers), None);

    headers.insert(
        "cookie",
        "storyos_session=; storyos_session=opaque-a"
            .parse()
            .expect("test Cookie should be valid"),
    );
    assert_eq!(session_cookie(&headers), None);
}
