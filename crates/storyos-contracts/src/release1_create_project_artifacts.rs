use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_create_project::{
    CREATE_PROJECT, CREATE_PROJECT_CHALLENGE, CREATE_PROJECT_CHALLENGE_REQUEST_SCHEMA_ID,
    CREATE_PROJECT_CHALLENGE_RESPONSE_SCHEMA_ID, CREATE_PROJECT_DIGEST_PROFILE,
    CREATE_PROJECT_REQUEST_SCHEMA_ID, CREATE_PROJECT_RESPONSE_SCHEMA_ID,
    CreateProjectChallengeRequest, CreateProjectChallengeResponse, CreateProjectInput,
    CreateProjectRequest, CreateProjectResponse,
};

pub(super) const CHALLENGE_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/create-project-challenge-request.schema.json";
pub(super) const CHALLENGE_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/create-project-challenge-response.schema.json";
pub(super) const CHALLENGE_FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/create-project-challenge.json",
    "generated/golden-wire/storyos-public-release-1/create-project-challenge.invalid.json",
    "generated/golden-wire/storyos-public-release-1/create-project-challenge.boundary.json",
];
pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/create-project-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/create-project-response.schema.json";
pub(super) const FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/create-project.json",
    "generated/golden-wire/storyos-public-release-1/create-project.invalid.json",
    "generated/golden-wire/storyos-public-release-1/create-project.boundary.json",
];

pub(super) fn challenge_request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<CreateProjectChallengeRequest>(
        CREATE_PROJECT_CHALLENGE_REQUEST_SCHEMA_ID,
        "StoryOS Create Project Challenge Request",
    );
    schema["properties"]["command_schema"]["const"] = json!(CREATE_PROJECT_REQUEST_SCHEMA_ID);
    schema["properties"]["idempotency_key"]["format"] = json!("uuid");
    schema["$defs"]["CreateProjectInput"]["properties"]["correlation_id"]["format"] = json!("uuid");
    let title = &mut schema["$defs"]["CreateProjectInput"]["properties"]["title"];
    title["minLength"] = json!(1);
    title["x-storyos-max-utf8-bytes"] = json!(1024);
    json_bytes(&schema)
}

pub(super) fn challenge_response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<CreateProjectChallengeResponse>(
        CREATE_PROJECT_CHALLENGE_RESPONSE_SCHEMA_ID,
        "StoryOS Create Project Challenge Response",
    );
    schema["properties"]["prospective_project_id"]["format"] = json!("uuid");
    schema["properties"]["expires_at"]["format"] = json!("date-time");
    schema["properties"]["nonce"]["pattern"] = json!("^[0-9a-f]{64}$");
    schema["properties"]["limit_profile_revision"]["const"] = json!(crate::LIMIT_PROFILE_REVISION);
    schema["$defs"]["DigestValue"]["properties"]["profile"]["const"] =
        json!(CREATE_PROJECT_DIGEST_PROFILE);
    schema["$defs"]["DigestValue"]["properties"]["value_hex_lowercase"]["pattern"] =
        json!("^[0-9a-f]{64}$");
    json_bytes(&schema)
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        CreateProjectInput::decl(&config),
        CreateProjectChallengeRequest::decl(&config),
        CreateProjectChallengeResponse::decl(&config),
        CreateProjectRequest::decl(&config),
        CreateProjectResponse::decl(&config),
    )
}

