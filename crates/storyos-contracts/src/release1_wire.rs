//! Shared helpers for the generated Release 1 wire artifacts.

use schemars::schema_for;
use serde_json::{Value, json};

use crate::release1::QueryOperation;

/// The JSON Schema pattern of a canonical decimal `u64` wire string.
pub(super) const U64_WIRE: &str = "^(?:0|[1-9][0-9]{0,18}|1[0-7][0-9]{18}|18[0-3][0-9]{17}|184[0-3][0-9]{16}|1844[0-5][0-9]{15}|18446[0-6][0-9]{14}|184467[0-3][0-9]{13}|1844674[0-3][0-9]{12}|184467440[0-6][0-9]{10}|1844674407[0-2][0-9]{9}|18446744073[0-6][0-9]{8}|1844674407370[0-8][0-9]{6}|18446744073709[0-4][0-9]{5}|184467440737095[0-4][0-9]{3}|1844674407370955[0-9]{2}|18446744073709551[0-5]|1844674407370955160|1844674407370955161[0-5])$";

/// The JSON Schema of a canonical decimal `u64` wire string.
pub(super) fn canonical_u64_wire_schema() -> Value {
    json!({
        "type": "string",
        "pattern": U64_WIRE
    })
}

pub(super) fn schema_value<T: schemars::JsonSchema>(schema_id: &str, title: &str) -> Value {
    let mut schema = serde_json::to_value(schema_for!(T)).expect("contract schema serializes");
    schema["$id"] = Value::String(schema_id.to_owned());
    schema["title"] = Value::String(title.to_owned());
    schema
}

pub(super) fn json_bytes(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("contract JSON should serialize");
    bytes.push(b'\n');
    bytes
}

pub(super) fn generated_ref(path: &str) -> &str {
    path.strip_prefix("generated/")
        .expect("schema is a generated artifact")
}

pub(super) fn query_openapi(
    operation: &QueryOperation,
    summary: &str,
    response_schema: &str,
    parameters: &[&str],
) -> String {
    let response_schema = response_schema
        .strip_prefix("generated/")
        .expect("OpenAPI response schemas must be generated artifacts");
    let parameters = if parameters.is_empty() {
        String::new()
    } else {
        format!(
            "      parameters:\n{}",
            parameters
                .iter()
                .map(|name| format!("        - name: {name}\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n"))
                .collect::<String>()
        )
    };
    let responses = operation.responses.iter().map(|(status, description)| format!(
        "        '{status}':\n          description: {description}\n{}",
        if *status == 200 { format!("          content:\n            application/json:\n              schema:\n                $ref: '../{response_schema}'\n") } else { String::new() }
    )).collect::<String>();
    format!(
        "    {}:\n      operationId: {}\n      summary: {summary}\n{parameters}      responses:\n{responses}",
        operation.method.to_ascii_lowercase(),
        operation.operation_id,
    )
}

pub(super) fn path_request_schema(schema_id: &str, fields: &[&str]) -> Value {
    let properties = fields
        .iter()
        .map(|field| {
            (
                (*field).to_owned(),
                json!({"type": "string", "format": "uuid"}),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "$id": schema_id,
           "type": "object", "additionalProperties": false, "required": fields,
           "properties": properties})
}

pub(super) fn without_project_scope(mut value: Value) -> Vec<u8> {
    value
        .as_object_mut()
        .expect("fixture is an object")
        .remove("project_scope");
    json_bytes(&value)
}

pub(super) fn with_boundary_project_scope(mut value: Value) -> Vec<u8> {
    value["project_scope"]["project_id"] =
        Value::String("018f0000-0000-7001-8000-000000000102".to_owned());
    json_bytes(&value)
}
