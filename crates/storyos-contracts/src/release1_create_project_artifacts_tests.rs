struct MaxUtf8Bytes(usize);

impl<'instance> jsonschema::Keyword<'instance> for MaxUtf8Bytes {
    fn validate(
        &self,
        instance: &'instance serde_json::Value,
    ) -> Result<(), jsonschema::ValidationError<'instance>> {
        if self.is_valid(instance) {
            Ok(())
        } else {
            Err(jsonschema::ValidationError::custom(
                "string exceeds the UTF-8 byte limit",
            ))
        }
    }

    fn is_valid(&self, instance: &'instance serde_json::Value) -> bool {
        instance.as_str().is_some_and(|value| value.len() <= self.0)
    }
}

fn max_utf8_bytes_validator<'schema>(
    _parent: &'schema serde_json::Map<String, serde_json::Value>,
    value: &'schema serde_json::Value,
    _path: jsonschema::paths::Location,
) -> Result<
    Box<dyn for<'instance> jsonschema::Keyword<'instance>>,
    jsonschema::ValidationError<'schema>,
> {
    let limit = value
        .as_u64()
        .and_then(|limit| usize::try_from(limit).ok())
        .ok_or_else(|| jsonschema::ValidationError::custom("UTF-8 byte limit must be a usize"))?;
    Ok(Box::new(MaxUtf8Bytes(limit)))
}

fn assert_each_present_field_is_required(
    validator: &jsonschema::Validator,
    valid: &serde_json::Value,
    object_pointer: &str,
) {
    let field_names = valid
        .pointer(object_pointer)
        .expect("required-field object must exist")
        .as_object()
        .expect("required-field target must be an object")
        .keys()
        .cloned()
        .collect::<Vec<_>>();

    for field_name in field_names {
        let mut invalid = valid.clone();
        invalid
            .pointer_mut(object_pointer)
            .expect("required-field object must exist")
            .as_object_mut()
            .expect("required-field target must be an object")
            .remove(&field_name);
        assert!(
            !validator.is_valid(&invalid),
            "{field_name} must be required at {object_pointer}"
        );
    }
}

