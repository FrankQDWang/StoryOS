use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_author_edit_artifacts as author_edit_artifacts;
use crate::release1_author_edit_outcome::{
    ApplyAuthorEditOutcome, ApplyAuthorEditRejectionReason, ApplyAuthorEditUnknownObservation,
    GET_APPLY_AUTHOR_EDIT_OUTCOME, GET_APPLY_AUTHOR_EDIT_OUTCOME_REQUEST_SCHEMA_ID,
    GET_APPLY_AUTHOR_EDIT_OUTCOME_RESPONSE_SCHEMA_ID, GetApplyAuthorEditOutcomeRequest,
    GetApplyAuthorEditOutcomeResponse,
};

pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/apply-author-edit-outcome-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/apply-author-edit-outcome-response.schema.json";
pub(super) const FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/get-apply-author-edit-outcome.json",
    "generated/golden-wire/storyos-public-release-1/get-apply-author-edit-outcome.invalid.json",
    "generated/golden-wire/storyos-public-release-1/get-apply-author-edit-outcome.boundary.json",
];

pub(super) fn request_schema_bytes() -> Vec<u8> {
    let mut schema = typed_schema::<GetApplyAuthorEditOutcomeRequest>(
        GET_APPLY_AUTHOR_EDIT_OUTCOME_REQUEST_SCHEMA_ID,
        "StoryOS Apply Author Edit Outcome Request",
    );
    schema["properties"]["project_id"]["format"] = json!("uuid");
    schema["properties"]["idempotency_key"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = typed_schema::<GetApplyAuthorEditOutcomeResponse>(
        GET_APPLY_AUTHOR_EDIT_OUTCOME_RESPONSE_SCHEMA_ID,
        "StoryOS Apply Author Edit Outcome Response",
    );
    let canonical_u64 = author_edit_artifacts::canonical_u64_wire_schema();
    author_edit_artifacts::apply_u64_wire_constraints(&mut schema, &canonical_u64);
    schema["$defs"]["ApplyAuthorEditResponse"]["properties"]["local_intent_sequence"] =
        canonical_u64;
    schema["$defs"]["ApplyAuthorEditUnknownObservation"]["oneOf"][0]["properties"]["expires_at"]
        ["format"] = json!("date-time");
    for definition in [
        "ApplyAuthorEditOutcome",
        "ApplyAuthorEditUnknownObservation",
    ] {
        for variant in schema["$defs"][definition]["oneOf"]
            .as_array_mut()
            .expect("tagged outcome variants must exist")
        {
            variant["additionalProperties"] = Value::Bool(false);
        }
    }
    json_bytes(&schema)
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        GetApplyAuthorEditOutcomeRequest::decl(&config),
        ApplyAuthorEditRejectionReason::decl(&config),
        ApplyAuthorEditUnknownObservation::decl(&config),
        ApplyAuthorEditOutcome::decl(&config),
        GetApplyAuthorEditOutcomeResponse::decl(&config),
    )
}

pub(super) fn typescript_client_source() -> String {
    let path = GET_APPLY_AUTHOR_EDIT_OUTCOME
        .path
        .replace("{project_id}", "${encodeURIComponent(projectId)}")
        .replace("{idempotency_key}", "${encodeURIComponent(idempotencyKey)}");
    format!(
        concat!(
            "\nexport async function getApplyAuthorEditOutcome({{ projectId, idempotencyKey, antiForgery, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"getApplyAuthorEditOutcome requires projectId\");\n",
            "  if (typeof idempotencyKey !== \"string\" || typeof antiForgery !== \"string\") throw new TypeError(\"getApplyAuthorEditOutcome requires security bindings\");\n",
            "  return queryJson({{ ...options, path: `{}`, queryHeaders: {{ \"x-storyos-anti-forgery\": antiForgery }} }});\n",
            "}}\n",
        ),
        path,
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    "export declare function getApplyAuthorEditOutcome(options: StoryOSQueryOptions & { projectId: string; idempotencyKey: string; antiForgery: string }): Promise<GetApplyAuthorEditOutcomeResponse>;\n"
}

pub(super) fn openapi() -> String {
    let response_schema = RESPONSE_SCHEMA_PATH
        .strip_prefix("generated/")
        .expect("response schema is generated");
    let responses = GET_APPLY_AUTHOR_EDIT_OUTCOME
        .responses
        .iter()
        .map(|(status, description)| {
            format!(
                "        '{status}':\n          description: {description}\n          headers:\n            Cache-Control:\n              required: true\n              schema:\n                type: string\n                const: no-store\n{}",
                if *status == 200 {
                    format!(
                        "          content:\n            application/json:\n              schema:\n                $ref: '../{response_schema}'\n"
                    )
                } else {
                    String::new()
                }
            )
        })
        .collect::<String>();
    format!(
        "  {}:\n    get:\n      operationId: {}\n      summary: Read one exact Apply Author Edit outcome\n      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n        - name: idempotency_key\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n      responses:\n{responses}",
        GET_APPLY_AUTHOR_EDIT_OUTCOME.path, GET_APPLY_AUTHOR_EDIT_OUTCOME.operation_id,
    )
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&fixture())
}

pub(super) fn invalid_fixture_bytes() -> Vec<u8> {
    let mut invalid = fixture();
    invalid
        .as_object_mut()
        .expect("fixture is an object")
        .remove("project_scope");
    json_bytes(&invalid)
}

pub(super) fn boundary_fixture_bytes() -> Vec<u8> {
    let mut boundary = fixture();
    boundary["outcome"] = json!({
        "outcome_kind": "still_unknown",
        "observation": {
            "observation_kind": "admission_committed",
            "command_id": "018f0000-0000-7001-8000-000000000031",
            "author_command_admission_id": "018f0000-0000-7001-8000-000000000032",
            "reconciliation_required": true
        }
    });
    json_bytes(&boundary)
}

fn fixture() -> Value {
    json!({
        "schema_id": GET_APPLY_AUTHOR_EDIT_OUTCOME_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000081",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000002"
        },
        "outcome": {
            "outcome_kind": "committed",
            "response": author_edit_artifacts::fixture()
        }
    })
}

fn typed_schema<T: schemars::JsonSchema>(schema_id: &str, title: &str) -> Value {
    let mut schema = serde_json::to_value(schema_for!(T)).expect("contract schema serializes");
    schema["$id"] = Value::String(schema_id.to_owned());
    schema["title"] = Value::String(title.to_owned());
    schema
}

fn json_bytes(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("contract JSON should serialize");
    bytes.push(b'\n');
    bytes
}
