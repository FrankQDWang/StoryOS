use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_project_assistance::{
    GET_PROJECT_ASSISTANCE, GET_PROJECT_ASSISTANCE_REQUEST_SCHEMA_ID,
    GET_PROJECT_ASSISTANCE_RESPONSE_SCHEMA_ID, GetProjectAssistanceResponse,
    ProjectAssistanceAvailability, ProjectAssistanceBinding, UPDATE_PROJECT_ASSISTANCE,
    UPDATE_PROJECT_ASSISTANCE_DIGEST_PROFILE, UPDATE_PROJECT_ASSISTANCE_REQUEST_SCHEMA_ID,
    UPDATE_PROJECT_ASSISTANCE_RESPONSE_SCHEMA_ID, UpdateProjectAssistanceConflictReason,
    UpdateProjectAssistanceEffect, UpdateProjectAssistanceInput,
    UpdateProjectAssistanceNoEffectReason, UpdateProjectAssistanceRequest,
    UpdateProjectAssistanceResponse,
};

pub(super) const GET_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/project-assistance-request.schema.json";
pub(super) const GET_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/project-assistance-response.schema.json";
pub(super) const UPDATE_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/update-project-assistance-request.schema.json";
pub(super) const UPDATE_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/update-project-assistance-response.schema.json";
pub(super) const GET_FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/get-project-assistance.json",
    "generated/golden-wire/storyos-public-release-1/get-project-assistance.invalid.json",
    "generated/golden-wire/storyos-public-release-1/get-project-assistance.boundary.json",
];
pub(super) const UPDATE_FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/update-project-assistance.json",
    "generated/golden-wire/storyos-public-release-1/update-project-assistance.invalid.json",
    "generated/golden-wire/storyos-public-release-1/update-project-assistance.boundary.json",
];

const U64_WIRE: &str = "^(?:0|[1-9][0-9]{0,18}|1[0-7][0-9]{18}|18[0-3][0-9]{17}|184[0-3][0-9]{16}|1844[0-5][0-9]{15}|18446[0-6][0-9]{14}|184467[0-3][0-9]{13}|1844674[0-3][0-9]{12}|184467440[0-6][0-9]{10}|1844674407[0-2][0-9]{9}|18446744073[0-6][0-9]{8}|1844674407370[0-8][0-9]{6}|18446744073709[0-4][0-9]{5}|184467440737095[0-4][0-9]{3}|1844674407370955[0-9]{2}|18446744073709551[0-5]|1844674407370955160|1844674407370955161[0-5])$";

pub(super) fn get_request_schema_bytes() -> Vec<u8> {
    json_bytes(&json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": GET_PROJECT_ASSISTANCE_REQUEST_SCHEMA_ID,
        "title": "StoryOS Project Assistance Request",
        "type": "object",
        "additionalProperties": false,
        "maxProperties": 0
    }))
}

pub(super) fn get_response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<GetProjectAssistanceResponse>(
        GET_PROJECT_ASSISTANCE_RESPONSE_SCHEMA_ID,
        "StoryOS Project Assistance Response",
    );
    schema["properties"]["schema_id"]["const"] = json!(GET_PROJECT_ASSISTANCE_RESPONSE_SCHEMA_ID);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    constrain_binding(&mut schema);
    json_bytes(&schema)
}

pub(super) fn update_request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<UpdateProjectAssistanceRequest>(
        UPDATE_PROJECT_ASSISTANCE_REQUEST_SCHEMA_ID,
        "StoryOS Update Project Assistance Request",
    );
    schema["properties"]["command_schema"]["const"] =
        json!(UPDATE_PROJECT_ASSISTANCE_REQUEST_SCHEMA_ID);
    let input = &mut schema["$defs"]["UpdateProjectAssistanceInput"]["properties"];
    input["correlation_id"]["format"] = json!("uuid");
    input["expected_assistance_revision"] = json!({"type": "string", "pattern": U64_WIRE});
    json_bytes(&schema)
}

pub(super) fn update_response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<UpdateProjectAssistanceResponse>(
        UPDATE_PROJECT_ASSISTANCE_RESPONSE_SCHEMA_ID,
        "StoryOS Update Project Assistance Response",
    );
    schema["properties"]["schema_id"]["const"] =
        json!(UPDATE_PROJECT_ASSISTANCE_RESPONSE_SCHEMA_ID);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["command_id"]["format"] = json!("uuid");
    schema["properties"]["author_command_admission_id"]["format"] = json!("uuid");
    constrain_binding(&mut schema);
    json_bytes(&schema)
}