pub(super) fn openapi() -> String {
    let request_schema = generated_ref(CHALLENGE_REQUEST_SCHEMA_PATH);
    let response_schema = generated_ref(CHALLENGE_RESPONSE_SCHEMA_PATH);
    let responses = response_block(CREATE_PROJECT_CHALLENGE.responses, response_schema, &[200]);
    format!(
        concat!(
            "  {}:\n    post:\n      operationId: {}\n      summary: Issue one prospective Project command challenge\n",
            "      parameters:\n        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        CREATE_PROJECT_CHALLENGE.path,
        CREATE_PROJECT_CHALLENGE.operation_id,
        request_schema,
        responses,
    ) + &command_openapi()
}

pub(super) fn command_request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<CreateProjectRequest>(
        CREATE_PROJECT_REQUEST_SCHEMA_ID,
        "StoryOS Create Project Request",
    );
    schema["properties"]["command_schema"]["const"] = json!(CREATE_PROJECT_REQUEST_SCHEMA_ID);
    schema["properties"]["prospective_project_id"]["format"] = json!("uuid");
    schema["$defs"]["CreateProjectInput"]["properties"]["correlation_id"]["format"] = json!("uuid");
    let title = &mut schema["$defs"]["CreateProjectInput"]["properties"]["title"];
    title["minLength"] = json!(1);
    title["x-storyos-max-utf8-bytes"] = json!(1024);
    json_bytes(&schema)
}

pub(super) fn command_response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<CreateProjectResponse>(
        CREATE_PROJECT_RESPONSE_SCHEMA_ID,
        "StoryOS Create Project Response",
    );
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["command_id"]["format"] = json!("uuid");
    schema["properties"]["author_command_admission_id"]["format"] = json!("uuid");
    schema["properties"]["schema_id"]["const"] = json!(CREATE_PROJECT_RESPONSE_SCHEMA_ID);
    if schema["$defs"].get("DomainReceipt").is_some() {
        schema["$defs"]["DomainReceipt"]["properties"]["author_action_sequence"] = json!({
            "anyOf": [
                {
                    "type": "string",
                    "pattern": "^(0|[1-9][0-9]{0,19})$"
                },
                {"type": "null"}
            ]
        });
        schema["$defs"]["DomainReceipt"]["properties"]["idempotency_key"]["format"] = json!("uuid");
        schema["$defs"]["DomainReceipt"]["properties"]["receipt_id"]["format"] = json!("uuid");
        schema["$defs"]["DomainReceipt"]["properties"]["author_command_admission_id"]["format"] =
            json!("uuid");
        schema["$defs"]["DomainReceipt"]["properties"]["created_at"]["format"] = json!("date-time");
    }
    json_bytes(&schema)
}

