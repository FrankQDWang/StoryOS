use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_agent_run::AgentRunStatus;
use crate::release1_agent_run_control::{
    CANCEL_AGENT_RUN, CANCEL_AGENT_RUN_DIGEST_PROFILE, CANCEL_AGENT_RUN_REQUEST_SCHEMA_ID,
    CANCEL_AGENT_RUN_RESPONSE_SCHEMA_ID, CancelAgentRunConflictReason, CancelAgentRunEffect,
    CancelAgentRunInput, CancelAgentRunNoEffectReason, CancelAgentRunRequest,
    CancelAgentRunResponse, PAUSE_AGENT_RUN, PAUSE_AGENT_RUN_DIGEST_PROFILE,
    PAUSE_AGENT_RUN_REQUEST_SCHEMA_ID, PAUSE_AGENT_RUN_RESPONSE_SCHEMA_ID,
    PauseAgentRunConflictReason, PauseAgentRunEffect, PauseAgentRunInput,
    PauseAgentRunNoEffectReason, PauseAgentRunRequest, PauseAgentRunResponse, STEER_AGENT_RUN,
    STEER_AGENT_RUN_DIGEST_PROFILE, STEER_AGENT_RUN_REQUEST_SCHEMA_ID,
    STEER_AGENT_RUN_RESPONSE_SCHEMA_ID, SteerAgentRunEffect, SteerAgentRunInput,
    SteerAgentRunRequest, SteerAgentRunResponse,
};
use crate::release1_operation_registry::{
    OperationArtifacts, RegisteredOperation, fixture_triple, operation_schemas, path_items,
};
use crate::release1_wire::{generated_ref, json_bytes, schema_value};

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[
        RegisteredOperation::command(
            &STEER_AGENT_RUN,
            &[
                "server_derived_project_scope",
                "run_scope_join",
                "exact_conversation",
                "nonterminal_run",
                "bounded_author_input",
            ],
        ),
        RegisteredOperation::command(
            &PAUSE_AGENT_RUN,
            &[
                "server_derived_project_scope",
                "run_scope_join",
                "current_run_state_pauseable",
                "current_fence_generation",
            ],
        ),
        RegisteredOperation::command(
            &CANCEL_AGENT_RUN,
            &[
                "server_derived_project_scope",
                "run_scope_join",
                "current_run_state_cancellable",
                "current_fence_generation",
            ],
        ),
    ],
    schemas: || {
        [
            operation_schemas(
                &STEER_AGENT_RUN,
                (STEER_REQUEST_SCHEMA_PATH, steer_request_schema_bytes()),
                (STEER_RESPONSE_SCHEMA_PATH, steer_response_schema_bytes()),
            ),
            operation_schemas(
                &PAUSE_AGENT_RUN,
                (PAUSE_REQUEST_SCHEMA_PATH, pause_request_schema_bytes()),
                (PAUSE_RESPONSE_SCHEMA_PATH, pause_response_schema_bytes()),
            ),
            operation_schemas(
                &CANCEL_AGENT_RUN,
                (CANCEL_REQUEST_SCHEMA_PATH, cancel_request_schema_bytes()),
                (CANCEL_RESPONSE_SCHEMA_PATH, cancel_response_schema_bytes()),
            ),
        ]
        .into_iter()
        .flatten()
        .collect()
    },
    openapi: || path_items(openapi()),
    typescript_types: typescript_type_declarations,
    typescript_client: typescript_client_source,
    typescript_declarations,
    fixtures: || {
        [
            fixture_triple(
                STEER_FIXTURE_PATHS,
                &STEER_AGENT_RUN,
                [
                    |_| steer_fixture_bytes(),
                    |_| steer_invalid_fixture_bytes(),
                    |_| steer_fixture_bytes(),
                ],
            ),
            fixture_triple(
                PAUSE_FIXTURE_PATHS,
                &PAUSE_AGENT_RUN,
                [
                    |_| pause_fixture_bytes(),
                    |_| pause_invalid_fixture_bytes(),
                    |_| pause_boundary_fixture_bytes(),
                ],
            ),
            fixture_triple(
                CANCEL_FIXTURE_PATHS,
                &CANCEL_AGENT_RUN,
                [
                    |_| cancel_fixture_bytes(),
                    |_| cancel_invalid_fixture_bytes(),
                    |_| cancel_boundary_fixture_bytes(),
                ],
            ),
        ]
        .concat()
    },
};

