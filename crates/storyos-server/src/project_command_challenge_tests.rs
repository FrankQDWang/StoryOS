use super::*;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderValue, Request};
use tower::ServiceExt as _;

const USER_ID: &str = "018f0000-0000-7001-8000-000000000001";
const PROJECT_ID: &str = "018f0000-0000-7001-8000-000000000002";
const HOST: &str = "127.0.0.1:3000";
const ORIGIN: &str = "http://127.0.0.1:3000";
const CLIENT_CONTRACT: &str = "storyos.web-client.release-1.v1";

fn challenge_config() -> ServerConfig {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("test clock should be after the Unix epoch")
        .as_secs();
    ServerConfig {
        session_bindings: HashMap::from([(
            "opaque-a".to_owned(),
            ClientSessionBinding {
                owner_user_id: UserId::new(USER_ID),
                allowed_host: HOST.to_owned(),
                allowed_origin: ORIGIN.to_owned(),
                session_generation: 7,
                issued_at_unix_seconds: now.saturating_sub(60),
                expires_at_unix_seconds: now + 60,
                client_contract_revision: CLIENT_CONTRACT.to_owned(),
                security_policy_revision: RELEASE_1_SECURITY_POLICY_REVISION.to_owned(),
            },
        )]),
        current_session_generation: 7,
        accepted_client_contract_revision: Some(CLIENT_CONTRACT.to_owned()),
        accepted_security_policy_revision: Some(RELEASE_1_SECURITY_POLICY_REVISION.to_owned()),
        allowed_host: Some(HOST.to_owned()),
        allowed_origin: Some(ORIGIN.to_owned()),
        ..ServerConfig::default()
    }
}

fn challenge_request(origin: Option<&str>, referer: Option<&str>) -> Request<Body> {
    let body = serde_json::json!({
        "method": "PATCH",
        "route_template": "/api/v1/projects/{project_id}",
        "command_schema": "storyos.command.update-project.request.v1",
        "canonical_command_digest": {
            "algorithm": "sha256",
            "profile": "storyos.command.updateProject.jcs.v1",
            "value_hex_lowercase": "a".repeat(64)
        },
        "idempotency_key": "018f0000-0000-7001-8000-000000000004"
    });
    let mut builder = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/v1/projects/{PROJECT_ID}/anti-forgery-challenges"
        ))
        .header("host", HOST)
        .header("content-type", "application/json")
        .header("cookie", "storyos_session=opaque-a");
    if let Some(origin) = origin {
        builder = builder.header("origin", origin);
    }
    if let Some(referer) = referer {
        builder = builder.header("referer", referer);
    }
    builder
        .body(Body::from(body.to_string()))
        .expect("challenge request should be valid")
}

#[tokio::test]
async fn public_challenge_route_requires_origin_and_never_uses_referer() {
    let response = router_with_config(challenge_config())
        .oneshot(challenge_request(None, Some(&format!("{ORIGIN}/project"))))
        .await
        .expect("public request should complete");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn public_challenge_route_rejects_every_invalid_origin_class_before_storage() {
    let invalid_origins = [
        "",
        "not a URL",
        "http://user@127.0.0.1:3000",
        "http://127.0.0.1:3000/#fragment",
        "data:text/plain,opaque",
        "file:///tmp/storyos",
        "ftp://127.0.0.1:3000",
        "http:\\127.0.0.1:3000",
    ];
    for origin in invalid_origins {
        let response = router_with_config(challenge_config())
            .oneshot(challenge_request(
                Some(origin),
                Some(&format!("{ORIGIN}/project")),
            ))
            .await
            .expect("public request should complete");
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{origin:?}");
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(!body.contains(USER_ID));
        assert!(!body.contains(PROJECT_ID));
        assert!(!body.contains("nonce"));
    }

    let mut repeated = challenge_request(Some(ORIGIN), None);
    repeated
        .headers_mut()
        .append("origin", HeaderValue::from_static(ORIGIN));
    let repeated_response = router_with_config(challenge_config())
        .oneshot(repeated)
        .await
        .unwrap();
    assert_eq!(repeated_response.status(), StatusCode::FORBIDDEN);

    let mut non_utf8 = challenge_request(Some(ORIGIN), None);
    non_utf8.headers_mut().insert(
        "origin",
        HeaderValue::from_bytes(b"http://127.0.0.1:3000/\xff").unwrap(),
    );
    let non_utf8_response = router_with_config(challenge_config())
        .oneshot(non_utf8)
        .await
        .unwrap();
    assert_eq!(non_utf8_response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn public_challenge_route_reaches_the_project_store_after_security_validation() {
    let response = router_with_config(challenge_config())
        .oneshot(challenge_request(Some(ORIGIN), None))
        .await
        .expect("public request should complete");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn rate_refusal_has_a_bounded_retry_time_without_binding_disclosure() {
    let response = challenge_error(ProjectCommandChallengeError::RateLimited {
        retry_after_seconds: 37,
    })
    .into_response();

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers().get("retry-after").unwrap(), "37");
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
        serde_json::json!({
            "schema_id": "storyos.problem.v1",
            "code": "challenge_rate_limited",
            "message": "The command challenge rate limit is exceeded."
        })
    );
}

#[tokio::test]
async fn public_challenge_route_enforces_the_absolute_json_body_ceiling() {
    let oversized_body = format!("{{\"padding\":\"{}\"}}", "a".repeat(1024 * 1024));
    let request = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/v1/projects/{PROJECT_ID}/anti-forgery-challenges"
        ))
        .header("host", HOST)
        .header("origin", ORIGIN)
        .header("content-type", "application/json")
        .header("cookie", "storyos_session=opaque-a")
        .body(Body::from(oversized_body.clone()))
        .expect("challenge request should be valid");
    let response = router_with_config(challenge_config())
        .oneshot(request)
        .await
        .expect("public request should complete");

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let hostile_request = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/v1/projects/{PROJECT_ID}/anti-forgery-challenges"
        ))
        .header("host", HOST)
        .header("origin", "https://foreign.example")
        .header("content-type", "application/json")
        .header("cookie", "storyos_session=opaque-a")
        .body(Body::from(oversized_body))
        .expect("challenge request should be valid");
    let hostile_response = router_with_config(challenge_config())
        .oneshot(hostile_request)
        .await
        .expect("public request should complete");
    assert_eq!(hostile_response.status(), StatusCode::FORBIDDEN);
}
