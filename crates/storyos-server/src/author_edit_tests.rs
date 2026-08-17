use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt as _;

use super::*;

fn request() -> contracts::ApplyAuthorEditRequest {
    contracts::ApplyAuthorEditRequest {
        command_schema: contracts::APPLY_AUTHOR_EDIT_REQUEST_SCHEMA_ID.to_owned(),
        client_contract_revision: "client".to_owned(),
        security_policy_revision: "security".to_owned(),
        correlation_id: "018f0000-0000-7001-8000-000000000030".to_owned(),
        editor_session_id: "018f0000-0000-7001-8000-000000000021".to_owned(),
        writer_generation: "1".to_owned(),
        chapter_id: "018f0000-0000-7001-8000-000000000003".to_owned(),
        expected_authoritative_revision_id: "018f0000-0000-7001-8000-000000000004".to_owned(),
        expected_proposal_head_revision_ids: Vec::new(),
        target_refs: vec!["manuscript:018f0000-0000-7001-8000-000000000003".to_owned()],
        observed_ownership_partition: "authoritative".to_owned(),
        editor_contract_revision: contracts::EDITOR_CONTRACT_REVISION.to_owned(),
        undo_group_id: "018f0000-0000-7001-8000-000000000031".to_owned(),
        completed_intent_record_id: "018f0000-0000-7001-8000-000000000032".to_owned(),
        local_intent_sequence: "1".to_owned(),
        author_edit_units: vec![contracts::AuthorEditUnit {
            normalized_primitives: vec![contracts::AuthorEditPrimitive::ReplaceSelection {
                from: 4,
                to: 4,
                text: "!".to_owned(),
            }],
            selection_snapshot: contracts::SelectionSnapshot {
                coordinate_profile: storyos_core::UTF16_COORDINATE_PROFILE.to_owned(),
                from: 4,
                to: 4,
            },
        }],
    }
}

#[test]
fn request_validation_rejects_a_foreign_target() {
    let mut request = request();
    request.target_refs = vec!["manuscript:foreign".to_owned()];
    assert!(validate_request(&request).is_err());
}

#[test]
fn request_validation_accepts_a_bounded_ordered_batch() {
    let mut request = request();
    request.author_edit_units.push(contracts::AuthorEditUnit {
        normalized_primitives: vec![contracts::AuthorEditPrimitive::ReplaceSelection {
            from: 5,
            to: 5,
            text: "?".to_owned(),
        }],
        selection_snapshot: contracts::SelectionSnapshot {
            coordinate_profile: storyos_core::UTF16_COORDINATE_PROFILE.to_owned(),
            from: 5,
            to: 5,
        },
    });

    assert!(validate_request(&request).is_ok());
}

#[tokio::test]
async fn apply_author_edit_rejects_referer_without_origin() {
    let response = router()
        .oneshot(
            Request::post(
                "/api/v1/projects/018f0000-0000-7001-8000-000000000002/manuscript/author-edits",
            )
            .header(header::REFERER, "https://example.com/projects/one")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("{}"))
            .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
