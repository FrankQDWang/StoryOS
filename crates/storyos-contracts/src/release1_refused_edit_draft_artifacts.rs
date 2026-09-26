use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_refused_edit_draft::{
    GET_REFUSED_EDIT_DRAFT, GET_REFUSED_EDIT_DRAFT_REQUEST_SCHEMA_ID,
    GET_REFUSED_EDIT_DRAFT_RESPONSE_SCHEMA_ID, GetRefusedEditDraftResponse,
    REFUSED_EDIT_DRAFT_CREATED_SCHEMA_ID, RefusedEditDraftCreated, RefusedEditDraftCreator,
    RefusedEditDraftInspect, RefusedEditDraftSource,
};

pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/refused-edit-draft-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/refused-edit-draft-response.schema.json";
pub(super) const FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/get-refused-edit-draft.json",
    "generated/golden-wire/storyos-public-release-1/get-refused-edit-draft.invalid.json",
    "generated/golden-wire/storyos-public-release-1/get-refused-edit-draft.boundary.json",
];

pub(super) fn request_schema_bytes() -> Vec<u8> {
    json_bytes(&json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": GET_REFUSED_EDIT_DRAFT_REQUEST_SCHEMA_ID,
        "title": "StoryOS Refused Edit Draft Request",
        "type": "object",
        "additionalProperties": false,
        "required": ["project_id", "draft_id"],
        "properties": {
            "project_id": {"type": "string", "format": "uuid"},
            "draft_id": {"type": "string", "format": "uuid"}
        }
    }))
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = serde_json::to_value(schema_for!(GetRefusedEditDraftResponse))
        .expect("proposal response schema serializes");
    schema["$id"] = json!(GET_REFUSED_EDIT_DRAFT_RESPONSE_SCHEMA_ID);
    schema["title"] = json!("StoryOS Refused Edit Draft Response");
    schema["additionalProperties"] = json!(false);
    schema["properties"]["schema_id"]["const"] = json!(GET_REFUSED_EDIT_DRAFT_RESPONSE_SCHEMA_ID);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    if let Some(scope) = schema["$defs"].get_mut("ProjectScope") {
        scope["properties"]["owner_user_id"]["format"] = json!("uuid");
        scope["properties"]["project_id"]["format"] = json!("uuid");
    }
    for name in [
        "RefusedEditDraftInspect",
        "RefusedEditDraftCreated",
        "RefusedEditDraftSource",
        "RefusedEditPayload",
    ] {
        if let Some(definition) = schema["$defs"].get_mut(name) {
            for field in [
                "draft_id",
                "draft_revision_id",
                "creation_event_id",
                "command_id",
                "author_command_admission_id",
                "receipt_id",
                "idempotency_key",
                "chapter_id",
                "expected_authoritative_revision_id",
                "undo_group_id",
                "completed_intent_record_id",
            ] {
                if let Some(property) = definition["properties"].get_mut(field) {
                    property["format"] = json!("uuid");
                }
            }
        }
    }
    schema["$defs"]["RefusedEditDraftInspect"]["properties"]["kind"]["const"] =
        json!("refused_edit");
    schema["$defs"]["RefusedEditDraftInspect"]["properties"]["closure"]["enum"] =
        json!(["open", "closed"]);
    schema["$defs"]["RefusedEditDraftInspect"]["properties"]["retention_state"]["enum"] =
        json!(["retained", "archived", "tombstoned"]);
    schema["$defs"]["RefusedEditDraftInspect"]["properties"]["payload_digest_profile"]["const"] =
        json!("storyos.refused-edit-payload.jcs.v1");
    schema["$defs"]["RefusedEditDraftCreated"]["properties"]["event_kind"]["const"] =
        json!("refused_edit_draft_created");
    schema["$defs"]["RefusedEditDraftCreated"]["properties"]["schema_id"]["const"] =
        json!(REFUSED_EDIT_DRAFT_CREATED_SCHEMA_ID);
    schema["$defs"]["RefusedEditPayload"]["properties"]["schema_revision"]["const"] =
        json!("storyos.refused-edit-payload.v1");
    json_bytes(&schema)
}

