use axum::http::StatusCode;

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
        proposal_target: None,
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
fn request_validation_refuses_a_structural_primitive_on_a_proposal_target() {
    let mut request = request();
    request.observed_ownership_partition = "mixed".to_owned();
    request.expected_proposal_head_revision_ids =
        vec!["018f0000-0000-7001-8000-000000000043".to_owned()];
    request.proposal_target = Some(contracts::AuthorEditProposalTarget {
        proposal_id: "018f0000-0000-7001-8000-000000000041".to_owned(),
        operation_id: "018f0000-0000-7001-8000-000000000042".to_owned(),
        revision_id: "018f0000-0000-7001-8000-000000000043".to_owned(),
        manuscript_block_id: "018f0000-0000-7001-8000-000000000044".to_owned(),
    });
    assert!(validate_request(&request).is_ok());

    request.author_edit_units[0].normalized_primitives =
        vec![contracts::AuthorEditPrimitive::SplitBlock {
            manuscript_block_id: "018f0000-0000-7001-8000-000000000044".to_owned(),
            offset: 4,
            new_manuscript_block_id: "018f0000-0000-7001-8000-000000000045".to_owned(),
        }];
    let Err(ApiError(status, problem)) = validate_request(&request) else {
        panic!("a structural primitive on a Proposal target must be refused");
    };
    assert_eq!(
        (status, problem.1.0.code.as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, "command_target_refused")
    );
}
