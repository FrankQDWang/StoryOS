use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1::{APPLY_AUTHOR_EDIT_REQUEST_SCHEMA_ID, APPLY_AUTHOR_EDIT_RESPONSE_SCHEMA_ID};
use crate::release1_author_edit::{
    APPLY_AUTHOR_EDIT, ApplyAuthorEditEffect, ApplyAuthorEditRequest, ApplyAuthorEditResponse,
    AuthorEditConflictReason, AuthorEditPrimitive, AuthorEditProposalTarget,
    AuthorEditRefusalReason, AuthorEditUnit, DomainReceipt, DomainReceiptCommandKind,
    DomainReceiptProducerCause, DomainReceiptResult, EditSourceOwner, NoEffectReason,
    OrderedSourceSelection, RefusedEditOrigin, RefusedEditPayload, ReplacementBlock,
    SelectedEditSource, SelectionSnapshot, SourceSelectionEndpoint,
};
use crate::release1_operation_registry::{
    OperationArtifacts, RegisteredOperation, fixture_triple, method, operation_schemas,
};
use crate::release1_wire::{canonical_u64_wire_schema, json_bytes};

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::command(
        &APPLY_AUTHOR_EDIT,
        &[
            "server_derived_project_scope",
            "strict_origin",
            "protected_client_session_binding",
            "project_command_challenge",
            "editor_session_writer_generation",
            "expected_authoritative_head",
        ],
    )],
    schemas: || {
        operation_schemas(
            &APPLY_AUTHOR_EDIT,
            (REQUEST_SCHEMA_PATH, request_schema_bytes()),
            (RESPONSE_SCHEMA_PATH, response_schema_bytes()),
        )
        .into()
    },
    openapi: || method(&APPLY_AUTHOR_EDIT, openapi()),
    typescript_types: typescript_type_declarations,
    typescript_client: typescript_client_source,
    typescript_declarations,
    fixtures: || {
        fixture_triple(
            FIXTURE_PATHS,
            &APPLY_AUTHOR_EDIT,
            [
                |_| fixture_bytes(),
                |_| invalid_fixture_bytes(),
                |_| boundary_fixture_bytes(),
            ],
        )
        .into()
    },
};

pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/apply-author-edit-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/apply-author-edit-response.schema.json";
pub(super) const FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/apply-author-edit.json",
    "generated/golden-wire/storyos-public-release-1/apply-author-edit.invalid.json",
    "generated/golden-wire/storyos-public-release-1/apply-author-edit.boundary.json",
];

// This is true only in the layer that adds the public Server route.
pub(super) const IS_IMPLEMENTED: bool = true;

