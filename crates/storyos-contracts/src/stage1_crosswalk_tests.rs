use serde_json::Value;

use super::{
    CrosswalkError, GENERATED_CROSSWALK_PATH, ROUTE_CATALOG_PATH, generate_crosswalk,
    read_baseline_file, read_baseline_json, repository_root, validate_operations,
    verify_baseline_tree, verify_delivery_baseline_tree, verify_historical_foundation_crosswalk,
};

fn route_catalog() -> Value {
    read_baseline_json(repository_root(), ROUTE_CATALOG_PATH).expect("route catalog should load")
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
    let path = "crates/storyos-contracts/src/stage1_delivery_tests.rs";
    let worktree = std::fs::read_to_string(repository_root().join(path))
        .expect("worktree delivery tests should exist");
    let baseline =
        read_baseline_file(repository_root(), path).expect("baseline delivery tests should exist");

    assert!(worktree.contains("acknowledgement_loss_ticket_matches_the_current_process_profile"));
    assert!(
        !String::from_utf8(baseline)
            .expect("baseline delivery tests should be UTF-8")
            .contains("acknowledgement_loss_ticket_matches_the_current_process_profile")
    );
}

#[test]
fn wrong_baseline_tree_is_rejected() {
    assert!(matches!(
        verify_baseline_tree(repository_root(), "wrong-tree"),
        Err(CrosswalkError::Invalid(message)) if message.contains("baseline tree drifted")
    ));
}

#[test]
fn wrong_delivery_baseline_tree_is_rejected() {
    assert!(matches!(
        verify_delivery_baseline_tree(repository_root(), "wrong-tree"),
        Err(CrosswalkError::Invalid(message))
            if message.contains("delivery baseline tree drifted")
    ));
}

#[test]
fn wrong_historical_foundation_crosswalk_digest_is_rejected() {
    assert!(matches!(
        verify_historical_foundation_crosswalk(repository_root(), "sha256:wrong"),
        Err(CrosswalkError::Invalid(message))
            if message.contains("PR #102 foundation crosswalk drifted")
    ));
}
