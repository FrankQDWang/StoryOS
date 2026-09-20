use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_agent_run::{
    AgentRunContextInspect, AgentRunRef, AgentRunStatus, AgentRunStreamItemInspect,
    AgentRunUsageInspect, AssistanceCause, AssistanceWorkingTarget, AttemptEvidence, AuthorMessage,
    CREATE_AGENT_RUN, CREATE_AGENT_RUN_DIGEST_PROFILE, CREATE_AGENT_RUN_REQUEST_SCHEMA_ID,
    CREATE_AGENT_RUN_RESPONSE_SCHEMA_ID, ContextBlockReason, ContextProjectionInspect,
    ContextPurpose, ContextRejectionInspect, ContextRejectionReason, ContextSourceClass,
    ContextSourceInspect, ContextSufficiency, ContinuationAdmissionInspect,
    ContinuationInputMappingInspect, ConversationSelection, CreateAgentRunEffect,
    CreateAgentRunInput, CreateAgentRunRequest, CreateAgentRunResponse, CurrentAvailabilityInspect,
    DestinationIo, EvidenceAvailability, GET_AGENT_RUN, GET_AGENT_RUN_REQUEST_SCHEMA_ID,
    GET_AGENT_RUN_RESPONSE_SCHEMA_ID, GetAgentRunRequest, GetAgentRunResponse, HostControlInspect,
    InstructionBinding, OptionalContinuationInspect, OptionalDecisionInspect, OptionalManifestRef,
    OptionalModelAttemptInspect, OptionalOpenedProposalInspect, ProjectionMode, SourceAvailability,
    TokenCountingProfileInspect,
};

pub(super) const CREATE_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/create-agent-run-request.schema.json";
pub(super) const CREATE_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/create-agent-run-response.schema.json";
pub(super) const GET_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/agent-run-request.schema.json";
pub(super) const GET_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/agent-run-response.schema.json";
pub(super) const CREATE_FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/create-agent-run.json",
    "generated/golden-wire/storyos-public-release-1/create-agent-run.invalid.json",
    "generated/golden-wire/storyos-public-release-1/create-agent-run.boundary.json",
];
pub(super) const GET_FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/get-agent-run.json",
    "generated/golden-wire/storyos-public-release-1/get-agent-run.invalid.json",
    "generated/golden-wire/storyos-public-release-1/get-agent-run.boundary.json",
];

const U64_WIRE: &str = "^(?:0|[1-9][0-9]{0,18}|1[0-7][0-9]{18}|18[0-3][0-9]{17}|184[0-3][0-9]{16}|1844[0-5][0-9]{15}|18446[0-6][0-9]{14}|184467[0-3][0-9]{13}|1844674[0-3][0-9]{12}|184467440[0-6][0-9]{10}|1844674407[0-2][0-9]{9}|18446744073[0-6][0-9]{8}|1844674407370[0-8][0-9]{6}|18446744073709[0-4][0-9]{5}|184467440737095[0-4][0-9]{3}|1844674407370955[0-9]{2}|18446744073709551[0-5]|1844674407370955160|1844674407370955161[0-5])$";

pub(super) fn create_request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<CreateAgentRunRequest>(
        CREATE_AGENT_RUN_REQUEST_SCHEMA_ID,
        "StoryOS Create Agent Run Request",
    );
    schema["properties"]["command_schema"]["const"] = json!(CREATE_AGENT_RUN_REQUEST_SCHEMA_ID);
    let input = &mut schema["$defs"]["CreateAgentRunInput"]["properties"];
    input["correlation_id"]["format"] = json!("uuid");
    if let Some(message) = schema["$defs"].get_mut("AuthorMessage") {
        message["properties"]["text"]["maxLength"] = json!(8000);
        message["properties"]["text"]["minLength"] = json!(1);
    }
    if let Some(existing) = schema["$defs"].get_mut("ConversationSelection") {
        constrain_uuid_fields(existing, &["conversation_id"]);
    }
    if let Some(target) = schema["$defs"].get_mut("AssistanceWorkingTarget") {
        constrain_uuid_fields(target, &["chapter_id"]);
    }
    json_bytes(&schema)
}

