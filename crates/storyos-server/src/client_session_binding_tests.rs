use super::*;

const USER_ID: &str = "018f0000-0000-7001-8000-000000000001";
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

#[test]
fn current_client_session_binding_accepts_its_exact_request_site_and_identities() {
    let (config, binding, headers) = binding_fixture();

    assert!(validate_request_site(&config, &headers).is_ok());
    assert_eq!(session_cookie(&headers), Some("opaque-a"));
    assert!(validate_session_binding(&config, &binding, &headers).is_ok());
}

#[test]
fn stale_or_identity_mismatched_client_session_bindings_fail_closed() {
    let (config, binding, headers) = binding_fixture();

    let mut stale = binding.clone();
    stale.session_generation -= 1;
    assert_eq!(
        validate_session_binding(&config, &stale, &headers)
            .expect_err("stale generation must fail")
            .0,
        StatusCode::UNAUTHORIZED,
    );

    let mut expired = binding.clone();
    expired.expires_at_unix_seconds = current_time();
    assert_eq!(
        validate_session_binding(&config, &expired, &headers)
            .expect_err("expired binding must fail")
            .0,
        StatusCode::UNAUTHORIZED,
    );

    let mut wrong_client = binding.clone();
    wrong_client.client_contract_revision = "storyos.web-client.release-0.v1".to_owned();
    assert_eq!(
        validate_session_binding(&config, &wrong_client, &headers)
            .expect_err("wrong client contract must fail")
            .0,
        StatusCode::UNAUTHORIZED,
    );

    let mut wrong_policy = binding;
    wrong_policy.security_policy_revision = "storyos.web-security-policy.release-0.v1".to_owned();
    assert_eq!(
        validate_session_binding(&config, &wrong_policy, &headers)
            .expect_err("wrong security policy must fail")
            .0,
        StatusCode::UNAUTHORIZED,
    );
}

#[test]
fn missing_origin_and_ambiguous_cookie_fail_closed() {
    let (config, _binding, mut headers) = binding_fixture();
    headers.remove("origin");
    assert_eq!(
        validate_request_site(&config, &headers)
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