pub(super) fn openapi() -> String {
    let get_response = generated_ref(GET_RESPONSE_SCHEMA_PATH);
    let update_request = generated_ref(UPDATE_REQUEST_SCHEMA_PATH);
    let update_response = generated_ref(UPDATE_RESPONSE_SCHEMA_PATH);
    let get_responses = status_block(
        GET_PROJECT_ASSISTANCE.responses,
        get_response,
        /*command*/ false,
    );
    let update_responses = status_block(
        UPDATE_PROJECT_ASSISTANCE.responses,
        update_response,
        /*command*/ true,
    );
    format!(
        concat!(
            "  {}:\n    get:\n      operationId: {}\n      summary: Inspect Project assistance and its Host-owned fake binding\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "      responses:\n{}",
            "    put:\n      operationId: {}\n      summary: Prepare or change Project assistance availability\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        GET_PROJECT_ASSISTANCE.path,
        GET_PROJECT_ASSISTANCE.operation_id,
        get_responses,
        UPDATE_PROJECT_ASSISTANCE.operation_id,
        update_request,
        update_responses,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        ProjectAssistanceAvailability::decl(&config),
        ProjectAssistanceBinding::decl(&config),
        GetProjectAssistanceResponse::decl(&config),
        UpdateProjectAssistanceInput::decl(&config),
        UpdateProjectAssistanceRequest::decl(&config),
        UpdateProjectAssistanceNoEffectReason::decl(&config),
        UpdateProjectAssistanceConflictReason::decl(&config),
        UpdateProjectAssistanceEffect::decl(&config),
        UpdateProjectAssistanceResponse::decl(&config),
    )
}

pub(super) fn typescript_client_source() -> String {
    format!(
        concat!(
            "\nexport async function getProjectAssistance({{ projectId, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"getProjectAssistance requires projectId\");\n",
            "  return queryJson({{ ...options, path: `{}` }});\n}}\n",
            "\nexport async function digestUpdateProjectAssistance(request, cryptoImpl = globalThis.crypto) {{\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"digestUpdateProjectAssistance requires request\");\n",
            "  const canonical = canonicalJson(request);\n",
            "  const bytes = new TextEncoder().encode(JSON.stringify(canonical));\n",
            "  const digest = new Uint8Array(await cryptoImpl.subtle.digest(\"SHA-256\", bytes));\n",
            "  return {{ algorithm: \"sha256\", profile: \"{}\", value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, \"0\")).join(\"\") }};\n}}\n",
            "\nexport async function updateProjectAssistance({{ projectId, request, idempotencyKey, antiForgery, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"updateProjectAssistance requires projectId\");\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"updateProjectAssistance requires request\");\n",
            "  if (typeof idempotencyKey !== \"string\" || typeof antiForgery !== \"string\") throw new TypeError(\"updateProjectAssistance requires security bindings\");\n",
            "  return commandJson({{ ...options, method: \"PUT\", path: `{}`, body: request, commandHeaders: {{ \"idempotency-key\": idempotencyKey, \"x-storyos-anti-forgery\": antiForgery }} }});\n}}\n",
        ),
        GET_PROJECT_ASSISTANCE
            .path
            .replace("{project_id}", "${encodeURIComponent(projectId)}"),
        UPDATE_PROJECT_ASSISTANCE_DIGEST_PROFILE,
        UPDATE_PROJECT_ASSISTANCE
            .path
            .replace("{project_id}", "${encodeURIComponent(projectId)}"),
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    concat!(
        "export declare function getProjectAssistance(options: StoryOSQueryOptions & { projectId: string }): Promise<GetProjectAssistanceResponse>;\n",
        "export declare function digestUpdateProjectAssistance(request: UpdateProjectAssistanceRequest, cryptoImpl?: Crypto): Promise<DigestValue>;\n",
        "export declare function updateProjectAssistance(options: StoryOSQueryOptions & { projectId: string; request: UpdateProjectAssistanceRequest; idempotencyKey: string; antiForgery: string }): Promise<UpdateProjectAssistanceResponse>;\n",
    )
}

pub(super) fn get_fixture_bytes() -> Vec<u8> {
    json_bytes(&get_fixture())
}

pub(super) fn get_invalid_fixture_bytes() -> Vec<u8> {
    let mut value = get_fixture();
    value
        .as_object_mut()
        .expect("fixture is an object")
        .remove("assistance");
    json_bytes(&value)
}

pub(super) fn get_boundary_fixture_bytes() -> Vec<u8> {
    json_bytes(&get_fixture())
}

pub(super) fn update_fixture_bytes() -> Vec<u8> {
    json_bytes(&update_fixture("2026-09-16T03:00:00.000Z"))
}

pub(super) fn update_invalid_fixture_bytes() -> Vec<u8> {
    let mut value = update_fixture("2026-09-16T03:00:00.000Z");
    value
        .as_object_mut()
        .expect("fixture is an object")
        .remove("assistance");
    json_bytes(&value)
}

pub(super) fn update_boundary_fixture_bytes() -> Vec<u8> {
    json_bytes(&update_fixture("1970-01-01T00:00:00.000Z"))
}

fn constrain_binding(schema: &mut Value) {
    if let Some(binding) = schema["$defs"].get_mut("ProjectAssistanceBinding") {
        let properties = &mut binding["properties"];
        properties["revision"] = json!({"type": "string", "pattern": U64_WIRE});
        properties["processing_destination_identity_evidence_revision"] =
            json!({"type": "string", "pattern": U64_WIRE});
        properties["model_registration_revision"]["format"] = json!("uuid");
        properties["processing_destination_identity"]["format"] = json!("uuid");
        properties["project_model_use_binding_revision"]["format"] = json!("uuid");
        properties["external_compatibility_decision"]["format"] = json!("uuid");
    }
    if let Some(scope) = schema["$defs"].get_mut("ProjectScope") {
        scope["properties"]["owner_user_id"]["format"] = json!("uuid");
        scope["properties"]["project_id"]["format"] = json!("uuid");
    }
}

fn assistance_binding() -> Value {
    json!({
        "availability": "available",
        "revision": "1",
        "model_registration_revision": "018f0000-0000-7001-8000-000000000901",
        "processing_destination_identity": "018f0000-0000-7001-8000-000000000902",
        "processing_destination_identity_evidence_revision": "1",
        "project_model_use_binding_revision": "018f0000-0000-7001-8000-000000000903",
        "external_compatibility_decision": "018f0000-0000-7001-8000-000000000904"
    })
}

fn get_fixture() -> Value {
    json!({
        "schema_id": GET_PROJECT_ASSISTANCE_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000920",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "assistance": assistance_binding()
    })
}

fn update_fixture(created_at: &str) -> Value {
    json!({
        "schema_id": UPDATE_PROJECT_ASSISTANCE_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000921",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "command_id": "018f0000-0000-7001-8000-000000000922",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000923",
        "receipt": {
            "receipt_id": "018f0000-0000-7001-8000-000000000924",
            "project_scope": {
                "owner_user_id": "018f0000-0000-7001-8000-000000000001",
                "project_id": "018f0000-0000-7001-8000-000000000201"
            },
            "command_kind": "updateProjectAssistance",
            "command_digest": {
                "algorithm": "sha256",
                "profile": UPDATE_PROJECT_ASSISTANCE_DIGEST_PROFILE,
                "value_hex_lowercase": "c".repeat(64)
            },
            "idempotency_key": "018f0000-0000-7001-8000-000000000925",
            "producer_cause": "author_command_admission",
            "author_command_admission_id": "018f0000-0000-7001-8000-000000000923",
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
        },
        "assistance": assistance_binding(),
        "effect": {
            "kind": "initialized",
            "availability": "available",
            "revision": "1",
            "project_activity_position": "1"
        }
    })
}

fn status_block(responses: &[(u16, &str)], response_schema: &str, command: bool) -> String {
    responses
        .iter()
        .map(|(status, description)| {
            let retry_after = if command && *status == 429 {
                "          headers:\n            Retry-After:\n              required: true\n              schema:\n                type: integer\n                minimum: 1\n                maximum: 60\n"
            } else {
                ""
            };
            let content = if *status == 200 {
                format!(
                    "          content:\n            application/json:\n              schema:\n                $ref: '../{response_schema}'\n"
                )
            } else {
                String::new()
            };
            format!("        '{status}':\n          description: {description}\n{retry_after}{content}")
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