pub(super) fn create_response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<CreateAgentRunResponse>(
        CREATE_AGENT_RUN_RESPONSE_SCHEMA_ID,
        "StoryOS Create Agent Run Response",
    );
    schema["properties"]["schema_id"]["const"] = json!(CREATE_AGENT_RUN_RESPONSE_SCHEMA_ID);
    for field in [
        "correlation_id",
        "command_id",
        "author_command_admission_id",
        "project_agent_id",
        "conversation_id",
    ] {
        schema["properties"][field]["format"] = json!("uuid");
    }
    schema["properties"]["memory_settings_revision"]["format"] = json!("uuid");
    if let Some(effect) = schema["$defs"].get_mut("CreateAgentRunEffect") {
        constrain_uuid_fields(
            effect,
            &[
                "project_agent_id",
                "conversation_id",
                "run_id",
                "memory_settings_revision",
            ],
        );
        constrain_revision_fields(effect);
    }
    if let Some(run_ref) = schema["$defs"].get_mut("AgentRunRef") {
        constrain_uuid_fields(run_ref, &["run_id"]);
    }
    if let Some(scope) = schema["$defs"].get_mut("ProjectScope") {
        scope["properties"]["owner_user_id"]["format"] = json!("uuid");
        scope["properties"]["project_id"]["format"] = json!("uuid");
    }
    json_bytes(&schema)
}

pub(super) fn get_request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<GetAgentRunRequest>(
        GET_AGENT_RUN_REQUEST_SCHEMA_ID,
        "StoryOS Agent Run Request",
    );
    if let Some(properties) = schema.get_mut("properties")
        && properties.get("model_attempt_id").is_some()
    {
        properties["model_attempt_id"]["format"] = json!("uuid");
    }
    json_bytes(&schema)
}

pub(super) fn get_response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<GetAgentRunResponse>(
        GET_AGENT_RUN_RESPONSE_SCHEMA_ID,
        "StoryOS Agent Run Response",
    );
    schema["properties"]["schema_id"]["const"] = json!(GET_AGENT_RUN_RESPONSE_SCHEMA_ID);
    for field in [
        "correlation_id",
        "project_agent_id",
        "conversation_id",
        "run_id",
    ] {
        schema["properties"][field]["format"] = json!("uuid");
    }
    schema["properties"]["memory_settings_revision"]["format"] = json!("uuid");
    if let Some(scope) = schema["$defs"].get_mut("ProjectScope") {
        scope["properties"]["owner_user_id"]["format"] = json!("uuid");
        scope["properties"]["project_id"]["format"] = json!("uuid");
    }
    if let Some(inspect) = schema["$defs"].get_mut("AgentRunContextInspect") {
        constrain_uuid_fields(
            inspect,
            &[
                "operation_requirement_id",
                "input_snapshot_id",
                "assembly_manifest_id",
            ],
        );
        constrain_count_fields(inspect);
    }
    for name in [
        "TokenCountingProfileInspect",
        "ContextSourceInspect",
        "ContextProjectionInspect",
        "ContextRejectionInspect",
    ] {
        if let Some(definition) = schema["$defs"].get_mut(name) {
            constrain_count_fields(definition);
        }
    }
    if let Some(manifest) = schema["$defs"].get_mut("OptionalManifestRef") {
        constrain_uuid_fields(manifest, &["manifest_id"]);
    }
    if let Some(availability) = schema["$defs"].get_mut("SourceAvailability") {
        constrain_uuid_fields(availability, &["current_revision_id"]);
    }
    for name in [
        "AttemptEvidence",
        "OptionalDecisionInspect",
        "OptionalModelAttemptInspect",
        "OptionalContinuationInspect",
        "OptionalOpenedProposalInspect",
        "ContinuationAdmissionInspect",
    ] {
        if let Some(definition) = schema["$defs"].get_mut(name) {
            constrain_uuid_fields(
                definition,
                &[
                    "attempt_id",
                    "decision_id",
                    "model_attempt_id",
                    "destination_attempt_id",
                    "outbound_disclosure_event_id",
                    "model_invocation_id",
                    "continuation_binding_id",
                    "reference_id",
                    "proposal_id",
                    "processing_destination_identity",
                    "model_registration_revision",
                    "project_model_use_binding_revision",
                    "external_compatibility_decision",
                ],
            );
        }
    }
    json_bytes(&schema)
}

