use std::collections::BTreeSet;

use crate::project_command_targets::PROJECT_COMMAND_TARGETS;
use crate::{
    APPLY_AUTHOR_EDIT_METHOD, APPLY_AUTHOR_EDIT_PATH, APPLY_AUTHOR_EDIT_REQUEST_SCHEMA_ID,
    ApplyAuthorEditRequest, AuthorEditPrimitive, AuthorEditUnit, EDITOR_CONTRACT_REVISION,
    SelectionSnapshot,
};

#[test]
fn challenge_targets_equal_the_project_scoped_release_1_command_catalog() {
    let catalog: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            super::super::repository_root()
                .join("docs/foundation/versioned-protocol-release-1-route-catalog.json"),
        )
        .expect("Release 1 route catalog must exist"),
    )
    .expect("Release 1 route catalog must be valid JSON");
    let catalog_targets = catalog["operations"]
        .as_array()
        .expect("operations must be an array")
        .iter()
        .filter(|operation| {
            operation["kind"] == "command"
                && operation["path"]
                    .as_str()
                    .is_some_and(|path| path.contains("{project_id}"))
        })
        .map(|operation| {
            (
                operation["operation_id"].as_str().unwrap(),
                operation["method"].as_str().unwrap(),
                operation["path"].as_str().unwrap(),
                operation["schemas"]["request"].as_str().unwrap(),
            )
        })
        .collect::<BTreeSet<_>>();
    let implementation_targets = PROJECT_COMMAND_TARGETS
        .iter()
        .map(|target| {
            (
                target.command_kind,
                target.method,
                target.route_template,
                target.command_schema,
            )
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(implementation_targets, catalog_targets);
}

#[test]
fn apply_author_edit_contract_has_ordered_replace_selection_units() {
    let request = ApplyAuthorEditRequest {
        command_schema: APPLY_AUTHOR_EDIT_REQUEST_SCHEMA_ID.to_owned(),
        client_contract_revision: "storyos.web-client.release-1.v1".to_owned(),
        security_policy_revision: "storyos.web-security-policy.release-1.v1".to_owned(),
        correlation_id: "018f0000-0000-7001-8000-000000000030".to_owned(),
        editor_session_id: "018f0000-0000-7001-8000-000000000021".to_owned(),
        writer_generation: "1".to_owned(),
        chapter_id: "018f0000-0000-7001-8000-000000000003".to_owned(),
        expected_authoritative_revision_id: "018f0000-0000-7001-8000-000000000004".to_owned(),
        expected_proposal_head_revision_ids: Vec::new(),
        target_refs: vec!["manuscript:018f0000-0000-7001-8000-000000000003".to_owned()],
        observed_ownership_partition: "authoritative".to_owned(),
        editor_contract_revision: EDITOR_CONTRACT_REVISION.to_owned(),
        undo_group_id: "018f0000-0000-7001-8000-000000000031".to_owned(),
        completed_intent_record_id: "018f0000-0000-7001-8000-000000000032".to_owned(),
        local_intent_sequence: "1".to_owned(),
        author_edit_units: vec![
            AuthorEditUnit {
                normalized_primitives: vec![AuthorEditPrimitive::ReplaceSelection {
                    from: 4,
                    to: 4,
                    text: "!".to_owned(),
                }],
                selection_snapshot: SelectionSnapshot {
                    coordinate_profile: "storyos.editor.utf16-code-unit.v1".to_owned(),
                    from: 4,
                    to: 4,
                },
            },
            AuthorEditUnit {
                normalized_primitives: vec![AuthorEditPrimitive::ReplaceSelection {
                    from: 5,
                    to: 5,
                    text: "?".to_owned(),
                }],
                selection_snapshot: SelectionSnapshot {
                    coordinate_profile: "storyos.editor.utf16-code-unit.v1".to_owned(),
                    from: 5,
                    to: 5,
                },
            },
        ],
    };

    assert_eq!(APPLY_AUTHOR_EDIT_METHOD, "POST");
    assert_eq!(
        APPLY_AUTHOR_EDIT_PATH,
        "/api/v1/projects/{project_id}/manuscript/author-edits"
    );
    assert_eq!(
        serde_json::to_value(&request).expect("request serializes")["author_edit_units"],
        serde_json::json!([
            {
                "normalized_primitives": [
                    {"kind": "replace_selection", "from": 4, "to": 4, "text": "!"}
                ],
                "selection_snapshot": {
                    "coordinate_profile": "storyos.editor.utf16-code-unit.v1",
                    "from": 4,
                    "to": 4
                }
            },
            {
                "normalized_primitives": [
                    {"kind": "replace_selection", "from": 5, "to": 5, "text": "?"}
                ],
                "selection_snapshot": {
                    "coordinate_profile": "storyos.editor.utf16-code-unit.v1",
                    "from": 5,
                    "to": 5
                }
            }
        ])
    );
}
