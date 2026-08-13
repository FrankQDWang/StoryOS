use axum::body::Body;
use axum::http::Request;
use tower::ServiceExt as _;

use super::*;

#[tokio::test]
async fn editor_session_routes_exist_and_state_change_never_uses_referer() {
    let create = router()
        .oneshot(
            Request::post("/api/v1/projects/018f0000-0000-7001-8000-000000000002/editor-sessions")
                .header("referer", "http://storyos.test/project")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::FORBIDDEN);

    let read = router()
        .oneshot(
            Request::get("/api/v1/projects/018f0000-0000-7001-8000-000000000002/editor-sessions/018f0000-0000-7001-8000-000000000020")
                .header("origin", "http://storyos.test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(read.status(), StatusCode::UNAUTHORIZED);
}