pub(super) fn openapi() -> String {
    let response_schema = RESPONSE_SCHEMA_PATH
        .strip_prefix("generated/")
        .expect("schema is a generated artifact");
    let responses = GET_REFUSED_EDIT_DRAFT
        .responses
        .iter()
        .map(|(status, description)| {
            let content = if *status == 200 {
                format!(
                    "          content:\n            application/json:\n              schema:\n                $ref: '../{response_schema}'\n"
                )
            } else {
                String::new()
            };
            format!("        '{status}':\n          description: {description}\n{content}")
        })
        .collect::<String>();
    format!(
        concat!(
            "  {}:\n    get:\n      operationId: {}\n      summary: Inspect one retained Refused Edit Draft\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: draft_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "      responses:\n{}",
        ),
        GET_REFUSED_EDIT_DRAFT.path, GET_REFUSED_EDIT_DRAFT.operation_id, responses,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    [
        RefusedEditDraftSource::decl(&config),
        RefusedEditDraftCreated::decl(&config),
        RefusedEditDraftCreator::decl(&config),
        RefusedEditDraftInspect::decl(&config),
        GetRefusedEditDraftResponse::decl(&config),
    ]
    .iter()
    .map(|declaration| format!("export {declaration}\n"))
    .collect()
}

pub(super) fn typescript_client_source() -> String {
    let path = GET_REFUSED_EDIT_DRAFT
        .path
        .replace("{project_id}", "${encodeURIComponent(projectId)}")
        .replace("{draft_id}", "${encodeURIComponent(draftId)}");
    format!(
        concat!(
            "\nexport async function getRefusedEditDraft({{ projectId, draftId, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"getRefusedEditDraft requires projectId\");\n",
            "  if (typeof draftId !== \"string\" || draftId.length === 0) throw new TypeError(\"getRefusedEditDraft requires draftId\");\n",
            "  return queryJson({{ ...options, path: `{path}` }});\n}}\n"
        ),
        path = path
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    "export declare function getRefusedEditDraft(options: StoryOSQueryOptions & { projectId: string; draftId: string }): Promise<GetRefusedEditDraftResponse>;\n"
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&proposal_fixture())
}

pub(super) fn invalid_fixture_bytes() -> Vec<u8> {
    let mut value = proposal_fixture();
    value
        .as_object_mut()
        .expect("fixture is an object")
        .remove("draft");
    json_bytes(&value)
}

pub(super) fn boundary_fixture_bytes() -> Vec<u8> {
    let mut boundary = proposal_fixture();
    boundary["draft"]["closure"] = json!("closed");
    json_bytes(&boundary)
}

fn proposal_fixture() -> Value {
    let id = "018f0000-0000-7001-8000-000000000b02";
    json!({
        "schema_id": GET_REFUSED_EDIT_DRAFT_RESPONSE_SCHEMA_ID,
        "correlation_id": id,
        "project_scope": {"owner_user_id": id, "project_id": id},
        "draft": {
            "draft_id": id, "draft_revision_id": id, "kind": "refused_edit", "closure": "open", "retention_state": "retained",
            "payload": {
                "schema_revision": "storyos.refused-edit-payload.v1", "chapter_id": id,
                "expected_authoritative_revision_id": id, "expected_proposal_head_revision_ids": [id],
                "target_refs": [format!("manuscript:{id}")], "author_edit_units": [{
                    "normalized_primitives": [{"kind": "replace_structured_selection", "replacement": [{"block_kind": "paragraph", "text": "New passage"}]}],
                    "selection_snapshot": {"coordinate_profile": "storyos.editor.ordered-source.v1", "from": 0, "to": 9,
                        "ordered_selection": {
                            "sources": [{"owner": {"kind": "manuscript", "manuscript_block_id": id}, "coordinate_profile": "prosemirror-token-utf16.v1", "from": 0, "to": 5, "block_kind": "paragraph", "source_text": "First"},
                            {"owner": {"kind": "proposal", "proposal_id": id, "operation_id": id, "revision_id": id, "manuscript_block_id": id}, "coordinate_profile": "storyos.editor.utf16-code-unit.v1", "from": 0, "to": 9, "block_kind": "paragraph", "source_text": "Candidate"}],
                            "anchor": {"source_index": 0, "source_offset": 0}, "head": {"source_index": 1, "source_offset": 9}
                        }
                    }
                }], "undo_group_id": id, "completed_intent_record_id": id, "local_intent_sequence": "1"
            },
            "payload_digest": "7e791d480f4a8e5d8f6dd9b12007f5e747d26bc6efef0d50426d1449a46a1592", "payload_digest_profile": "storyos.refused-edit-payload.jcs.v1",
            "creation": {"event_kind": "refused_edit_draft_created", "project_scope": {"owner_user_id": id, "project_id": id}, "creator": {"kind": "core_transition", "receipt_id": id}, "schema_id": REFUSED_EDIT_DRAFT_CREATED_SCHEMA_ID, "creation_event_id": id, "draft_id": id, "draft_revision_id": id,
                "created_at": "2026-09-26T00:00:00.000Z", "source": {"command_id": id, "author_command_admission_id": id, "receipt_id": id, "idempotency_key": id, "command_digest": {"algorithm": "sha256", "profile": "storyos.command.applyAuthorEdit.jcs.v1", "value_hex_lowercase": "0".repeat(64)}}}
        }
    })
}

fn json_bytes(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("contract JSON should serialize");
    bytes.push(b'\n');
    bytes
}

pub(super) const EVENT_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/refused-edit-draft-created.schema.json";
pub(super) const EVENT_FIXTURE_PATHS: [&str; 2] = [
    "generated/golden-wire/storyos-public-release-1/refused-edit-draft-created.json",
    "generated/golden-wire/storyos-public-release-1/refused-edit-draft-created.invalid.json",
];

pub(super) fn event_schema_bytes() -> Vec<u8> {
    let mut schema = serde_json::to_value(schema_for!(RefusedEditDraftCreated))
        .expect("creation schema serializes");
    schema["$id"] = json!(REFUSED_EDIT_DRAFT_CREATED_SCHEMA_ID);
    schema["properties"]["schema_id"]["const"] = json!(REFUSED_EDIT_DRAFT_CREATED_SCHEMA_ID);
    schema["properties"]["event_kind"]["const"] = json!("refused_edit_draft_created");
    for field in ["creation_event_id", "draft_id", "draft_revision_id"] {
        schema["properties"][field]["format"] = json!("uuid");
    }
    schema["$defs"]["ProjectScope"]["properties"]["owner_user_id"]["format"] = json!("uuid");
    schema["$defs"]["ProjectScope"]["properties"]["project_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn event_fixture_bytes() -> Vec<u8> {
    json_bytes(&proposal_fixture()["draft"]["creation"])
}
pub(super) fn event_invalid_fixture_bytes() -> Vec<u8> {
    let mut value = proposal_fixture()["draft"]["creation"].clone();
    value["creator"] = json!({"kind": "author"});
    json_bytes(&value)
}
