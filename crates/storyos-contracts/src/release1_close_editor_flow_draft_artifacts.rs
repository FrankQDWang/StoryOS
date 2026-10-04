use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_close_editor_flow_draft::{
    CLOSE_EDITOR_FLOW_DRAFT, CLOSE_EDITOR_FLOW_DRAFT_DIGEST_PROFILE,
    CLOSE_EDITOR_FLOW_DRAFT_REQUEST_SCHEMA_ID, CLOSE_EDITOR_FLOW_DRAFT_RESPONSE_SCHEMA_ID,
    CloseEditorFlowDraftEffect, CloseEditorFlowDraftInput, CloseEditorFlowDraftRequest,
    CloseEditorFlowDraftResponse, DraftCloseRefusal, EDITOR_FLOW_DRAFT_CLOSED_SCHEMA_ID,
    EditorFlowDraftClosed,
};
use crate::release1_operation_registry::{
    GeneratedSchema, OperationArtifacts, RegisteredOperation, event_fixtures, fixture_triple,
    method, operation_schemas,
};
use crate::release1_wire::{generated_ref, json_bytes, schema_value};

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::command(
        &CLOSE_EDITOR_FLOW_DRAFT,
        &[
            "exact_scope",
            "current_writer",
            "exact_retained_open_source",
            "explicit_editor_command",
        ],
    )],
    schemas: || {
        let [request, response] = operation_schemas(
            &CLOSE_EDITOR_FLOW_DRAFT,
            (REQUEST_SCHEMA_PATH, request_schema_bytes()),
            (RESPONSE_SCHEMA_PATH, response_schema_bytes()),
        );
        vec![
            request,
            response,
            GeneratedSchema {
                schema_id: EDITOR_FLOW_DRAFT_CLOSED_SCHEMA_ID,
                path: EVENT_SCHEMA_PATH,
                bytes: event_schema_bytes(),
            },
        ]
    },
    openapi: || method(&CLOSE_EDITOR_FLOW_DRAFT, openapi()),
    typescript_types: typescript_type_declarations,
    typescript_client: typescript_client_source,
    typescript_declarations,
    fixtures: || {
        fixture_triple(
            FIXTURE_PATHS,
            &CLOSE_EDITOR_FLOW_DRAFT,
            [
                |_| fixture_bytes(),
                |_| invalid_fixture_bytes(),
                |_| boundary_fixture_bytes(),
            ],
        )
        .into_iter()
        .chain(event_fixtures(
            EVENT_FIXTURE_PATHS,
            [
                "storyos.golden.storyos.event.editor-flow-draft-closed.v1.positive.v1",
                "storyos.golden.storyos.event.editor-flow-draft-closed.v1.negative.v1",
            ],
            CLOSE_EDITOR_FLOW_DRAFT.operation_id,
            [|_| event_fixture_bytes(), |_| event_invalid_fixture_bytes()],
        ))
        .collect()
    },
};

pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/close-editor-flow-draft-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/close-editor-flow-draft-response.schema.json";
pub(super) const FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/close-editor-flow-draft.json",
    "generated/golden-wire/storyos-public-release-1/close-editor-flow-draft.invalid.json",
    "generated/golden-wire/storyos-public-release-1/close-editor-flow-draft.boundary.json",
];

