use std::path::Path;

use serde_json::Value;

use super::{
    BASELINE_COMMIT, BASELINE_TREE, CONTRACT_REVISION, GENERATED_CROSSWALK_PATH, ISSUE_BODY_SHA256,
    ManifestError, OPERATIONS, ROUTE_CATALOG_PATH, generate_crosswalk, read_json,
    validate_operations,
};

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("contracts crate must be nested under the repository root")
}

fn route_catalog() -> Value {
    read_json(repo_root(), ROUTE_CATALOG_PATH).expect("route catalog should load")
}

#[test]
fn generated_crosswalk_matches_the_locked_contract_as_a_whole() {
    let generated: Value = serde_json::from_slice(
        &generate_crosswalk(repo_root()).expect("crosswalk generation should succeed"),
    )
    .expect("generated crosswalk should be JSON");
    assert_eq!(
        generated["execution_contract"],
        serde_json::json!({
            "issue": "https://github.com/FrankQDWang/StoryOS/issues/100",
            "revision": CONTRACT_REVISION,
            "baseline_commit": BASELINE_COMMIT,
            "baseline_tree": BASELINE_TREE,
            "issue_body_sha256": ISSUE_BODY_SHA256,
        })
    );
    assert_eq!(
        generated["operations"].as_array().map(Vec::len),
        Some(OPERATIONS.len())
    );
    assert_eq!(
        generated["claim_ceiling"],
        "contract-foundation-only; no S1 requirement or stage acceptance"
    );
}

#[test]
fn checked_in_crosswalk_matches_fresh_generation() {
    let actual = std::fs::read(repo_root().join(GENERATED_CROSSWALK_PATH))
        .expect("checked-in crosswalk should exist");
    let expected = generate_crosswalk(repo_root()).expect("crosswalk generation should succeed");
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
        Err(ManifestError::Invalid(message))
            if message == "missing Stage 1 operation applyAuthorEdit"
    ));
}

#[test]
fn stage1_schema_drift_is_rejected() {
    let mut catalog = route_catalog();
    let operation = catalog["operations"]
        .as_array_mut()
        .expect("operations should be an array")
        .iter_mut()
        .find(|operation| operation["operation_id"] == "getSnapshot")
        .expect("getSnapshot should exist");
    operation["schemas"]["response"] = Value::String("invented.snapshot.v1".into());
    assert!(matches!(
        validate_operations(&catalog),
        Err(ManifestError::Invalid(message))
            if message.contains("getSnapshot response_schema drifted")
    ));
}