pub(super) fn openapi() -> String {
    let create_request = generated_ref(CREATE_REQUEST_SCHEMA_PATH);
    let create_response = generated_ref(CREATE_RESPONSE_SCHEMA_PATH);
    let get_response = generated_ref(GET_RESPONSE_SCHEMA_PATH);
    let create_responses = status_block(
        CREATE_AGENT_RUN.responses,
        create_response,
        /*content_status*/ 202,
        /*command*/ true,
    );
    let get_responses = status_block(
        GET_AGENT_RUN.responses,
        get_response,
        /*content_status*/ 200,
        /*command*/ false,
    );
    format!(
        concat!(
            "  {}:\n    post:\n      operationId: {}\n      summary: Admit one bounded assistance request\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
            "  {}:\n    get:\n      operationId: {}\n      summary: Inspect one durable AgentRun status\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: run_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: model_attempt_id\n          in: query\n          required: false\n          schema:\n            type: string\n            format: uuid\n",
            "      responses:\n{}",
        ),
        CREATE_AGENT_RUN.path,
        CREATE_AGENT_RUN.operation_id,
        create_request,
        create_responses,
        GET_AGENT_RUN.path,
        GET_AGENT_RUN.operation_id,
        get_responses,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        ConversationSelection::decl(&config),
        AuthorMessage::decl(&config),
        AssistanceWorkingTarget::decl(&config),
        InstructionBinding::decl(&config),
        AssistanceCause::decl(&config),
        CreateAgentRunInput::decl(&config),
        CreateAgentRunRequest::decl(&config),
        AgentRunRef::decl(&config),
        CreateAgentRunEffect::decl(&config),
        CreateAgentRunResponse::decl(&config),
        AgentRunStatus::decl(&config),
        ContextPurpose::decl(&config),
        ContextSourceClass::decl(&config),
        ProjectionMode::decl(&config),
        ContextSufficiency::decl(&config),
        ContextBlockReason::decl(&config),
        ContextRejectionReason::decl(&config),
        DestinationIo::decl(&config),
        OptionalManifestRef::decl(&config),
        SourceAvailability::decl(&config),
        TokenCountingProfileInspect::decl(&config),
        ContextSourceInspect::decl(&config),
        ContextProjectionInspect::decl(&config),
        ContextRejectionInspect::decl(&config),
        HostControlInspect::decl(&config),
        CurrentAvailabilityInspect::decl(&config),
        AgentRunContextInspect::decl(&config),
        EvidenceAvailability::decl(&config),
        AttemptEvidence::decl(&config),
        OptionalContinuationInspect::decl(&config),
        OptionalOpenedProposalInspect::decl(&config),
        OptionalDecisionInspect::decl(&config),
        ContinuationInputMappingInspect::decl(&config),
        ContinuationAdmissionInspect::decl(&config),
        OptionalModelAttemptInspect::decl(&config),
        AgentRunStreamItemInspect::decl(&config),
        AgentRunUsageInspect::decl(&config),
        GetAgentRunRequest::decl(&config),
        GetAgentRunResponse::decl(&config),
    )
}