#[test]
fn prospective_create_project_challenge_wire_is_generated_from_one_closed_contract() {
    let operation = &crate::release1_create_project::CREATE_PROJECT_CHALLENGE;
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
        .expect("reviewed route catalog must contain the prospective Project challenge");

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
        "kind": "challenge",
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
        &generated[crate::release1_create_project_artifacts::CHALLENGE_REQUEST_SCHEMA_PATH],
    )
    .expect("prospective Project challenge request schema must be JSON");
    let response_schema: serde_json::Value = serde_json::from_slice(
        &generated[crate::release1_create_project_artifacts::CHALLENGE_RESPONSE_SCHEMA_PATH],
    )
    .expect("prospective Project challenge response schema must be JSON");
    assert_eq!(request_schema["$id"], operation.request_schema);
    assert_eq!(response_schema["$id"], operation.response_schema);
    assert_eq!(request_schema["additionalProperties"], false);
    assert_eq!(
        request_schema["properties"]["command_schema"]["const"],
        crate::CREATE_PROJECT_REQUEST_SCHEMA_ID
    );
    let request_validator = jsonschema::options()
        .should_validate_formats(true)
        .with_keyword("x-storyos-max-utf8-bytes", max_utf8_bytes_validator)
        .build(&request_schema)
        .expect("prospective Project challenge request schema must compile");
    let valid_request = serde_json::json!({
        "command_schema": crate::CREATE_PROJECT_REQUEST_SCHEMA_ID,
        "create_project_input": {
            "title": format!("{}a", "界".repeat(341)),
            "client_contract_revision": "storyos.web-client.release-1.v3",
            "security_policy_revision": "storyos.web-security-policy.release-1.v1",
            "correlation_id": "018f0000-0000-7001-8000-000000000202"
        },
        "idempotency_key": "018f0000-0000-7001-8000-000000000206"
    });
    assert!(request_validator.is_valid(&valid_request));
    assert_each_present_field_is_required(&request_validator, &valid_request, "");
    assert_each_present_field_is_required(
        &request_validator,
        &valid_request,
        "/create_project_input",
    );
    let mut invalid_requests = Vec::new();
    let mut invalid = valid_request.clone();
    invalid["command_schema"] = serde_json::json!("storyos.command.update-project.request.v1");
    invalid_requests.push(invalid);
    let mut invalid = valid_request.clone();
    invalid["create_project_input"]["title"] = serde_json::json!("");
    invalid_requests.push(invalid);
    let mut invalid = valid_request.clone();
    invalid["create_project_input"]["title"] = serde_json::json!(format!("{}aa", "界".repeat(341)));
    invalid_requests.push(invalid);
    let mut invalid = valid_request.clone();
    invalid["create_project_input"]["correlation_id"] = serde_json::json!("not-a-uuid");
    invalid_requests.push(invalid);
    let mut invalid = valid_request.clone();
    invalid["idempotency_key"] = serde_json::json!("not-a-uuid");
    invalid_requests.push(invalid);
    let mut invalid = valid_request.clone();
    invalid["unexpected"] = serde_json::json!(true);
    invalid_requests.push(invalid);
    let mut invalid = valid_request.clone();
    invalid["create_project_input"]["unexpected"] = serde_json::json!(true);
    invalid_requests.push(invalid);
    assert!(
        invalid_requests
            .iter()
            .all(|request| !request_validator.is_valid(request))
    );

    let response_validator = jsonschema::options()
        .should_validate_formats(true)
        .build(&response_schema)
        .expect("prospective Project challenge response schema must compile");
    let positive: serde_json::Value = serde_json::from_slice(
        &generated[crate::release1_create_project_artifacts::CHALLENGE_FIXTURE_PATHS[0]],
    )
    .expect("positive challenge fixture must be JSON");
    assert!(response_validator.is_valid(&positive));
    serde_json::from_value::<crate::CreateProjectChallengeResponse>(positive.clone())
        .expect("positive challenge fixture must match the Rust response contract");
    assert_each_present_field_is_required(&response_validator, &positive, "");
    assert_each_present_field_is_required(
        &response_validator,
        &positive,
        "/canonical_command_digest",
    );
    let boundary: serde_json::Value = serde_json::from_slice(
        &generated[crate::release1_create_project_artifacts::CHALLENGE_FIXTURE_PATHS[2]],
    )
    .expect("boundary challenge fixture must be JSON");
    assert!(response_validator.is_valid(&boundary));
    let invalid_fixture: serde_json::Value = serde_json::from_slice(
        &generated[crate::release1_create_project_artifacts::CHALLENGE_FIXTURE_PATHS[1]],
    )
    .expect("invalid challenge fixture must be JSON");
    let mut invalid_responses = vec![invalid_fixture];
    for (pointer, value) in [
        ("/prospective_project_id", serde_json::json!("not-a-uuid")),
        (
            "/canonical_command_digest/profile",
            serde_json::json!("storyos.command.other.jcs.v1"),
        ),
        (
            "/canonical_command_digest/value_hex_lowercase",
            serde_json::json!("D".repeat(64)),
        ),
        ("/nonce", serde_json::json!("e".repeat(63))),
        ("/expires_at", serde_json::json!("not-a-date-time")),
        (
            "/limit_profile_revision",
            serde_json::json!("storyos.foundation.other.v1"),
        ),
    ] {
        let mut invalid = positive.clone();
        *invalid
            .pointer_mut(pointer)
            .expect("invalid response target must exist") = value;
        invalid_responses.push(invalid);
    }
    let mut invalid = positive.clone();
    invalid["unexpected"] = serde_json::json!(true);
    invalid_responses.push(invalid);
    let mut invalid = positive.clone();
    invalid["canonical_command_digest"]["unexpected"] = serde_json::json!(true);
    invalid_responses.push(invalid);
    assert!(
        invalid_responses
            .iter()
            .all(|response| !response_validator.is_valid(response))
    );

    let openapi =
        String::from_utf8(generated["generated/openapi/storyos-public-release-1.yaml"].clone())
            .expect("OpenAPI must be UTF-8");
    assert!(openapi.contains(&format!(
        "  {}:\n    {}:\n      operationId: {}",
        operation.path,
        operation.method.to_ascii_lowercase(),
        operation.operation_id
    )));
    let path_marker = format!("  {}:\n", operation.path);
    let operation_start = openapi
        .find(&path_marker)
        .expect("OpenAPI must contain the prospective Project challenge path");
    let remaining = &openapi[operation_start..];
    let operation_end = remaining.find("\n  /api/").unwrap_or(remaining.len());
    let operation_openapi = &remaining[..operation_end];
    let (_, request_and_responses) = operation_openapi
        .split_once("      requestBody:\n")
        .expect("challenge operation must have a request body");
    let (request_body, responses) = request_and_responses
        .split_once("      responses:\n")
        .expect("challenge operation must have responses");
    let request_reference = crate::release1_create_project_artifacts::CHALLENGE_REQUEST_SCHEMA_PATH
        .strip_prefix("generated/")
        .expect("request schema path must be generated");
    let request_references = request_body
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("$ref: '../")
                .and_then(|value| value.strip_suffix('\''))
        })
        .collect::<Vec<_>>();
    assert_eq!(request_references, vec![request_reference]);

    let response_statuses = responses
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix("':"))
                .and_then(|value| value.parse::<u16>().ok())
        })
        .collect::<Vec<_>>();
    let expected_statuses = operation
        .responses
        .iter()
        .map(|(status, _)| *status)
        .collect::<Vec<_>>();
    assert_eq!(response_statuses, expected_statuses);
    let success_response = responses
        .split("        '")
        .find(|block| block.starts_with("200':\n"))
        .expect("challenge operation must have a 200 response");
    let response_reference =
        crate::release1_create_project_artifacts::CHALLENGE_RESPONSE_SCHEMA_PATH
            .strip_prefix("generated/")
            .expect("response schema path must be generated");
    let response_references = success_response
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("$ref: '../")
                .and_then(|value| value.strip_suffix('\''))
        })
        .collect::<Vec<_>>();
    assert_eq!(response_references, vec![response_reference]);

    let implemented_slice = openapi
        .lines()
        .find(|line| line.starts_with("  x-storyos-implemented-slice:"))
        .expect("OpenAPI names the runtime-implemented slice");
    assert!(!implemented_slice.contains(operation.operation_id));
    assert!(!openapi.contains("/api/v1/projects:\n    post:\n      operationId: createProject"));

    let schema_catalog: serde_json::Value = serde_json::from_slice(
        &generated["generated/schema-catalog/storyos-public-release-1.json"],
    )
    .expect("schema catalog must be JSON");
    let implemented_operations = schema_catalog["implemented_operations"]
        .as_array()
        .expect("implemented operations must be an array");
    assert!(
        !implemented_operations
            .iter()
            .any(|implemented| implemented == operation.operation_id)
    );
    let schema_ids = schema_catalog["schemas"]
        .as_array()
        .expect("schema catalog entries must be an array")
        .iter()
        .map(|schema| {
            schema["schema_id"]
                .as_str()
                .expect("schema ID must be a string")
        })
        .collect::<Vec<_>>();
    for schema_id in [operation.request_schema, operation.response_schema] {
        assert!(schema_ids.contains(&schema_id));
    }

    let client = String::from_utf8(
        generated["generated/typescript/storyos-public-release-1/client.mjs"].clone(),
    )
    .expect("generated client must be UTF-8");
    let typescript_operation = reviewed_operation["generated"]["typescript_operation"]
        .as_str()
        .expect("TypeScript operation must be a string");
    assert!(client.contains(&format!("export async function {typescript_operation}(")));
    assert!(!client.contains("export async function createProject("));
    assert!(!client.contains("export async function digestCreateProject"));

    let fixture_catalog: serde_json::Value =
        serde_json::from_slice(&generated["generated/fixtures/storyos-public-release-1.json"])
            .expect("fixture catalog must be JSON");
    let generated_fixture_ids = fixture_catalog["fixtures"]
        .as_array()
        .expect("fixture catalog entries must be an array")
        .iter()
        .filter(|fixture| fixture["operation_id"] == operation.operation_id)
        .map(|fixture| {
            fixture["fixture_id"]
                .as_str()
                .expect("fixture ID must be a string")
        })
        .collect::<Vec<_>>();
    assert_eq!(generated_fixture_ids, operation.fixtures);

    let graph: serde_json::Value = serde_json::from_slice(&super::contract_graph_bytes())
        .expect("contract graph must be JSON");
    let graph_operation = graph["operations"]
        .as_array()
        .expect("contract graph operations must be an array")
        .iter()
        .find(|entry| entry["operation_id"] == operation.operation_id)
        .expect("contract graph must contain the prospective Project challenge");
    let graph_surface = serde_json::json!({
        "operation_id": graph_operation["operation_id"],
        "kind": graph_operation["kind"],
        "method": graph_operation["method"],
        "path": graph_operation["path"],
        "request_schema": graph_operation["request_schema"],
        "response_schema": graph_operation["response_schema"],
        "http_statuses": graph_operation["http_statuses"],
        "fixtures": graph_operation["fixtures"],
    });
    assert_eq!(graph_surface, source_surface);
    let graph_preconditions = graph_operation["preconditions"]
        .as_array()
        .expect("graph preconditions must be an array");
    for precondition in reviewed_operation["preconditions"]
        .as_array()
        .expect("reviewed preconditions must be an array")
    {
        assert!(graph_preconditions.contains(precondition));
    }
}
