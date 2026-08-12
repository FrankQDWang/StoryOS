use std::collections::BTreeSet;

use crate::project_command_targets::PROJECT_COMMAND_TARGETS;

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
