use serde_json::Value;

use super::{
    BASELINE_COMMIT, BASELINE_TREE, CONTRACT_REVISION, CrosswalkError, GENERATED_CROSSWALK_PATH,
    ISSUE_BODY_SHA256, OPERATION_IDS, ROUTE_CATALOG_PATH, generate_crosswalk, read_baseline_file,
    read_baseline_json, repository_root, validate_operations, verify_baseline_tree,
};

fn route_catalog() -> Value {
    read_baseline_json(repository_root(), ROUTE_CATALOG_PATH).expect("route catalog should load")
}

#[test]
fn generated_crosswalk_matches_the_locked_contract_as_a_whole() {
    let generated: Value = serde_json::from_slice(
        &generate_crosswalk(repository_root()).expect("crosswalk generation should succeed"),
    )
    .expect("generated crosswalk should be JSON");
    assert_eq!(
        generated["declarations"]["execution_contract"],
        serde_json::json!({
            "issue": "https://github.com/FrankQDWang/StoryOS/issues/100",
            "revision": CONTRACT_REVISION,
            "baseline_commit": BASELINE_COMMIT,
            "baseline_tree": BASELINE_TREE,
            "issue_body_sha256": ISSUE_BODY_SHA256,
        })
    );
    assert_eq!(
        generated["verifications"]["operations"]
            .as_array()
            .map(Vec::len),
        Some(OPERATION_IDS.len())
    );
    assert_eq!(
        generated["verifications"]["baseline"],
        serde_json::json!({"commit": BASELINE_COMMIT, "tree": BASELINE_TREE})
    );
    assert_eq!(
        generated["claim_ceiling"],
        "contract-foundation-only; no S1 requirement or stage acceptance"
    );
}

#[test]
fn checked_in_crosswalk_matches_fresh_generation() {
    let actual = std::fs::read(repository_root().join(GENERATED_CROSSWALK_PATH))
        .expect("checked-in crosswalk should exist");
    let expected =
        generate_crosswalk(repository_root()).expect("crosswalk generation should succeed");
    assert_eq!(actual, expected);
}

#[test]
fn missing_stage1_operation_is_rejected() {
    let mut catalog = route_catalog();
    catalog["operations"]
        .as_array_mut()
        .expect("operations should be an array")
        .retain(|operation| operation["operation_id"] != "applyAuthorEdit");
    assert!(matches!(
        validate_operations(&catalog),
        Err(CrosswalkError::Invalid(message))
            if message == "missing Stage 1 operation applyAuthorEdit"
    ));
}

#[test]
fn missing_project_scope_precondition_is_rejected() {
    let mut catalog = route_catalog();
    let operation = catalog["operations"]
        .as_array_mut()
        .expect("operations should be an array")
        .iter_mut()
        .find(|operation| operation["operation_id"] == "applyAuthorEdit")
        .expect("applyAuthorEdit should exist");
    operation["preconditions"]
        .as_array_mut()
        .expect("preconditions should be an array")
        .retain(|value| value != "server_derived_project_scope");
    assert!(matches!(
        validate_operations(&catalog),
        Err(CrosswalkError::Invalid(message))
            if message.contains("applyAuthorEdit Project Scope")
    ));
}

#[test]
fn baseline_reader_never_falls_through_to_the_worktree() {
    assert!(repository_root().join("Cargo.toml").exists());
    assert!(matches!(
        read_baseline_file(repository_root(), "Cargo.toml"),
        Err(CrosswalkError::Invalid(_))
    ));
}

#[test]
fn wrong_baseline_tree_is_rejected() {
    assert!(matches!(
        verify_baseline_tree(repository_root(), "wrong-tree"),
        Err(CrosswalkError::Invalid(message)) if message.contains("baseline tree drifted")
    ));
}