fn command_openapi() -> String {
    let request_schema = generated_ref(REQUEST_SCHEMA_PATH);
    let response_schema = generated_ref(RESPONSE_SCHEMA_PATH);
    let responses = response_block(CREATE_PROJECT.responses, response_schema, &[200]);
    format!(
        concat!(
            "  {}:\n    post:\n      operationId: {}\n      summary: Create one empty Project\n",
            "      parameters:\n        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        CREATE_PROJECT.path, CREATE_PROJECT.operation_id, request_schema, responses,
    )
}

pub(super) fn typescript_client_source() -> String {
    format!(
        concat!(
            "\nexport async function createProjectChallenge({{ request, ...options }} = {{}}) {{\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"createProjectChallenge requires request\");\n",
            "  return commandJson({{ ...options, path: \"{}\", body: request }});\n}}\n",
            "\nexport async function createProject({{ request, idempotencyKey, antiForgery, ...options }} = {{}}) {{\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"createProject requires request\");\n",
            "  if (typeof idempotencyKey !== \"string\" || typeof antiForgery !== \"string\") throw new TypeError(\"createProject requires security bindings\");\n",
            "  return commandJson({{ ...options, path: \"{}\", body: request, commandHeaders: {{ \"idempotency-key\": idempotencyKey, \"x-storyos-anti-forgery\": antiForgery }} }});\n}}\n",
        ),
        CREATE_PROJECT_CHALLENGE.path, CREATE_PROJECT.path,
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    "export declare function createProjectChallenge(options: StoryOSQueryOptions & { request: CreateProjectChallengeRequest }): Promise<CreateProjectChallengeResponse>;\nexport declare function createProject(options: StoryOSQueryOptions & { request: CreateProjectRequest; idempotencyKey: string; antiForgery: string }): Promise<CreateProjectResponse>;\n"
}

pub(super) fn challenge_fixture_bytes() -> Vec<u8> {
    json_bytes(&challenge_fixture("2026-08-23T08:05:00.000Z"))
}

pub(super) fn challenge_invalid_fixture_bytes() -> Vec<u8> {
    let mut value = challenge_fixture("2026-08-23T08:05:00.000Z");
    value
        .as_object_mut()
        .expect("fixture is an object")
        .remove("prospective_project_id");
    json_bytes(&value)
}

pub(super) fn challenge_boundary_fixture_bytes() -> Vec<u8> {
    json_bytes(&challenge_fixture("1970-01-01T00:00:00.000Z"))
}

fn challenge_fixture(expires_at: &str) -> Value {
    json!({
        "prospective_project_id": "018f0000-0000-7001-8000-000000000201",
        "canonical_command_digest": {
            "algorithm": "sha256",
            "profile": CREATE_PROJECT_DIGEST_PROFILE,
            "value_hex_lowercase": "d".repeat(64)
        },
        "nonce": "e".repeat(64),
        "expires_at": expires_at,
        "limit_profile_revision": crate::LIMIT_PROFILE_REVISION
    })
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&command_fixture("2026-08-25T08:05:00.000Z"))
}

pub(super) fn invalid_fixture_bytes() -> Vec<u8> {
    let mut value = command_fixture("2026-08-25T08:05:00.000Z");
    value
        .as_object_mut()
        .expect("fixture is an object")
        .remove("project");
    json_bytes(&value)
}

pub(super) fn boundary_fixture_bytes() -> Vec<u8> {
    json_bytes(&command_fixture("1970-01-01T00:00:00.000Z"))
}

fn command_fixture(created_at: &str) -> Value {
    let project_scope = json!({
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
    });
    json!({
        "schema_id": CREATE_PROJECT_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000210",
        "project_scope": project_scope,
        "command_id": "018f0000-0000-7001-8000-000000000211",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000212",
        "receipt": {
            "receipt_id": "018f0000-0000-7001-8000-000000000213",
            "project_scope": project_scope,
            "command_kind": "createProject",
            "command_digest": {
                "algorithm": "sha256",
                "profile": CREATE_PROJECT_DIGEST_PROFILE,
                "value_hex_lowercase": "c".repeat(64)
            },
            "idempotency_key": "018f0000-0000-7001-8000-000000000214",
            "producer_cause": "author_command_admission",
            "author_command_admission_id": "018f0000-0000-7001-8000-000000000212",
            "expected_heads": [],
            "prior_heads": [],
            "resulting_heads": [],
            "authoritative_revision_ids": [],
            "proposal_revision_ids": [],
            "authoritative_commit_ids": [],
            "author_action_sequence": null,
            "draft_artifact_refs": [],
            "artifact_lifecycle_event_refs": [],
            "condition_refs": [],
            "result": "authoritative_applied",
            "created_at": created_at
        },
        "project": {
            "project_id": "018f0000-0000-7001-8000-000000000201",
            "title": "Empty Novel",
            "open": {"kind": "empty"}
        }
    })
}

fn response_block(
    responses: &[(u16, &str)],
    response_schema: &str,
    success_statuses: &[u16],
) -> String {
    responses
        .iter()
        .map(|(status, description)| {
            let retry_after = if *status == 429 {
                "          headers:\n            Retry-After:\n              required: true\n              schema:\n                type: integer\n                minimum: 1\n                maximum: 60\n"
            } else {
                ""
            };
            let content = if success_statuses.contains(status) {
                format!(
                    "          content:\n            application/json:\n              schema:\n                $ref: '../{response_schema}'\n"
                )
            } else {
                String::new()
            };
            format!(
                "        '{status}':\n          description: {description}\n{retry_after}{content}"
            )
        })
        .collect()
}

fn generated_ref(path: &str) -> &str {
    path.strip_prefix("generated/")
        .expect("schema is a generated artifact")
}

fn schema_value<T: schemars::JsonSchema>(schema_id: &str, title: &str) -> Value {
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
