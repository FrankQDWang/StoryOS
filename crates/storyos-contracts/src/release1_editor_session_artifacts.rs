use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1::{
    CREATE_EDITOR_SESSION, CREATE_EDITOR_SESSION_REQUEST_SCHEMA_ID,
    CREATE_EDITOR_SESSION_RESPONSE_SCHEMA_ID, CreateEditorSessionRequest,
    CreateEditorSessionResponse, EditorBaseSnapshot, EditorReadOnlyReason, EditorSessionBinding,
    EditorWriterProjection, GET_EDITOR_SESSION, GET_EDITOR_SESSION_REQUEST_SCHEMA_ID,
    GET_EDITOR_SESSION_RESPONSE_SCHEMA_ID, GetEditorSessionResponse, WEB_CLIENT_CONTRACT_REVISION,
};
use crate::release1_operation_registry::{
    GeneratedSchema, OperationArtifacts, RegisteredOperation, fixture_triple, method,
};
use crate::release1_wire::{
    canonical_u64_wire_schema, json_bytes, path_request_schema, query_openapi, schema_value,
    with_boundary_project_scope, without_project_scope,
};

pub(super) const EDITOR_SESSION_CREATE_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/editor-session-create-request.schema.json";

pub(super) const EDITOR_SESSION_CREATE_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/editor-session-create-response.schema.json";

pub(super) const EDITOR_SESSION_GET_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/editor-session-get-request.schema.json";

pub(super) const EDITOR_SESSION_GET_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/editor-session-get-response.schema.json";

const CREATE_EDITOR_SESSION_FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/create-editor-session.json",
    "generated/golden-wire/storyos-public-release-1/create-editor-session.invalid.json",
    "generated/golden-wire/storyos-public-release-1/create-editor-session.boundary.json",
];

