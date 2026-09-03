#[test]
fn get_export_operation_wire_is_generated_from_one_closed_contract() {
    let operation = &crate::release1_project_export_query::GET_EXPORT_OPERATION;
    let generated = super::generated_files()
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    let reviewed_catalog: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../docs/foundation/versioned-protocol-release-1-route-catalog.json"
    ))
    .expect("reviewed route catalog must be JSON");
    let reviewed_operation = reviewed_catalog["operations"]
        .as_array()
        .expect("reviewed operations must be an array")
        .iter()
        .find(|entry| entry["operation_id"] == operation.operation_id)
        .expect("reviewed catalog must contain getExportOperation");
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
        "kind": "query",
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

    let request_schema: serde_json::Value = serde_json::from_slice(
        &generated[crate::release1_project_export_query_artifacts::REQUEST_SCHEMA_PATH],
    )
    .expect("export query request schema must be JSON");
    let response_schema: serde_json::Value = serde_json::from_slice(
        &generated[crate::release1_project_export_query_artifacts::RESPONSE_SCHEMA_PATH],
    )
    .expect("export query response schema must be JSON");
    assert_eq!(request_schema["$id"], operation.request_schema);
    assert_eq!(response_schema["$id"], operation.response_schema);

    let openapi =
        String::from_utf8(generated["generated/openapi/storyos-public-release-1.yaml"].clone())
            .expect("OpenAPI must be UTF-8");
    assert!(openapi.contains(
        "/api/v1/projects/{project_id}/exports/{export_id}:\n    get:\n      operationId: getExportOperation"
    ));

    let client = String::from_utf8(
        generated["generated/typescript/storyos-public-release-1/client.mjs"].clone(),
    )
    .expect("generated client must be UTF-8");
    assert!(client.contains("export async function getExportOperation("));

    let graph: serde_json::Value = serde_json::from_slice(&super::contract_graph_bytes())
        .expect("contract graph must be JSON");
    let graph_operation = graph["operations"]
        .as_array()
        .expect("contract graph operations must be an array")
        .iter()
        .find(|entry| entry["operation_id"] == operation.operation_id)
        .expect("contract graph must contain getExportOperation");
    assert_eq!(graph_operation["path"], operation.path);
    let preconditions = graph_operation["preconditions"]
        .as_array()
        .expect("graph preconditions must be an array");
    assert!(preconditions.contains(&serde_json::json!("export_scope_join")));
    assert!(preconditions.contains(&serde_json::json!("export_operation_visibility")));

    let positive: serde_json::Value = serde_json::from_slice(
        &generated[crate::release1_project_export_query_artifacts::FIXTURE_PATHS[0]],
    )
    .expect("positive export query fixture must be JSON");
    assert_eq!(positive["status"], "ready");
    assert_eq!(
        positive["immutable_root"],
        format!("sha256:{}", "b".repeat(64))
    );
    assert_eq!(positive["archive_profile"], "storyos.project-export.v1");
}