pub(super) const PAUSE_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/pause-agent-run-request.schema.json";
pub(super) const PAUSE_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/pause-agent-run-response.schema.json";
pub(super) const CANCEL_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/cancel-agent-run-request.schema.json";
pub(super) const CANCEL_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/cancel-agent-run-response.schema.json";
pub(super) const PAUSE_FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/pause-agent-run.json",
    "generated/golden-wire/storyos-public-release-1/pause-agent-run.invalid.json",
    "generated/golden-wire/storyos-public-release-1/pause-agent-run.boundary.json",
];
pub(super) const CANCEL_FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/cancel-agent-run.json",
    "generated/golden-wire/storyos-public-release-1/cancel-agent-run.invalid.json",
    "generated/golden-wire/storyos-public-release-1/cancel-agent-run.boundary.json",
];

pub(super) fn pause_request_schema_bytes() -> Vec<u8> {
    request_schema_bytes::<PauseAgentRunRequest>(
        PAUSE_AGENT_RUN_REQUEST_SCHEMA_ID,
        "StoryOS Pause Agent Run Request",
    )
}

pub(super) fn pause_response_schema_bytes() -> Vec<u8> {
    response_schema_bytes::<PauseAgentRunResponse>(
        PAUSE_AGENT_RUN_RESPONSE_SCHEMA_ID,
        "StoryOS Pause Agent Run Response",
    )
}

pub(super) fn cancel_request_schema_bytes() -> Vec<u8> {
    request_schema_bytes::<CancelAgentRunRequest>(
        CANCEL_AGENT_RUN_REQUEST_SCHEMA_ID,
        "StoryOS Cancel Agent Run Request",
    )
}

pub(super) fn cancel_response_schema_bytes() -> Vec<u8> {
    response_schema_bytes::<CancelAgentRunResponse>(
        CANCEL_AGENT_RUN_RESPONSE_SCHEMA_ID,
        "StoryOS Cancel Agent Run Response",
    )
}

pub(super) fn openapi() -> String {
    format!(
        "{}{}{}",
        operation_openapi(
            &PAUSE_AGENT_RUN,
            "Pause one AgentRun without cancelling it",
            PAUSE_REQUEST_SCHEMA_PATH,
            PAUSE_RESPONSE_SCHEMA_PATH,
        ),
        operation_openapi(
            &CANCEL_AGENT_RUN,
            "Cancel one AgentRun after a durable fence",
            CANCEL_REQUEST_SCHEMA_PATH,
            CANCEL_RESPONSE_SCHEMA_PATH,
        ),
        operation_openapi(
            &STEER_AGENT_RUN,
            "Retain guidance for the next safe decision",
            STEER_REQUEST_SCHEMA_PATH,
            STEER_RESPONSE_SCHEMA_PATH
        )
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    let declarations = format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        PauseAgentRunInput::decl(&config),
        PauseAgentRunRequest::decl(&config),
        PauseAgentRunNoEffectReason::decl(&config),
        PauseAgentRunConflictReason::decl(&config),
        PauseAgentRunEffect::decl(&config),
        PauseAgentRunResponse::decl(&config),
        CancelAgentRunInput::decl(&config),
        CancelAgentRunRequest::decl(&config),
        CancelAgentRunNoEffectReason::decl(&config),
        CancelAgentRunConflictReason::decl(&config),
        CancelAgentRunEffect::decl(&config),
        CancelAgentRunResponse::decl(&config),
    );
    format!(
        "{declarations}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        SteerAgentRunInput::decl(&config),
        SteerAgentRunRequest::decl(&config),
        SteerAgentRunEffect::decl(&config),
        SteerAgentRunResponse::decl(&config)
    )
}

