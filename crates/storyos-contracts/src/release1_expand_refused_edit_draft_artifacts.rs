use crate::release1_close_editor_flow_draft_artifacts::{generated_ref, json_bytes, schema_value};
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_expand_refused_edit_draft::{
    EXPAND_REFUSED_EDIT_DRAFT, EXPAND_REFUSED_EDIT_DRAFT_DIGEST_PROFILE,
    EXPAND_REFUSED_EDIT_DRAFT_REQUEST_SCHEMA_ID, EXPAND_REFUSED_EDIT_DRAFT_RESPONSE_SCHEMA_ID,
    ExpandRefusedEditDraftEffect, ExpandRefusedEditDraftInput, ExpandRefusedEditDraftRequest,
    ExpandRefusedEditDraftResponse, WholeDraftPayload,
};

pub(super) const REQUEST_SCHEMA_PATH: &str = "generated/json-schema/storyos-public-release-1/expand-refused-edit-draft-to-proposal-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str = "generated/json-schema/storyos-public-release-1/expand-refused-edit-draft-to-proposal-response.schema.json";
pub(super) const FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/expand-refused-edit-draft-to-proposal.json",
    "generated/golden-wire/storyos-public-release-1/expand-refused-edit-draft-to-proposal.invalid.json",
    "generated/golden-wire/storyos-public-release-1/expand-refused-edit-draft-to-proposal.boundary.json",
];

pub(super) fn request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<ExpandRefusedEditDraftRequest>(
        EXPAND_REFUSED_EDIT_DRAFT_REQUEST_SCHEMA_ID,
        "StoryOS Close Editor Flow Draft Request",
    );
    schema["properties"]["command_schema"]["const"] =
        json!(EXPAND_REFUSED_EDIT_DRAFT_REQUEST_SCHEMA_ID);
    let input = &mut schema["$defs"]["ExpandRefusedEditDraftInput"]["properties"];
    input["draft_id"]["format"] = json!("uuid");
    input["source_current_draft_revision_id"]["format"] = json!("uuid");
    input["source_draft_payload_digest"]["pattern"] = json!("^[0-9a-f]{64}$");
    input["expected_source_draft_closure"]["const"] = json!("open");
    input["proposal_kind"]["const"] = json!("inline_edit");
    input["editor_session_id"]["format"] = json!("uuid");
    input["writer_generation"] = super::release1_author_edit_artifacts::canonical_u64_wire_schema();
    input["correlation_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<ExpandRefusedEditDraftResponse>(
        EXPAND_REFUSED_EDIT_DRAFT_RESPONSE_SCHEMA_ID,
        "StoryOS Close Editor Flow Draft Response",
    );
    schema["properties"]["schema_id"]["const"] =
        json!(EXPAND_REFUSED_EDIT_DRAFT_RESPONSE_SCHEMA_ID);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["command_id"]["format"] = json!("uuid");
    schema["properties"]["author_command_admission_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn openapi() -> String {
    let request_schema = generated_ref(REQUEST_SCHEMA_PATH);
    let response_schema = generated_ref(RESPONSE_SCHEMA_PATH);
    let responses = EXPAND_REFUSED_EDIT_DRAFT
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
            "  {}:\n    post:\n      operationId: {}\n      summary: Expand one complete retained Refused Edit Draft\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: draft_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        EXPAND_REFUSED_EDIT_DRAFT.path,
        EXPAND_REFUSED_EDIT_DRAFT.operation_id,
        request_schema,
        responses,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    [
        ExpandRefusedEditDraftInput::decl(&config),
        ExpandRefusedEditDraftRequest::decl(&config),
        WholeDraftPayload::decl(&config),
        ExpandRefusedEditDraftEffect::decl(&config),
        ExpandRefusedEditDraftResponse::decl(&config),
    ]
    .iter()
    .map(|declaration| format!("export {declaration}\n"))
    .collect()
}

pub(super) fn typescript_client_source() -> String {
    format!(
        concat!(
            "\nexport async function digestExpandRefusedEditDraft(request, cryptoImpl = globalThis.crypto) {{\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"digestExpandRefusedEditDraft requires request\");\n",
            "  const canonical = canonicalJson(request);\n",
            "  const bytes = new TextEncoder().encode(JSON.stringify(canonical));\n",
            "  const digest = new Uint8Array(await cryptoImpl.subtle.digest(\"SHA-256\", bytes));\n",
            "  return {{ algorithm: \"sha256\", profile: \"{}\", value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, \"0\")).join(\"\") }};\n}}\n",
            "\nexport async function expandRefusedEditDraftToProposal({{ projectId, draftId, request, idempotencyKey, antiForgery, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"expandRefusedEditDraftToProposal requires projectId\");\n",
            "  if (typeof draftId !== \"string\" || draftId.length === 0) throw new TypeError(\"expandRefusedEditDraftToProposal requires draftId\");\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"expandRefusedEditDraftToProposal requires request\");\n",
            "  if (typeof idempotencyKey !== \"string\" || typeof antiForgery !== \"string\") throw new TypeError(\"expandRefusedEditDraftToProposal requires security bindings\");\n",
            "  return commandJson({{ ...options, method: \"POST\", path: `{}`, body: request, commandHeaders: {{ \"idempotency-key\": idempotencyKey, \"x-storyos-anti-forgery\": antiForgery }} }});\n}}\n",
        ),
        EXPAND_REFUSED_EDIT_DRAFT_DIGEST_PROFILE,
        EXPAND_REFUSED_EDIT_DRAFT
            .path
            .replace("{project_id}", "${encodeURIComponent(projectId)}")
            .replace("{draft_id}", "${encodeURIComponent(draftId)}"),
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    concat!(
        "export declare function digestExpandRefusedEditDraft(request: ExpandRefusedEditDraftRequest, cryptoImpl?: Crypto): Promise<DigestValue>;\n",
        "export declare function expandRefusedEditDraftToProposal(options: StoryOSQueryOptions & { projectId: string; draftId: string; request: ExpandRefusedEditDraftRequest; idempotencyKey: string; antiForgery: string }): Promise<ExpandRefusedEditDraftResponse>;\n",
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
    let mut value: Value =
        serde_json::from_slice(&crate::release1_close_editor_flow_draft_artifacts::fixture_bytes())
            .expect("Draft fixture is valid");
    let id = value["command_id"].clone();
    value["schema_id"] = json!(EXPAND_REFUSED_EDIT_DRAFT_RESPONSE_SCHEMA_ID);
    value["receipt"]["command_kind"] = json!("expandRefusedEditDraftToProposal");
    value["receipt"]["command_digest"]["profile"] = json!(EXPAND_REFUSED_EDIT_DRAFT_DIGEST_PROFILE);
    value["receipt"]["result"] = json!("proposal_created_from_draft");
    value["receipt"]["expected_heads"] = json!([id]);
    value["receipt"]["prior_heads"] = json!([id]);
    value["receipt"]["resulting_heads"] = json!([id]);
    value["receipt"]["proposal_revision_ids"] = json!([id]);
    value["receipt"]["created_at"] = json!(created_at);
    value["effect"]["kind"] = json!("proposal_created_from_draft");
    value["effect"]["proposal_id"] = id.clone();
    value["effect"]["proposal_revision_id"] = id;
    value["effect"]["event"]["close_reason"] = json!("superseded");
    value["effect"]["event"]["source"]["command_digest"]["profile"] =
        json!(EXPAND_REFUSED_EDIT_DRAFT_DIGEST_PROFILE);
    value["effect"]["event"]["created_at"] = json!(created_at);
    value
}