pub(super) fn request_schema_bytes() -> Vec<u8> {
    schema_bytes::<ApplyAuthorEditRequest>(
        APPLY_AUTHOR_EDIT_REQUEST_SCHEMA_ID,
        "StoryOS Apply Author Edit Request",
    )
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    schema_bytes::<ApplyAuthorEditResponse>(
        APPLY_AUTHOR_EDIT_RESPONSE_SCHEMA_ID,
        "StoryOS Apply Author Edit Response",
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    let source_types = [
        crate::DraftRetry::decl(&config),
        crate::DraftRetryKind::decl(&config),
        crate::ExactStructuredRangeKind::decl(&config),
        crate::DraftPayloadPosition::decl(&config),
        crate::ExactStructuredRange::decl(&config),
        crate::RetryDraftKind::decl(&config),
        crate::RetryDraftClosure::decl(&config),
        crate::SourceDraftDisposition::decl(&config),
        crate::ObservedDraftClosure::decl(&config),
        crate::DraftRetryReplacement::decl(&config),
        RefusedEditPayload::decl(&config),
        RefusedEditOrigin::decl(&config),
        ReplacementBlock::decl(&config),
        OrderedSourceSelection::decl(&config),
        SelectedEditSource::decl(&config),
        SourceSelectionEndpoint::decl(&config),
        EditSourceOwner::decl(&config),
    ]
    .into_iter()
    .map(|value| format!("export {value}\n\n"))
    .collect::<String>();
    source_types
        + &format!(
            "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
            AuthorEditPrimitive::decl(&config),
            SelectionSnapshot::decl(&config),
            AuthorEditUnit::decl(&config),
            AuthorEditProposalTarget::decl(&config),
            ApplyAuthorEditRequest::decl(&config),
            DomainReceiptCommandKind::decl(&config),
            DomainReceiptProducerCause::decl(&config),
            DomainReceiptResult::decl(&config),
            DomainReceipt::decl(&config),
            NoEffectReason::decl(&config),
            AuthorEditConflictReason::decl(&config),
            AuthorEditRefusalReason::decl(&config),
            ApplyAuthorEditEffect::decl(&config),
            ApplyAuthorEditResponse::decl(&config),
        )
}

pub(super) fn openapi() -> String {
    let request_schema = REQUEST_SCHEMA_PATH
        .strip_prefix("generated/")
        .expect("request schema is generated");
    let response_schema = RESPONSE_SCHEMA_PATH
        .strip_prefix("generated/")
        .expect("response schema is generated");
    let responses = APPLY_AUTHOR_EDIT.responses.iter().map(|(status, description)| format!(
        "        '{status}':\n          description: {description}\n{}",
        if *status == 200 { format!("          content:\n            application/json:\n              schema:\n                $ref: '../{response_schema}'\n") } else { String::new() }
    )).collect::<String>();
    format!(
        "    post:\n      operationId: {}\n      summary: Settle one direct Author Edit\n      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{request_schema}'\n      responses:\n{responses}",
        APPLY_AUTHOR_EDIT.operation_id
    )
}

pub(super) fn apply_u64_wire_constraints(schema: &mut Value, canonical_u64: &Value) {
    if schema["properties"].get("writer_generation").is_some() {
        schema["properties"]["writer_generation"] = canonical_u64.clone();
    }
    if schema["properties"].get("local_intent_sequence").is_some() {
        schema["properties"]["local_intent_sequence"] = canonical_u64.clone();
    }
    if schema["$defs"].get("DomainReceipt").is_some() {
        schema["$defs"]["DomainReceipt"]["properties"]["author_action_sequence"] =
            json!({"anyOf": [canonical_u64, {"type": "null"}]});
    }
    if schema["$defs"].get("ApplyAuthorEditEffect").is_some() {
        let variants = schema["$defs"]["ApplyAuthorEditEffect"]["oneOf"]
            .as_array_mut()
            .expect("Author Edit response variants must exist");
        for variant in variants.iter_mut() {
            variant["additionalProperties"] = Value::Bool(false);
        }
        let applied = variants
            .iter_mut()
            .find(|variant| variant["properties"]["kind"]["const"] == "authoritative_applied")
            .expect("authoritative effect exists");
        applied["properties"]["author_action_sequence"] = canonical_u64.clone();
        applied["properties"]["project_activity_position"] = canonical_u64.clone();
        if let Some(revised) = variants
            .iter_mut()
            .find(|variant| variant["properties"]["kind"]["const"] == "proposal_revised")
        {
            revised["properties"]["author_action_sequence"] = canonical_u64.clone();
        }
    }
}

pub(super) fn typescript_client_source() -> String {
    if !IS_IMPLEMENTED {
        return String::new();
    }
    format!(
        concat!(
            "\nexport async function digestApplyAuthorEdit(request, cryptoImpl = globalThis.crypto) {{\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"digestApplyAuthorEdit requires request\");\n",
            "  const canonical = canonicalJson(request);\n",
            "  const bytes = new TextEncoder().encode(JSON.stringify(canonical));\n",
            "  const digest = new Uint8Array(await cryptoImpl.subtle.digest(\"SHA-256\", bytes));\n",
            "  return {{ algorithm: \"sha256\", profile: \"storyos.command.applyAuthorEdit.jcs.v1\", value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, \"0\")).join(\"\") }};\n}}\n",
            "\nexport async function applyAuthorEdit({{ projectId, request, idempotencyKey, antiForgery, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"applyAuthorEdit requires projectId\");\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"applyAuthorEdit requires request\");\n",
            "  if (typeof idempotencyKey !== \"string\" || typeof antiForgery !== \"string\") throw new TypeError(\"applyAuthorEdit requires security bindings\");\n",
            "  return commandJson({{ ...options, path: `{}`, body: request, commandHeaders: {{ \"idempotency-key\": idempotencyKey, \"x-storyos-anti-forgery\": antiForgery }} }});\n}}\n",
        ),
        APPLY_AUTHOR_EDIT
            .path
            .replace("{project_id}", "${encodeURIComponent(projectId)}"),
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    if IS_IMPLEMENTED {
        concat!(
            "export declare function digestApplyAuthorEdit(request: ApplyAuthorEditRequest, cryptoImpl?: Crypto): Promise<DigestValue>;\n",
            "export declare function applyAuthorEdit(options: StoryOSQueryOptions & { projectId: string; request: ApplyAuthorEditRequest; idempotencyKey: string; antiForgery: string }): Promise<ApplyAuthorEditResponse>;\n",
        )
    } else {
        ""
    }
}

pub(super) fn fixture() -> Value {
    json!({
        "schema_id": APPLY_AUTHOR_EDIT_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000030",
        "project_scope": {"owner_user_id": "018f0000-0000-7001-8000-000000000001", "project_id": "018f0000-0000-7001-8000-000000000002"},
        "command_id": "018f0000-0000-7001-8000-000000000031",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000032",
        "receipt": {
            "receipt_id": "018f0000-0000-7001-8000-000000000033",
            "project_scope": {"owner_user_id": "018f0000-0000-7001-8000-000000000001", "project_id": "018f0000-0000-7001-8000-000000000002"},
            "command_kind": "applyAuthorEdit",
            "command_digest": {"algorithm": "sha256", "profile": "storyos.command.applyAuthorEdit.jcs.v1", "value_hex_lowercase": "a".repeat(64)},
            "idempotency_key": "018f0000-0000-7001-8000-000000000037",
            "producer_cause": "author_command_admission",
            "author_command_admission_id": "018f0000-0000-7001-8000-000000000032",
            "expected_heads": ["018f0000-0000-7001-8000-000000000004"],
            "prior_heads": ["018f0000-0000-7001-8000-000000000004"],
            "resulting_heads": ["018f0000-0000-7001-8000-000000000034"],
            "authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000034"],
            "proposal_revision_ids": [],
            "authoritative_commit_ids": ["018f0000-0000-7001-8000-000000000035"],
            "author_action_sequence": "1",
            "draft_artifact_refs": [],
            "artifact_lifecycle_event_refs": [],
            "condition_refs": [],
            "result": "authoritative_applied",
            "created_at": "2026-08-15T08:00:00.000Z"
        },
        "effect": {
            "kind": "authoritative_applied",
            "authoritative_revision": {"revision_id": "018f0000-0000-7001-8000-000000000034", "body": "雨落在窗沿。", "blocks": [{"manuscript_block_id": "018f0000-0000-7001-8000-0000000000b1", "block_kind": "paragraph", "text": "雨落在窗沿。"}]},
            "authoritative_commit_id": "018f0000-0000-7001-8000-000000000035",
            "author_action_sequence": "1",
            "project_activity_position": "1"
        },
        "completed_intent_record_id": "018f0000-0000-7001-8000-000000000036",
        "local_intent_sequence": "1"
    })
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&fixture())
}