const GET_EDITOR_SESSION_FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/get-editor-session.json",
    "generated/golden-wire/storyos-public-release-1/get-editor-session.invalid.json",
    "generated/golden-wire/storyos-public-release-1/get-editor-session.boundary.json",
];

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[
        RegisteredOperation::command(
            &CREATE_EDITOR_SESSION,
            &[
                "server_derived_project_scope",
                "strict_origin",
                "protected_client_session_binding",
                "project_command_challenge",
            ],
        ),
        RegisteredOperation::query(
            &GET_EDITOR_SESSION,
            &[
                "server_derived_project_scope",
                "session_scope_join",
                "protected_client_session_binding",
            ],
        ),
    ],
    schemas: || {
        vec![
            GeneratedSchema {
                schema_id: CREATE_EDITOR_SESSION_REQUEST_SCHEMA_ID,
                path: EDITOR_SESSION_CREATE_REQUEST_SCHEMA_PATH,
                bytes: json_bytes(&schema_value::<CreateEditorSessionRequest>(
                    CREATE_EDITOR_SESSION_REQUEST_SCHEMA_ID,
                    "StoryOS Create Editor Session Request",
                )),
            },
            GeneratedSchema {
                schema_id: CREATE_EDITOR_SESSION_RESPONSE_SCHEMA_ID,
                path: EDITOR_SESSION_CREATE_RESPONSE_SCHEMA_PATH,
                bytes: json_bytes(&session_response_schema::<CreateEditorSessionResponse>(
                    CREATE_EDITOR_SESSION_RESPONSE_SCHEMA_ID,
                    "StoryOS Create Editor Session Response",
                )),
            },
            GeneratedSchema {
                schema_id: GET_EDITOR_SESSION_REQUEST_SCHEMA_ID,
                path: EDITOR_SESSION_GET_REQUEST_SCHEMA_PATH,
                bytes: json_bytes(&path_request_schema(
                    GET_EDITOR_SESSION_REQUEST_SCHEMA_ID,
                    &["project_id", "editor_session_id"],
                )),
            },
            GeneratedSchema {
                schema_id: GET_EDITOR_SESSION_RESPONSE_SCHEMA_ID,
                path: EDITOR_SESSION_GET_RESPONSE_SCHEMA_PATH,
                bytes: json_bytes(&session_response_schema::<GetEditorSessionResponse>(
                    GET_EDITOR_SESSION_RESPONSE_SCHEMA_ID,
                    "StoryOS Get Editor Session Response",
                )),
            },
        ]
    },
    openapi: || {
        let mut methods = method(&CREATE_EDITOR_SESSION, editor_session_create_openapi());
        methods.extend(method(
            &GET_EDITOR_SESSION,
            query_openapi(
                &GET_EDITOR_SESSION,
                "Read one exact Editor Session",
                EDITOR_SESSION_GET_RESPONSE_SCHEMA_PATH,
                &["project_id", "editor_session_id"],
            ),
        ));
        methods
    },
    typescript_types: || {
        let config = Config::default();
        [
            CreateEditorSessionRequest::decl(&config),
            EditorReadOnlyReason::decl(&config),
            EditorWriterProjection::decl(&config),
            EditorSessionBinding::decl(&config),
            EditorBaseSnapshot::decl(&config),
            CreateEditorSessionResponse::decl(&config),
            GetEditorSessionResponse::decl(&config),
        ]
        .map(|declaration| format!("export {declaration}"))
        .join("\n\n")
    },
    typescript_client: || {
        format!(
            concat!(
                "\nexport async function digestCreateEditorSession(request, cryptoImpl = globalThis.crypto) {{\n",
                "  if (!request || typeof request !== \"object\") throw new TypeError(\"digestCreateEditorSession requires request\");\n",
                "  const canonical = {{ client_contract_revision: request.client_contract_revision, command_schema: request.command_schema, correlation_id: request.correlation_id, security_policy_revision: request.security_policy_revision }};\n",
                "  const bytes = new TextEncoder().encode(JSON.stringify(canonical));\n",
                "  const digest = new Uint8Array(await cryptoImpl.subtle.digest(\"SHA-256\", bytes));\n",
                "  return {{ algorithm: \"sha256\", profile: \"storyos.command.createEditorSession.jcs.v1\", value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, \"0\")).join(\"\") }};\n}}\n",
                "\nexport async function createEditorSession({{ projectId, request, idempotencyKey, antiForgery, ...options }} = {{}}) {{\n",
                "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"createEditorSession requires projectId\");\n",
                "  if (!request || typeof request !== \"object\") throw new TypeError(\"createEditorSession requires request\");\n",
                "  if (typeof idempotencyKey !== \"string\" || typeof antiForgery !== \"string\") throw new TypeError(\"createEditorSession requires security bindings\");\n",
                "  return commandJson({{ ...options, path: `{}`, body: request, commandHeaders: {{ \"idempotency-key\": idempotencyKey, \"x-storyos-anti-forgery\": antiForgery }} }});\n}}\n",
                "\nexport async function getEditorSession({{ projectId, editorSessionId, ...options }} = {{}}) {{\n",
                "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"getEditorSession requires projectId\");\n",
                "  if (typeof editorSessionId !== \"string\" || editorSessionId.length === 0) throw new TypeError(\"getEditorSession requires editorSessionId\");\n",
                "  return queryJson({{ ...options, path: `{}` }});\n}}\n",
            ),
            CREATE_EDITOR_SESSION
                .path
                .replace("{project_id}", "${encodeURIComponent(projectId)}"),
            GET_EDITOR_SESSION
                .path
                .replace("{project_id}", "${encodeURIComponent(projectId)}")
                .replace(
                    "{editor_session_id}",
                    "${encodeURIComponent(editorSessionId)}"
                ),
        )
    },
    typescript_declarations: || {
        concat!(
            "export declare function digestCreateEditorSession(request: CreateEditorSessionRequest, cryptoImpl?: Crypto): Promise<DigestValue>;\n",
            "export declare function createEditorSession(options: StoryOSQueryOptions & { projectId: string; request: CreateEditorSessionRequest; idempotencyKey: string; antiForgery: string }): Promise<CreateEditorSessionResponse>;\n",
            "export declare function getEditorSession(options: StoryOSQueryOptions & { projectId: string; editorSessionId: string }): Promise<GetEditorSessionResponse>;\n",
        )
    },
    fixtures: || {
        [
            fixture_triple(
                CREATE_EDITOR_SESSION_FIXTURE_PATHS,
                &CREATE_EDITOR_SESSION,
                [
                    |_| create_editor_session_fixture_bytes(),
                    |_| invalid_create_editor_session_fixture_bytes(),
                    |_| boundary_create_editor_session_fixture_bytes(),
                ],
            ),
            fixture_triple(
                GET_EDITOR_SESSION_FIXTURE_PATHS,
                &GET_EDITOR_SESSION,
                [
                    |_| get_editor_session_fixture_bytes(),
                    |_| invalid_get_editor_session_fixture_bytes(),
                    |_| boundary_get_editor_session_fixture_bytes(),
                ],
            ),
        ]
        .concat()
    },
};