pub(super) fn request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<CloseEditorFlowDraftRequest>(
        CLOSE_EDITOR_FLOW_DRAFT_REQUEST_SCHEMA_ID,
        "StoryOS Close Editor Flow Draft Request",
    );
    schema["properties"]["command_schema"]["const"] =
        json!(CLOSE_EDITOR_FLOW_DRAFT_REQUEST_SCHEMA_ID);
    let input = &mut schema["$defs"]["CloseEditorFlowDraftInput"]["properties"];
    input["draft_id"]["format"] = json!("uuid");
    input["source_current_draft_revision_id"]["format"] = json!("uuid");
    input["source_draft_payload_digest"]["pattern"] = json!("^[0-9a-f]{64}$");
    for (field, value) in [
        ("draft_kind", "refused_edit"),
        ("expected_closure", "open"),
        ("close_reason", "abandoned"),
    ] {
        input[field]["const"] = json!(value);
    }
    input["editor_session_id"]["format"] = json!("uuid");
    input["writer_generation"] = super::release1_author_edit_artifacts::canonical_u64_wire_schema();
    input["correlation_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<CloseEditorFlowDraftResponse>(
        CLOSE_EDITOR_FLOW_DRAFT_RESPONSE_SCHEMA_ID,
        "StoryOS Close Editor Flow Draft Response",
    );
    schema["properties"]["schema_id"]["const"] = json!(CLOSE_EDITOR_FLOW_DRAFT_RESPONSE_SCHEMA_ID);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["command_id"]["format"] = json!("uuid");
    schema["properties"]["author_command_admission_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn openapi() -> String {
    let request_schema = generated_ref(REQUEST_SCHEMA_PATH);
    let response_schema = generated_ref(RESPONSE_SCHEMA_PATH);
    let responses = CLOSE_EDITOR_FLOW_DRAFT
        .responses
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
        .collect::<String>();
    format!(
        concat!(
            "    post:\n      operationId: {}\n      summary: Discard one open retained Refused Edit Draft\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: draft_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        CLOSE_EDITOR_FLOW_DRAFT.operation_id, request_schema, responses,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    [
        CloseEditorFlowDraftInput::decl(&config),
        CloseEditorFlowDraftRequest::decl(&config),
        DraftCloseRefusal::decl(&config),
        EditorFlowDraftClosed::decl(&config),
        CloseEditorFlowDraftEffect::decl(&config),
        CloseEditorFlowDraftResponse::decl(&config),
    ]
    .map(|declaration| format!("export {declaration}"))
    .join("\n")
}

pub(super) fn typescript_client_source() -> String {
    format!(
        concat!(
            "\nexport async function digestCloseEditorFlowDraft(request, cryptoImpl = globalThis.crypto) {{\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"digestCloseEditorFlowDraft requires request\");\n",
            "  const canonical = canonicalJson(request);\n",
            "  const bytes = new TextEncoder().encode(JSON.stringify(canonical));\n",
            "  const digest = new Uint8Array(await cryptoImpl.subtle.digest(\"SHA-256\", bytes));\n",
            "  return {{ algorithm: \"sha256\", profile: \"{}\", value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, \"0\")).join(\"\") }};\n}}\n",
            "\nexport async function closeEditorFlowDraft({{ projectId, draftId, request, idempotencyKey, antiForgery, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"closeEditorFlowDraft requires projectId\");\n",
            "  if (typeof draftId !== \"string\" || draftId.length === 0) throw new TypeError(\"closeEditorFlowDraft requires draftId\");\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"closeEditorFlowDraft requires request\");\n",
            "  if (typeof idempotencyKey !== \"string\" || typeof antiForgery !== \"string\") throw new TypeError(\"closeEditorFlowDraft requires security bindings\");\n",
            "  return commandJson({{ ...options, method: \"POST\", path: `{}`, body: request, commandHeaders: {{ \"idempotency-key\": idempotencyKey, \"x-storyos-anti-forgery\": antiForgery }} }});\n}}\n",
        ),
        CLOSE_EDITOR_FLOW_DRAFT_DIGEST_PROFILE,
        CLOSE_EDITOR_FLOW_DRAFT
            .path
            .replace("{project_id}", "${encodeURIComponent(projectId)}")
            .replace("{draft_id}", "${encodeURIComponent(draftId)}"),
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    concat!(
        "export declare function digestCloseEditorFlowDraft(request: CloseEditorFlowDraftRequest, cryptoImpl?: Crypto): Promise<DigestValue>;\n",
        "export declare function closeEditorFlowDraft(options: StoryOSQueryOptions & { projectId: string; draftId: string; request: CloseEditorFlowDraftRequest; idempotencyKey: string; antiForgery: string }): Promise<CloseEditorFlowDraftResponse>;\n",
    )
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&command_fixture("2026-09-19T12:00:00.000Z"))
}

pub(super) fn invalid_fixture_bytes() -> Vec<u8> {
    let mut value = command_fixture("2026-09-19T12:00:00.000Z");
    value
        .as_object_mut()
        .expect("fixture is an object")
        .remove("effect");
    json_bytes(&value)
}

pub(super) fn boundary_fixture_bytes() -> Vec<u8> {
    let mut value = command_fixture("1970-01-01T00:00:00.000Z");
    value["effect"] =
        json!({"kind":"refused","reason":"source_draft_not_open","current_closure":"closed"});
    value["receipt"]["result"] = json!("refused");
    value["receipt"]["author_action_sequence"] = Value::Null;
    value["receipt"]["artifact_lifecycle_event_refs"] = json!([]);
    json_bytes(&value)
}

fn command_fixture(created_at: &str) -> Value {
    let id = "018f0000-0000-7001-8000-000000000d10";
    let scope = json!({"owner_user_id":id,"project_id":id});
    let digest = json!({"algorithm":"sha256","profile":CLOSE_EDITOR_FLOW_DRAFT_DIGEST_PROFILE,"value_hex_lowercase":"d".repeat(64)});
    json!({
        "schema_id":CLOSE_EDITOR_FLOW_DRAFT_RESPONSE_SCHEMA_ID,"correlation_id":id,"project_scope":scope,
        "command_id":id,"author_command_admission_id":id,
        "receipt":{"receipt_id":id,"project_scope":scope,"command_kind":"closeEditorFlowDraft",
            "command_digest":digest,"idempotency_key":id,"producer_cause":"author_command_admission",
            "author_command_admission_id":id,"expected_heads":[],"prior_heads":[],"resulting_heads":[],
            "authoritative_revision_ids":[],"proposal_revision_ids":[],"authoritative_commit_ids":[],
            "author_action_sequence":"3","draft_artifact_refs":[id],"artifact_lifecycle_event_refs":[id],
            "condition_refs":[],"result":"draft_closure_changed","created_at":created_at},
        "effect":{"kind":"draft_closure_changed","event":{
            "schema_id":EDITOR_FLOW_DRAFT_CLOSED_SCHEMA_ID,"event_kind":"editor_flow_draft_closed","event_id":id,
            "project_scope":scope,"draft_id":id,"draft_revision_id":id,"payload_digest":"e".repeat(64),
            "prior_closure":"open","closure":"closed","close_reason":"abandoned",
            "source":{"command_id":id,"author_command_admission_id":id,"receipt_id":id,"idempotency_key":id,"command_digest":digest},
            "author_action_sequence":"3","created_at":created_at}}
    })
}

pub(super) const EVENT_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/editor-flow-draft-closed.schema.json";
pub(super) const EVENT_FIXTURE_PATHS: [&str; 2] = [
    "generated/golden-wire/storyos-public-release-1/editor-flow-draft-closed.json",
    "generated/golden-wire/storyos-public-release-1/editor-flow-draft-closed.invalid.json",
];
pub(super) fn event_schema_bytes() -> Vec<u8> {
    let mut value = schema_value::<EditorFlowDraftClosed>(
        EDITOR_FLOW_DRAFT_CLOSED_SCHEMA_ID,
        "StoryOS Draft Closed Event",
    );
    for (field, constant) in [
        ("schema_id", EDITOR_FLOW_DRAFT_CLOSED_SCHEMA_ID),
        ("event_kind", "editor_flow_draft_closed"),
        ("prior_closure", "open"),
        ("closure", "closed"),
    ] {
        value["properties"][field]["const"] = json!(constant);
    }
    value["properties"]["close_reason"]["enum"] = json!(["abandoned", "superseded"]);
    json_bytes(&value)
}
pub(super) fn event_fixture_bytes() -> Vec<u8> {
    json_bytes(&command_fixture("2026-09-26T00:00:00.000Z")["effect"]["event"])
}
pub(super) fn event_invalid_fixture_bytes() -> Vec<u8> {
    let mut value = command_fixture("2026-09-26T00:00:00.000Z")["effect"]["event"].clone();
    value["close_reason"] = json!("automatic");
    json_bytes(&value)
}