pub(super) fn typescript_client_source() -> String {
    let existing = format!(
        concat!(
            "\nexport async function digestPauseAgentRun(request, cryptoImpl = globalThis.crypto) {{\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"digestPauseAgentRun requires request\");\n",
            "  const canonical = canonicalJson(request);\n",
            "  const bytes = new TextEncoder().encode(JSON.stringify(canonical));\n",
            "  const digest = new Uint8Array(await cryptoImpl.subtle.digest(\"SHA-256\", bytes));\n",
            "  return {{ algorithm: \"sha256\", profile: \"{}\", value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, \"0\")).join(\"\") }};\n}}\n",
            "\nexport async function pauseAgentRun({{ projectId, runId, request, idempotencyKey, antiForgery, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"pauseAgentRun requires projectId\");\n",
            "  if (typeof runId !== \"string\" || runId.length === 0) throw new TypeError(\"pauseAgentRun requires runId\");\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"pauseAgentRun requires request\");\n",
            "  if (typeof idempotencyKey !== \"string\" || typeof antiForgery !== \"string\") throw new TypeError(\"pauseAgentRun requires security bindings\");\n",
            "  return commandJson({{ ...options, path: `{}`, body: request, commandHeaders: {{ \"idempotency-key\": idempotencyKey, \"x-storyos-anti-forgery\": antiForgery }} }});\n}}\n",
            "\nexport async function digestCancelAgentRun(request, cryptoImpl = globalThis.crypto) {{\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"digestCancelAgentRun requires request\");\n",
            "  const canonical = canonicalJson(request);\n",
            "  const bytes = new TextEncoder().encode(JSON.stringify(canonical));\n",
            "  const digest = new Uint8Array(await cryptoImpl.subtle.digest(\"SHA-256\", bytes));\n",
            "  return {{ algorithm: \"sha256\", profile: \"{}\", value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, \"0\")).join(\"\") }};\n}}\n",
            "\nexport async function cancelAgentRun({{ projectId, runId, request, idempotencyKey, antiForgery, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"cancelAgentRun requires projectId\");\n",
            "  if (typeof runId !== \"string\" || runId.length === 0) throw new TypeError(\"cancelAgentRun requires runId\");\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"cancelAgentRun requires request\");\n",
            "  if (typeof idempotencyKey !== \"string\" || typeof antiForgery !== \"string\") throw new TypeError(\"cancelAgentRun requires security bindings\");\n",
            "  return commandJson({{ ...options, path: `{}`, body: request, commandHeaders: {{ \"idempotency-key\": idempotencyKey, \"x-storyos-anti-forgery\": antiForgery }} }});\n}}\n",
        ),
        PAUSE_AGENT_RUN_DIGEST_PROFILE,
        PAUSE_AGENT_RUN
            .path
            .replace("{project_id}", "${encodeURIComponent(projectId)}")
            .replace("{run_id}", "${encodeURIComponent(runId)}"),
        CANCEL_AGENT_RUN_DIGEST_PROFILE,
        CANCEL_AGENT_RUN
            .path
            .replace("{project_id}", "${encodeURIComponent(projectId)}")
            .replace("{run_id}", "${encodeURIComponent(runId)}"),
    );
    format!(
        r#"{existing}
export async function digestSteerAgentRun(request, cryptoImpl = globalThis.crypto) {{
  if (!request || typeof request !== "object") throw new TypeError("digestSteerAgentRun requires request");
  const bytes = new TextEncoder().encode(JSON.stringify(canonicalJson(request)));
  const digest = new Uint8Array(await cryptoImpl.subtle.digest("SHA-256", bytes));
  return {{ algorithm: "sha256", profile: "{STEER_AGENT_RUN_DIGEST_PROFILE}", value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join("") }};
}}
export async function steerAgentRun({{ projectId, runId, request, idempotencyKey, antiForgery, ...options }} = {{}}) {{
  if (typeof projectId !== "string" || typeof runId !== "string" || !request || typeof idempotencyKey !== "string" || typeof antiForgery !== "string") throw new TypeError("steerAgentRun requires Scope, input and security bindings");
  return commandJson({{ ...options, path: `/api/v1/projects/${{encodeURIComponent(projectId)}}/agent-runs/${{encodeURIComponent(runId)}}/steering-inputs`, body: request, commandHeaders: {{ "idempotency-key": idempotencyKey, "x-storyos-anti-forgery": antiForgery }} }});
}}
"#
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    concat!(
        "export declare function digestSteerAgentRun(request: SteerAgentRunRequest, cryptoImpl?: Crypto): Promise<DigestValue>;\n",
        "export declare function steerAgentRun(options: StoryOSQueryOptions & { projectId: string; runId: string; request: SteerAgentRunRequest; idempotencyKey: string; antiForgery: string }): Promise<SteerAgentRunResponse>;\n",
        "export declare function digestPauseAgentRun(request: PauseAgentRunRequest, cryptoImpl?: Crypto): Promise<DigestValue>;\n",
        "export declare function pauseAgentRun(options: StoryOSQueryOptions & { projectId: string; runId: string; request: PauseAgentRunRequest; idempotencyKey: string; antiForgery: string }): Promise<PauseAgentRunResponse>;\n",
        "export declare function digestCancelAgentRun(request: CancelAgentRunRequest, cryptoImpl?: Crypto): Promise<DigestValue>;\n",
        "export declare function cancelAgentRun(options: StoryOSQueryOptions & { projectId: string; runId: string; request: CancelAgentRunRequest; idempotencyKey: string; antiForgery: string }): Promise<CancelAgentRunResponse>;\n",
    )
}

pub(super) fn pause_fixture_bytes() -> Vec<u8> {
    json_bytes(&control_fixture(
        PAUSE_AGENT_RUN_RESPONSE_SCHEMA_ID,
        "pauseAgentRun",
        PAUSE_AGENT_RUN_DIGEST_PROFILE,
        "018f0000-0000-7001-8000-000000000b01",
        AgentRunStatus::Paused,
        "1",
    ))
}

pub(super) fn pause_invalid_fixture_bytes() -> Vec<u8> {
    let mut value = control_fixture(
        PAUSE_AGENT_RUN_RESPONSE_SCHEMA_ID,
        "pauseAgentRun",
        PAUSE_AGENT_RUN_DIGEST_PROFILE,
        "018f0000-0000-7001-8000-000000000b01",
        AgentRunStatus::Paused,
        "1",
    );
    value
        .as_object_mut()
        .expect("fixture is an object")
        .remove("effect");
    json_bytes(&value)
}

pub(super) fn pause_boundary_fixture_bytes() -> Vec<u8> {
    pause_fixture_bytes()
}

pub(super) fn cancel_fixture_bytes() -> Vec<u8> {
    json_bytes(&control_fixture(
        CANCEL_AGENT_RUN_RESPONSE_SCHEMA_ID,
        "cancelAgentRun",
        CANCEL_AGENT_RUN_DIGEST_PROFILE,
        "018f0000-0000-7001-8000-000000000b11",
        AgentRunStatus::Cancelled,
        "1",
    ))
}

pub(super) fn cancel_invalid_fixture_bytes() -> Vec<u8> {
    let mut value = control_fixture(
        CANCEL_AGENT_RUN_RESPONSE_SCHEMA_ID,
        "cancelAgentRun",
        CANCEL_AGENT_RUN_DIGEST_PROFILE,
        "018f0000-0000-7001-8000-000000000b11",
        AgentRunStatus::Cancelled,
        "1",
    );
    value
        .as_object_mut()
        .expect("fixture is an object")
        .remove("effect");
    json_bytes(&value)
}

pub(super) fn cancel_boundary_fixture_bytes() -> Vec<u8> {
    cancel_fixture_bytes()
}

fn request_schema_bytes<T: schemars::JsonSchema>(schema_id: &str, title: &str) -> Vec<u8> {
    let mut schema = schema_value::<T>(schema_id, title);
    schema["properties"]["command_schema"]["const"] = json!(schema_id);
    if let Some(input) = schema["$defs"].get_mut("PauseAgentRunInput") {
        input["properties"]["correlation_id"]["format"] = json!("uuid");
    }
    if let Some(input) = schema["$defs"].get_mut("CancelAgentRunInput") {
        input["properties"]["correlation_id"]["format"] = json!("uuid");
    }
    json_bytes(&schema)
}

fn response_schema_bytes<T: schemars::JsonSchema>(schema_id: &str, title: &str) -> Vec<u8> {
    let mut schema = schema_value::<T>(schema_id, title);
    schema["properties"]["schema_id"]["const"] = json!(schema_id);
    for field in [
        "correlation_id",
        "command_id",
        "author_command_admission_id",
    ] {
        schema["properties"][field]["format"] = json!("uuid");
    }
    if let Some(scope) = schema["$defs"].get_mut("ProjectScope") {
        scope["properties"]["owner_user_id"]["format"] = json!("uuid");
        scope["properties"]["project_id"]["format"] = json!("uuid");
    }
    for name in [
        "PauseAgentRunEffect",
        "CancelAgentRunEffect",
        "SteerAgentRunEffect",
    ] {
        if let Some(effect) = schema["$defs"].get_mut(name) {
            constrain_uuid_fields(effect, &["run_id", "steering_input_id"]);
        }
    }
    json_bytes(&schema)
}

fn operation_openapi(
    operation: &crate::release1::QueryOperation,
    summary: &str,
    request_path: &str,
    response_path: &str,
) -> String {
    let request = generated_ref(request_path);
    let response = generated_ref(response_path);
    let responses = status_block(operation.responses, response);
    format!(
        concat!(
            "  {}:\n    post:\n      operationId: {}\n      summary: {}\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: run_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        operation.path, operation.operation_id, summary, request, responses,
    )
}

fn control_fixture(
    schema_id: &str,
    command_kind: &str,
    digest_profile: &str,
    receipt_stem: &str,
    status: AgentRunStatus,
    fence_generation: &str,
) -> Value {
    let status = match status {
        AgentRunStatus::Paused => "paused",
        AgentRunStatus::Cancelled => "cancelled",
        AgentRunStatus::Queued
        | AgentRunStatus::Claimed
        | AgentRunStatus::Waiting
        | AgentRunStatus::Completed
        | AgentRunStatus::Refused => "queued",
    };
    json!({
        "schema_id": schema_id,
        "correlation_id": receipt_stem,
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "command_id": "018f0000-0000-7001-8000-000000000b02",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000b03",
        "receipt": {
            "receipt_id": "018f0000-0000-7001-8000-000000000b04",
            "project_scope": {
                "owner_user_id": "018f0000-0000-7001-8000-000000000001",
                "project_id": "018f0000-0000-7001-8000-000000000201"
            },
            "command_kind": command_kind,
            "command_digest": {
                "algorithm": "sha256",
                "profile": digest_profile,
                "value_hex_lowercase": "c".repeat(64)
            },
            "idempotency_key": "018f0000-0000-7001-8000-000000000b05",
            "producer_cause": "author_command_admission",
            "author_command_admission_id": "018f0000-0000-7001-8000-000000000b03",
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
            "created_at": "2026-09-20T01:00:00.000Z"
        },
        "project": {
            "project_id": "018f0000-0000-7001-8000-000000000201",
            "title": "Empty Novel",
            "open": {"kind": "empty"}
        },
        "effect": {
            "kind": "applied",
            "run_id": "018f0000-0000-7001-8000-000000000a34",
            "status": status,
            "fence_generation": fence_generation,
            "project_activity_position": "2"
        }
    })
}

fn constrain_uuid_fields(value: &mut Value, fields: &[&str]) {
    match value {
        Value::Object(map) => {
            if let Some(Value::Object(properties)) = map.get_mut("properties") {
                for field in fields {
                    if let Some(property) = properties.get_mut(*field) {
                        property["format"] = json!("uuid");
                    }
                }
            }
            if let Some(Value::Array(one_of)) = map.get_mut("oneOf") {
                for variant in one_of {
                    constrain_uuid_fields(variant, fields);
                }
            }
            if let Some(Value::Array(any_of)) = map.get_mut("anyOf") {
                for variant in any_of {
                    constrain_uuid_fields(variant, fields);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                constrain_uuid_fields(value, fields);
            }
        }
        _ => {}
    }
}

fn status_block(responses: &[(u16, &str)], response_schema: &str) -> String {
    responses
        .iter()
        .map(|(status, description)| {
            let retry_after = if *status == 429 {
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

pub(super) const STEER_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/steer-agent-run-request.schema.json";
pub(super) const STEER_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/steer-agent-run-response.schema.json";
pub(super) const STEER_FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/steer-agent-run.json",
    "generated/golden-wire/storyos-public-release-1/steer-agent-run.invalid.json",
    "generated/golden-wire/storyos-public-release-1/steer-agent-run.boundary.json",
];
pub(super) fn steer_request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<SteerAgentRunRequest>(
        STEER_AGENT_RUN_REQUEST_SCHEMA_ID,
        "StoryOS Steering Input Request",
    );
    schema["properties"]["command_schema"]["const"] = json!(STEER_AGENT_RUN_REQUEST_SCHEMA_ID);
    for field in ["conversation_id", "correlation_id"] {
        schema["$defs"]["SteerAgentRunInput"]["properties"][field]["format"] = json!("uuid");
    }
    schema["$defs"]["AuthorMessage"]["properties"]["text"]["minLength"] = json!(1);
    schema["$defs"]["AuthorMessage"]["properties"]["text"]["maxLength"] = json!(8000);
    json_bytes(&schema)
}
pub(super) fn steer_response_schema_bytes() -> Vec<u8> {
    response_schema_bytes::<SteerAgentRunResponse>(
        STEER_AGENT_RUN_RESPONSE_SCHEMA_ID,
        "StoryOS Steering Input Response",
    )
}
pub(super) fn steer_fixture_bytes() -> Vec<u8> {
    let mut value: Value =
        serde_json::from_slice(&pause_fixture_bytes()).expect("control fixture is JSON");
    value["schema_id"] = json!(STEER_AGENT_RUN_RESPONSE_SCHEMA_ID);
    value["receipt"]["command_kind"] = json!("steerAgentRun");
    value["receipt"]["command_digest"]["profile"] = json!(STEER_AGENT_RUN_DIGEST_PROFILE);
    value["receipt"]["result"] = json!("no_effect");
    value["effect"] = json!({"kind": "retained", "run_id": "018f0000-0000-7001-8000-000000000b01", "steering_input_id": "018f0000-0000-7001-8000-000000000b02", "input_position": "1"});
    json_bytes(&value)
}
pub(super) fn steer_invalid_fixture_bytes() -> Vec<u8> {
    let mut value: Value =
        serde_json::from_slice(&steer_fixture_bytes()).expect("control fixture is JSON");
    value
        .as_object_mut()
        .expect("fixture is an object")
        .remove("effect");
    json_bytes(&value)
}