fn editor_session_create_openapi() -> String {
    let request_schema = EDITOR_SESSION_CREATE_REQUEST_SCHEMA_PATH
        .strip_prefix("generated/")
        .unwrap();
    let response_schema = EDITOR_SESSION_CREATE_RESPONSE_SCHEMA_PATH
        .strip_prefix("generated/")
        .unwrap();
    let responses = CREATE_EDITOR_SESSION.responses.iter().map(|(status, description)| format!(
        "        '{status}':\n          description: {description}\n{}",
        if matches!(status, 200 | 201) { format!("          content:\n            application/json:\n              schema:\n                $ref: '../{response_schema}'\n") } else { String::new() }
    )).collect::<String>();
    format!(
        "    post:\n      operationId: {}\n      summary: Create one Editor Session\n      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{request_schema}'\n      responses:\n{responses}",
        CREATE_EDITOR_SESSION.operation_id
    )
}

fn session_response_schema<T: schemars::JsonSchema>(schema_id: &str, title: &str) -> Value {
    let mut schema = schema_value::<T>(schema_id, title);
    apply_u64_wire_constraints(&mut schema);
    schema
}

fn apply_u64_wire_constraints(schema: &mut Value) {
    let canonical_u64 = canonical_u64_wire_schema();
    schema["$defs"]["EditorSessionBinding"]["properties"]["client_session_generation"] =
        canonical_u64.clone();
    schema["$defs"]["EditorBaseSnapshot"]["properties"]["project_activity_position"] =
        canonical_u64.clone();
    schema["$defs"]["EditorWriterProjection"]["oneOf"][0]["properties"]["writer_generation"] =
        canonical_u64.clone();
    schema["$defs"]["EditorWriterProjection"]["oneOf"][1]["properties"]["observed_writer_generation"] =
        canonical_u64;
}

fn editor_session_fixture(schema_id: &str) -> Value {
    json!({
        "schema_id": schema_id,
        "correlation_id": "018f0000-0000-7001-8000-000000000020",
        "project_scope": {"owner_user_id": "018f0000-0000-7001-8000-000000000001", "project_id": "018f0000-0000-7001-8000-000000000002"},
        "editor_session": {
            "editor_session_id": "018f0000-0000-7001-8000-000000000021",
            "client_session_binding_ref": "binding:7af2", "client_session_generation": "1",
            "client_contract_revision": WEB_CLIENT_CONTRACT_REVISION,
            "security_policy_revision": "storyos.web-security-policy.release-1.v1",
            "opened_at": "2026-08-13T08:00:00.000Z", "disposition": "open"
        },
        "writer": {"kind": "current_writer", "writer_generation": "1"},
        "base_snapshot": {
            "snapshot_id": "018f0000-0000-7001-8000-000000000022",
            "chapter_id": "018f0000-0000-7001-8000-000000000003",
            "project_activity_position": "0",
            "authoritative_head_revision_id": "018f0000-0000-7001-8000-000000000004",
            "proposal_head_revision_ids": [], "target_refs": ["manuscript:018f0000-0000-7001-8000-000000000003"],
            "observed_ownership_partition": "authoritative",
            "materialized_revision": {"revision_id": "018f0000-0000-7001-8000-000000000004", "body": "雨落在窗沿。", "blocks": [{"manuscript_block_id": "018f0000-0000-7001-8000-0000000000b1", "block_kind": "paragraph", "text": "雨落在窗沿。"}]},
            "materialized_payload_digest": {"algorithm": "sha256", "profile": "storyos.canonical-payload.sha256.v1", "value_hex_lowercase": "b".repeat(64)},
            "created_at": "2026-08-13T08:00:00.000Z"
        }
    })
}

fn create_editor_session_fixture_bytes() -> Vec<u8> {
    json_bytes(&editor_session_fixture(
        CREATE_EDITOR_SESSION_RESPONSE_SCHEMA_ID,
    ))
}

fn get_editor_session_fixture_bytes() -> Vec<u8> {
    json_bytes(&editor_session_fixture(
        GET_EDITOR_SESSION_RESPONSE_SCHEMA_ID,
    ))
}

fn invalid_create_editor_session_fixture_bytes() -> Vec<u8> {
    without_project_scope(editor_session_fixture(
        CREATE_EDITOR_SESSION_RESPONSE_SCHEMA_ID,
    ))
}

fn boundary_create_editor_session_fixture_bytes() -> Vec<u8> {
    with_boundary_project_scope(editor_session_fixture(
        CREATE_EDITOR_SESSION_RESPONSE_SCHEMA_ID,
    ))
}

fn invalid_get_editor_session_fixture_bytes() -> Vec<u8> {
    without_project_scope(editor_session_fixture(
        GET_EDITOR_SESSION_RESPONSE_SCHEMA_ID,
    ))
}

fn boundary_get_editor_session_fixture_bytes() -> Vec<u8> {
    with_boundary_project_scope(editor_session_fixture(
        GET_EDITOR_SESSION_RESPONSE_SCHEMA_ID,
    ))
}
