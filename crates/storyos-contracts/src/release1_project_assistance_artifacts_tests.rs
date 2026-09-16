#[test]
fn project_assistance_wire_is_generated_from_one_closed_contract() {
    let get_operation = &crate::release1_project_assistance::GET_PROJECT_ASSISTANCE;
    let update_operation = &crate::release1_project_assistance::UPDATE_PROJECT_ASSISTANCE;
    let generated = super::generated_files()
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    let reviewed_catalog: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../docs/foundation/versioned-protocol-release-1-route-catalog.json"
    ))
    .expect("reviewed route catalog must be JSON");
    for operation in [get_operation, update_operation] {
        let reviewed_operation = reviewed_catalog["operations"]
            .as_array()
            .expect("reviewed operations must be an array")
            .iter()
            .find(|entry| entry["operation_id"] == operation.operation_id)
            .unwrap_or_else(|| panic!("reviewed catalog must contain {}", operation.operation_id));
        let reviewed_fixture_ids = std::iter::once(
            reviewed_operation["fixtures"]["positive"]
                .as_str()
                .expect("positive fixture ID must be a string"),
        )
        .chain(
            reviewed_operation["fixtures"]["negative"]
                .as_array()
                .expect("negative fixture IDs must be an array")
                .iter()
                .map(|fixture| {
                    fixture
                        .as_str()
                        .expect("negative fixture ID must be a string")
                }),
        )
        .collect::<Vec<_>>();
        let source_surface = serde_json::json!({
            "operation_id": operation.operation_id,
            "kind": if operation.operation_id == "getProjectAssistance" { "query" } else { "command" },
            "method": operation.method,
            "path": operation.path,
            "request_schema": operation.request_schema,
            "response_schema": operation.response_schema,
            "http_statuses": operation.responses.iter().map(|(status, _)| status).collect::<Vec<_>>(),
            "fixtures": operation.fixtures,
        });
        let reviewed_surface = serde_json::json!({
            "operation_id": reviewed_operation["operation_id"],
            "kind": reviewed_operation["kind"],
            "method": reviewed_operation["method"],
            "path": reviewed_operation["path"],
            "request_schema": reviewed_operation["schemas"]["request"],
            "response_schema": reviewed_operation["schemas"]["response"],
            "http_statuses": reviewed_operation["settlement"]["http_statuses"],
            "fixtures": reviewed_fixture_ids,
        });
        assert_eq!(source_surface, reviewed_surface);
    }

    let openapi =
        String::from_utf8(generated["generated/openapi/storyos-public-release-1.yaml"].clone())
            .expect("OpenAPI must be UTF-8");
    assert!(openapi.contains("/api/v1/projects/{project_id}/assistance:"));
    assert!(openapi.contains("operationId: getProjectAssistance"));
    assert!(openapi.contains("operationId: updateProjectAssistance"));

    let client = String::from_utf8(
        generated["generated/typescript/storyos-public-release-1/client.mjs"].clone(),
    )
    .expect("generated client must be UTF-8");
    assert!(client.contains("export async function getProjectAssistance("));
    assert!(client.contains("export async function digestUpdateProjectAssistance("));
    assert!(client.contains("export async function updateProjectAssistance("));
    assert!(client.contains("storyos.command.updateProjectAssistance.jcs.v1"));
}