pub(super) fn typescript_client_source() -> String {
    format!(
        concat!(
            "\nexport async function digestCreateAgentRun(request, cryptoImpl = globalThis.crypto) {{\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"digestCreateAgentRun requires request\");\n",
            "  const canonical = canonicalJson(request);\n",
            "  const bytes = new TextEncoder().encode(JSON.stringify(canonical));\n",
            "  const digest = new Uint8Array(await cryptoImpl.subtle.digest(\"SHA-256\", bytes));\n",
            "  return {{ algorithm: \"sha256\", profile: \"{}\", value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, \"0\")).join(\"\") }};\n}}\n",
            "\nexport async function createAgentRun({{ projectId, request, idempotencyKey, antiForgery, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"createAgentRun requires projectId\");\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"createAgentRun requires request\");\n",
            "  if (typeof idempotencyKey !== \"string\" || typeof antiForgery !== \"string\") throw new TypeError(\"createAgentRun requires security bindings\");\n",
            "  return commandJson({{ ...options, path: `{}`, body: request, commandHeaders: {{ \"idempotency-key\": idempotencyKey, \"x-storyos-anti-forgery\": antiForgery }} }});\n}}\n",
            "\nexport async function getAgentRun({{ projectId, runId, modelAttemptId, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"getAgentRun requires projectId\");\n",
            "  if (typeof runId !== \"string\" || runId.length === 0) throw new TypeError(\"getAgentRun requires runId\");\n",
            "  const query = modelAttemptId == null || modelAttemptId === \"\" ? \"\" : `?model_attempt_id=${{encodeURIComponent(modelAttemptId)}}`;\n",
            "  return queryJson({{ ...options, path: `{}${{query}}` }});\n}}\n",
        ),
        CREATE_AGENT_RUN_DIGEST_PROFILE,
        CREATE_AGENT_RUN
            .path
            .replace("{project_id}", "${encodeURIComponent(projectId)}"),
        GET_AGENT_RUN
            .path
            .replace("{project_id}", "${encodeURIComponent(projectId)}")
            .replace("{run_id}", "${encodeURIComponent(runId)}"),
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    concat!(
        "export declare function digestCreateAgentRun(request: CreateAgentRunRequest, cryptoImpl?: Crypto): Promise<DigestValue>;\n",
        "export declare function createAgentRun(options: StoryOSQueryOptions & { projectId: string; request: CreateAgentRunRequest; idempotencyKey: string; antiForgery: string }): Promise<CreateAgentRunResponse>;\n",
        "export declare function getAgentRun(options: StoryOSQueryOptions & { projectId: string; runId: string; modelAttemptId?: string | null }): Promise<GetAgentRunResponse>;\n",
    )
}

pub(super) fn create_fixture_bytes() -> Vec<u8> {
    json_bytes(&create_fixture())
}

pub(super) fn create_invalid_fixture_bytes() -> Vec<u8> {
    let mut value = create_fixture();
    value
        .as_object_mut()
        .expect("fixture is an object")
        .remove("conversation_id");
    json_bytes(&value)
}

pub(super) fn create_boundary_fixture_bytes() -> Vec<u8> {
    json_bytes(&create_fixture())
}

pub(super) fn get_fixture_bytes() -> Vec<u8> {
    json_bytes(&get_fixture())
}

pub(super) fn get_invalid_fixture_bytes() -> Vec<u8> {
    let mut value = get_fixture();
    value
        .as_object_mut()
        .expect("fixture is an object")
        .remove("status");
    json_bytes(&value)
}

pub(super) fn get_boundary_fixture_bytes() -> Vec<u8> {
    json_bytes(&get_fixture())
}

fn create_fixture() -> Value {
    json!({
        "schema_id": CREATE_AGENT_RUN_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000a31",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "command_id": "018f0000-0000-7001-8000-000000000a32",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000a33",
        "acknowledgement": "accepted",
        "operation_ref": {
            "kind": "agent_run",
            "run_id": "018f0000-0000-7001-8000-000000000a34"
        },
        "project": {
            "project_id": "018f0000-0000-7001-8000-000000000201",
            "title": "Empty Novel",
            "open": {
                "kind": "current_chapter",
                "current_chapter_id": "018f0000-0000-7001-8000-000000000301"
            }
        },
        "project_agent_id": "018f0000-0000-7001-8000-000000000a35",
        "conversation_id": "018f0000-0000-7001-8000-000000000a36",
        "memory_settings_revision": "018f0000-0000-7001-8000-000000000a38",
        "effect": {
            "kind": "admitted",
            "project_agent_id": "018f0000-0000-7001-8000-000000000a35",
            "conversation_id": "018f0000-0000-7001-8000-000000000a36",
            "memory_settings_revision": "018f0000-0000-7001-8000-000000000a38",
            "run_id": "018f0000-0000-7001-8000-000000000a34",
            "project_activity_position": "1"
        }
    })
}

fn get_fixture() -> Value {
    json!({
        "schema_id": GET_AGENT_RUN_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000a37",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "project_agent_id": "018f0000-0000-7001-8000-000000000a35",
        "conversation_id": "018f0000-0000-7001-8000-000000000a36",
        "memory_settings_revision": "018f0000-0000-7001-8000-000000000a38",
        "run_id": "018f0000-0000-7001-8000-000000000a34",
        "status": "queued",
        "context": {
            "operation_requirement_id": "018f0000-0000-7001-8000-000000000a39",
            "input_snapshot_id": "018f0000-0000-7001-8000-000000000a3a",
            "purpose": "current_passage_assistance",
            "token_counting_profile": {
                "profile_revision": "storyos.token-counting.unicode-scalar.v1",
                "algorithm_revision": "storyos.statistics.unicode-16.0.0.v1",
                "item_token_limit": "10000"
            },
            "sufficiency": { "kind": "complete" },
            "considered": [
                {
                    "source_class": "host_control",
                    "source_version": "018f0000-0000-7001-8000-000000000a39",
                    "token_count": "0",
                    "eligible": true
                },
                {
                    "source_class": "author_instruction",
                    "source_version": "018f0000-0000-7001-8000-000000000a3a",
                    "token_count": "23",
                    "eligible": true
                },
                {
                    "source_class": "working_target",
                    "source_version": "018f0000-0000-7001-8000-000000000301",
                    "token_count": "0",
                    "eligible": true
                },
                {
                    "source_class": "instruction_binding",
                    "source_version": "absent",
                    "token_count": "0",
                    "eligible": true
                }
            ],
            "selected": [
                {
                    "source_class": "author_instruction",
                    "source_version": "018f0000-0000-7001-8000-000000000a3a",
                    "projection_mode": "exact_required",
                    "token_count": "23",
                    "content": "Help with this passage."
                },
                {
                    "source_class": "working_target",
                    "source_version": "018f0000-0000-7001-8000-000000000301",
                    "projection_mode": "exact_required",
                    "token_count": "0",
                    "content": ""
                }
            ],
            "rejected": [],
            "host_control": {
                "distinct_from_destination": true,
                "destination_visible": false
            },
            "assembly_manifest_id": "018f0000-0000-7001-8000-000000000a3b",
            "destination_context_manifest": { "kind": "absent" },
            "outbound_disclosure_manifest": { "kind": "absent" },
            "destination_io": { "kind": "none" },
            "current_availability": {
                "working_target": { "kind": "current" }
            }
        },
        "decision": { "kind": "absent" },
        "model_attempt": { "kind": "absent" },
        "evidence": [],
        "items": [],
        "usage": { "kind": "unknown" },
        "redaction_profile": "storyos.author.v1"
    })
}

fn constrain_count_fields(schema: &mut Value) {
    if let Some(properties) = schema.get_mut("properties") {
        for field in ["item_token_limit", "token_count"] {
            if properties.get(field).is_some() {
                properties[field] = json!({"type": "string", "pattern": U64_WIRE});
            }
        }
    }
    if let Some(one_of) = schema.get_mut("oneOf").and_then(Value::as_array_mut) {
        for variant in one_of {
            constrain_count_fields(variant);
        }
    }
}

fn constrain_revision_fields(schema: &mut Value) {
    if let Some(properties) = schema.get_mut("properties")
        && properties.get("project_activity_position").is_some()
    {
        properties["project_activity_position"] = json!({"type": "string", "pattern": U64_WIRE});
    }
    if let Some(one_of) = schema.get_mut("oneOf").and_then(Value::as_array_mut) {
        for variant in one_of {
            constrain_revision_fields(variant);
        }
    }
}

fn constrain_uuid_fields(schema: &mut Value, fields: &[&str]) {
    if let Some(properties) = schema.get_mut("properties") {
        for field in fields {
            if properties.get(*field).is_some() {
                properties[*field]["format"] = json!("uuid");
            }
        }
    }
    if let Some(one_of) = schema.get_mut("oneOf").and_then(Value::as_array_mut) {
        for variant in one_of {
            constrain_uuid_fields(variant, fields);
        }
    }
}

fn status_block(
    responses: &[(u16, &str)],
    response_schema: &str,
    content_status: u16,
    command: bool,
) -> String {
    responses
        .iter()
        .map(|(status, description)| {
            let retry_after = if command && *status == 429 {
                "          headers:\n            Retry-After:\n              required: true\n              schema:\n                type: integer\n                minimum: 1\n                maximum: 60\n"
            } else {
                ""
            };
            let content = if *status == content_status {
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