pub(super) fn invalid_fixture_bytes() -> Vec<u8> {
    let mut value = fixture();
    value
        .as_object_mut()
        .expect("fixture is an object")
        .remove("project_scope");
    json_bytes(&value)
}

pub(super) fn boundary_fixture_bytes() -> Vec<u8> {
    let mut value = fixture();
    value["receipt"]["resulting_heads"] = value["receipt"]["prior_heads"].clone();
    value["receipt"]["authoritative_revision_ids"] = json!([]);
    value["receipt"]["authoritative_commit_ids"] = json!([]);
    value["receipt"]["author_action_sequence"] = Value::Null;
    value["receipt"]["result"] = json!("refused");
    value["effect"] = json!({
        "kind": "refused",
        "reason": "invalid_selection"
    });
    json_bytes(&value)
}

fn schema_bytes<T: schemars::JsonSchema>(schema_id: &str, title: &str) -> Vec<u8> {
    let mut schema = serde_json::to_value(schema_for!(T)).expect("contract schema serializes");
    schema["$id"] = Value::String(schema_id.to_owned());
    schema["title"] = Value::String(title.to_owned());
    let canonical_u64 = canonical_u64_wire_schema();
    apply_u64_wire_constraints(&mut schema, &canonical_u64);
    json_bytes(&schema)
}
