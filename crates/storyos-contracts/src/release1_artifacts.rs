use std::fs;
use std::io;
use std::path::Path;
use std::sync::LazyLock;

use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::digest::sha256_prefixed;
use crate::release1::{
    ACTIVITY_PROFILE, API_MAJOR, ArtifactDigests, AuthoritativeChapterRevision,
    CHAPTER_REQUEST_SCHEMA_ID, CHAPTER_RESPONSE_SCHEMA_ID, COMPATIBILITY_PROFILE,
    CONTRACT_REVISION, CREATE_EDITOR_SESSION, CREATE_EDITOR_SESSION_REQUEST_SCHEMA_ID,
    CREATE_EDITOR_SESSION_RESPONSE_SCHEMA_ID, CREATE_PROJECT_COMMAND_CHALLENGE, ControlledProject,
    CreateEditorSessionRequest, CreateEditorSessionResponse, CreateProjectCommandChallengeRequest,
    CreateProjectCommandChallengeResponse, CurrentChapter, DigestAlgorithm, DigestValue,
    ENVELOPE_PROFILE, ENVELOPE_VERSION, EditorBaseSnapshot, EditorReadOnlyReason,
    EditorSessionBinding, EditorWriterProjection, GENERATED_CLIENT_REVISION, GET_CHAPTER,
    GET_EDITOR_SESSION, GET_EDITOR_SESSION_REQUEST_SCHEMA_ID,
    GET_EDITOR_SESSION_RESPONSE_SCHEMA_ID, GET_PROJECT, GET_PROTOCOL_PROFILE, GetChapterResponse,
    GetEditorSessionResponse, GetProjectResponse, LIMIT_PROFILE_REVISION, ManuscriptBlock,
    ManuscriptBlockKind, PROBLEM_PROFILE, PROJECT_COMMAND_CHALLENGE_REQUEST_SCHEMA_ID,
    PROJECT_COMMAND_CHALLENGE_RESPONSE_SCHEMA_ID, PROJECT_REQUEST_SCHEMA_ID,
    PROJECT_RESPONSE_SCHEMA_ID, PROTOCOL_PROFILE_REQUEST_SCHEMA_ID, PROTOCOL_PROFILE_SCHEMA_ID,
    PUBLIC_PROTOCOL_RELEASE, ProjectOpenState, ProjectScope, QueryOperation,
    RELEASE_IDENTITY_SCHEMA_ID, REQUIRED_CAPABILITIES, Release1CompatibilityIdentity,
    Release1ProtocolProfile, SERVER_CONTRACT_REVISION, WEB_CLIENT_CONTRACT_REVISION,
    WORKER_CONTRACT_REVISION, protocol_profile,
};
use crate::release1_operation_registry::{
    ContractGraphEntry, GeneratedSchema, OpenApiMethod, OperationArtifacts, OperationKind,
    RELEASE1_OPERATIONS, RegisteredOperation, fixture_triple, method,
};
use crate::release1_wire::json_bytes;
const FIXTURE_DIGEST_PLACEHOLDER: &str = "sha256:self-normalized";
const OPENAPI_PATH: &str = "generated/openapi/storyos-public-release-1.yaml";
const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/protocol-profile-request.schema.json";
const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/protocol-profile-response.schema.json";
const PROJECT_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/project-request.schema.json";
const PROJECT_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/project-response.schema.json";
const CHAPTER_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/chapter-request.schema.json";
const CHAPTER_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/chapter-response.schema.json";
const CHALLENGE_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/project-command-challenge-request.schema.json";
const CHALLENGE_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/project-command-challenge-response.schema.json";
pub(super) const EDITOR_SESSION_CREATE_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/editor-session-create-request.schema.json";
pub(super) const EDITOR_SESSION_CREATE_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/editor-session-create-response.schema.json";
pub(super) const EDITOR_SESSION_GET_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/editor-session-get-request.schema.json";
pub(super) const EDITOR_SESSION_GET_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/editor-session-get-response.schema.json";
const TYPESCRIPT_CLIENT_PATH: &str = "generated/typescript/storyos-public-release-1/client.mjs";
const PROJECT_ACTIVITY_MODULE_PATH: &str =
    "generated/typescript/storyos-public-release-1/project-activity.mjs";
const PROJECT_ACTIVITY_DECLARATION_PATH: &str =
    "generated/typescript/storyos-public-release-1/project-activity.d.mts";
const TYPESCRIPT_DECLARATION_PATH: &str =
    "generated/typescript/storyos-public-release-1/client.d.mts";
const RELEASE_PROFILE_MODULE_PATH: &str =
    "generated/typescript/storyos-public-release-1/release-profile.mjs";
const RELEASE_PROFILE_DECLARATION_PATH: &str =
    "generated/typescript/storyos-public-release-1/release-profile.d.mts";
const SCHEMA_CATALOG_PATH: &str = "generated/schema-catalog/storyos-public-release-1.json";
const FIXTURE_CATALOG_PATH: &str = "generated/fixtures/storyos-public-release-1.json";
const GOLDEN_PROFILE_PATH: &str =
    "generated/golden-wire/storyos-public-release-1/get-protocol-profile.json";
const INVALID_PROFILE_PATH: &str =
    "generated/golden-wire/storyos-public-release-1/get-protocol-profile.invalid.json";
const BOUNDARY_PROFILE_PATH: &str =
    "generated/golden-wire/storyos-public-release-1/get-protocol-profile.boundary.json";
const PROJECT_FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/get-project.json",
    "generated/golden-wire/storyos-public-release-1/get-project.invalid.json",
    "generated/golden-wire/storyos-public-release-1/get-project.boundary.json",
];
const CHAPTER_FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/get-chapter.json",
    "generated/golden-wire/storyos-public-release-1/get-chapter.invalid.json",
    "generated/golden-wire/storyos-public-release-1/get-chapter.boundary.json",
];
const CHALLENGE_FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/create-project-command-challenge.json",
    "generated/golden-wire/storyos-public-release-1/create-project-command-challenge.invalid.json",
    "generated/golden-wire/storyos-public-release-1/create-project-command-challenge.boundary.json",
];
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
const REVIEW_CATALOG_PATH: &str = "docs/foundation/versioned-protocol-release-1-route-catalog.json";
const REVIEW_CATALOG_SHA256: &str =
    "sha256:724246c75a29c503e9deee608707399dbd7277928724854d23418b63093a7595";
const REVIEWED_CONTRACT_GRAPH_SHA256: &str =
    "sha256:e18e4b121933df479b274b93a57a3ae93cb638a442e6329de006d8d670fc9f68";

type GeneratedFile = (&'static str, Vec<u8>);

#[path = "release1_fixture_corpus.rs"]
mod release1_fixture_corpus;
use release1_fixture_corpus::{
    fixture_catalog_bytes, fixture_corpus_bytes, generated_fixture_files,
};

struct Release1ArtifactAssembly {
    openapi: Vec<u8>,
    schemas: Vec<GeneratedSchema>,
    schema_catalog: Vec<u8>,
    typescript_client: Vec<u8>,
    typescript_declaration: Vec<u8>,
    release_profile_declaration: Vec<u8>,
    profile: Release1ProtocolProfile,
}

fn release1_artifact_assembly() -> Release1ArtifactAssembly {
    let openapi = openapi_bytes();
    let schemas = RELEASE1_OPERATIONS
        .iter()
        .flat_map(|artifacts| (artifacts.schemas)())
        .collect::<Vec<_>>();
    let schema_catalog = schema_catalog_bytes(&schemas);
    let typescript_client = typescript_client_bytes();
    let typescript_declaration = typescript_declaration_bytes();
    let release_profile_declaration = release_profile_declaration_bytes();
    let contract_graph = contract_graph_bytes();
    let typescript = [
        typescript_client.as_slice(),
        b"\n--declaration--\n",
        typescript_declaration.as_slice(),
        b"\n--release-profile-declaration--\n",
        release_profile_declaration.as_slice(),
    ]
    .concat();

    let mut profile = protocol_profile(ArtifactDigests {
        contract_graph: sha256_prefixed(contract_graph.as_slice()),
        openapi: sha256_prefixed(openapi.as_slice()),
        json_schema_catalog: sha256_prefixed(schema_catalog.as_slice()),
        typescript: sha256_prefixed(typescript),
        fixture_corpus: FIXTURE_DIGEST_PLACEHOLDER.to_owned(),
    });
    profile.release_identity.fixture_corpus_digest =
        sha256_prefixed(fixture_corpus_bytes(&profile));
    Release1ArtifactAssembly {
        openapi,
        schemas,
        schema_catalog,
        typescript_client,
        typescript_declaration,
        release_profile_declaration,
        profile,
    }
}

/// Build the one active Release 1 protocol profile from the Rust contract source.
pub fn release1_protocol_profile() -> Release1ProtocolProfile {
    static PROFILE: LazyLock<Release1ProtocolProfile> =
        LazyLock::new(|| release1_artifact_assembly().profile);
    PROFILE.clone()
}

/// Write all checked-in artifacts for the implemented Release 1 profile route.
pub fn write_release1_artifacts(repo_root: &Path) -> io::Result<()> {
    check_review_catalog(repo_root)?;
    for (relative_path, bytes) in generated_files() {
        let path = repo_root.join(relative_path);
        let parent = path.parent().expect("generated artifact path has a parent");
        fs::create_dir_all(parent).map_err(|source| path_error(parent, source))?;
        fs::write(&path, bytes).map_err(|source| path_error(&path, source))?;
    }
    Ok(())
}

/// Fail when any checked-in generated artifact differs from Rust-source generation.
pub fn check_release1_artifacts(repo_root: &Path) -> io::Result<()> {
    check_review_catalog(repo_root)?;
    for (relative_path, expected) in generated_files() {
        let path = repo_root.join(relative_path);
        let actual = match fs::read(&path) {
            Ok(actual) => actual,
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                return Err(stale_artifact(&path));
            }
            Err(source) => return Err(path_error(&path, source)),
        };
        if actual != expected {
            return Err(stale_artifact(&path));
        }
    }
    Ok(())
}

fn generated_files() -> Vec<GeneratedFile> {
    let Release1ArtifactAssembly {
        openapi,
        schemas,
        schema_catalog,
        typescript_client,
        typescript_declaration,
        release_profile_declaration,
        profile,
    } = release1_artifact_assembly();
    let profile_json =
        serde_json::to_string_pretty(&profile).expect("protocol profile should serialize");
    let profile_module = format!("// @generated by storyos-contracts; do not edit.\nexport const RELEASE_1_PROTOCOL_PROFILE = Object.freeze({profile_json});\n").into_bytes();
    let mut generated = vec![
        (
            "generated/json-schema/storyos-web-assets/manifest.schema.json",
            json_bytes(&typed_schema::<crate::WebAssetManifest>(
                crate::WEB_ASSET_SCHEMA,
                "StoryOS Web Asset Manifest",
            )),
        ),
        (OPENAPI_PATH, openapi),
    ];
    generated.extend(
        schemas
            .into_iter()
            .map(|schema| (schema.path, schema.bytes)),
    );
    generated.extend([
        (TYPESCRIPT_CLIENT_PATH, typescript_client),
        (TYPESCRIPT_DECLARATION_PATH, typescript_declaration),
        (PROJECT_ACTIVITY_MODULE_PATH, {
            let events = crate::ProjectActivityKind::ALL
                .iter()
                .map(|kind| (kind.as_str(), kind.event_schemas()))
                .collect::<std::collections::BTreeMap<_, _>>();
            let json = serde_json::to_string_pretty(&events)
                .expect("implemented Project Activity events should serialize");
            format!("// @generated by storyos-contracts; do not edit.\nexport const IMPLEMENTED_PROJECT_ACTIVITY_EVENTS = Object.freeze({json});\n")
                .into_bytes()
        }),
        (
            PROJECT_ACTIVITY_DECLARATION_PATH,
            b"// @generated by storyos-contracts; do not edit.\nexport declare const IMPLEMENTED_PROJECT_ACTIVITY_EVENTS: Readonly<Record<string, readonly string[]>>;\n".to_vec(),
        ),
        (RELEASE_PROFILE_MODULE_PATH, profile_module),
        (
            RELEASE_PROFILE_DECLARATION_PATH,
            release_profile_declaration,
        ),
        (SCHEMA_CATALOG_PATH, schema_catalog),
        (FIXTURE_CATALOG_PATH, fixture_catalog_bytes(&profile)),
    ]);
    generated.extend(generated_fixture_files(&profile));
    generated
}

fn path_error(path: &Path, source: io::Error) -> io::Error {
    io::Error::new(source.kind(), format!("{}: {source}", path.display()))
}

fn stale_artifact(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "{} is missing or stale; run `make generate-contracts`",
            path.display()
        ),
    )
}

fn check_review_catalog(repo_root: &Path) -> io::Result<()> {
    let path = repo_root.join(REVIEW_CATALOG_PATH);
    let bytes = fs::read(&path).map_err(|source| path_error(&path, source))?;
    validate_review_bindings(&bytes)
}

fn validate_review_bindings(catalog: &[u8]) -> io::Result<()> {
    let catalog_digest = sha256_prefixed(catalog);
    let graph_digest = sha256_prefixed(contract_graph_bytes());
    if catalog_digest != REVIEW_CATALOG_SHA256 || graph_digest != REVIEWED_CONTRACT_GRAPH_SHA256 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Release 1 profile drifted: catalog {catalog_digest}; graph {graph_digest}"),
        ));
    }
    Ok(())
}

fn contract_graph_bytes() -> Vec<u8> {
    let operations = registered_operations()
        .filter_map(|registered| match registered.graph {
            ContractGraphEntry::Preconditions(preconditions) => Some(operation_graph(
                registered.operation,
                registered.kind,
                preconditions,
            )),
            ContractGraphEntry::Absent => None,
        })
        .collect::<Vec<_>>();
    serde_json::to_vec(&json!({
        "schema_id": "storyos.contract-graph.release-1-profile-slice.v1",
        "contract_revision": CONTRACT_REVISION,
        "review_catalog_sha256": REVIEW_CATALOG_SHA256,
        "operations": operations,
        "release": {
            "api_major": API_MAJOR, "public_protocol_release": PUBLIC_PROTOCOL_RELEASE, "envelope_version": ENVELOPE_VERSION,
            "envelope_profile": ENVELOPE_PROFILE, "problem_profile": PROBLEM_PROFILE, "activity_profile": ACTIVITY_PROFILE,
            "limit_profile_revision": LIMIT_PROFILE_REVISION, "compatibility_profile": COMPATIBILITY_PROFILE,
            "release_identity_schema": RELEASE_IDENTITY_SCHEMA_ID, "mismatch_code": "upgrade_required",
            "web_client_contract_revision": WEB_CLIENT_CONTRACT_REVISION, "server_contract_revision": SERVER_CONTRACT_REVISION,
            "worker_contract_revision": WORKER_CONTRACT_REVISION, "generated_client_revision": GENERATED_CLIENT_REVISION,
            "required_capabilities": REQUIRED_CAPABILITIES,
        },
    }))
    .expect("the contract graph contains only serializable values")
}

fn registered_operations() -> impl Iterator<Item = &'static RegisteredOperation> {
    RELEASE1_OPERATIONS
        .iter()
        .flat_map(|artifacts| artifacts.operations)
}

fn operation_graph(
    operation: &QueryOperation,
    kind: OperationKind,
    preconditions: &[&str],
) -> Value {
    json!({
        "operation_id": operation.operation_id, "kind": kind.as_str(), "method": operation.method,
        "path": operation.path, "request_schema": operation.request_schema,
        "response_schema": operation.response_schema, "preconditions": preconditions,
        "http_statuses": operation.responses.iter().map(|(status, _)| status).collect::<Vec<_>>(),
        "fixtures": operation.fixtures,
        "generated": ["openapi", "json_schema", "typescript_client", "golden_wire"]
    })
}

fn openapi_bytes() -> Vec<u8> {
    let mut path_items: Vec<OpenApiMethod> = Vec::new();
    for method in RELEASE1_OPERATIONS
        .iter()
        .flat_map(|artifacts| (artifacts.openapi)())
    {
        match path_items.iter_mut().find(|item| item.path == method.path) {
            Some(item) => item.yaml.push_str(&method.yaml),
            None => path_items.push(method),
        }
    }
    let paths = path_items
        .iter()
        .map(|item| format!("  {}:\n{}", item.path, item.yaml))
        .collect::<String>();
    let implemented_slice = implemented_operation_ids().join(",");
    format!(
        "openapi: 3.1.0\ninfo:\n  title: StoryOS Public Release 1\n  version: {PUBLIC_PROTOCOL_RELEASE}\n  x-storyos-contract-revision: {CONTRACT_REVISION}\n  x-storyos-implemented-slice: {implemented_slice}\npaths:\n{paths}components: {{}}\n",
    ).into_bytes()
}

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

fn challenge_openapi() -> String {
    let operation = &CREATE_PROJECT_COMMAND_CHALLENGE;
    let request_schema = CHALLENGE_REQUEST_SCHEMA_PATH
        .strip_prefix("generated/")
        .unwrap();
    let response_schema = CHALLENGE_RESPONSE_SCHEMA_PATH
        .strip_prefix("generated/")
        .unwrap();
    let responses = operation.responses.iter().map(|(status, description)| format!(
        "        '{status}':\n          description: {description}\n{}{}",
        if *status == 429 { "          headers:\n            Retry-After:\n              required: true\n              schema:\n                type: integer\n                minimum: 1\n                maximum: 60\n" } else { "" },
        if *status == 200 { format!("          content:\n            application/json:\n              schema:\n                $ref: '../{response_schema}'\n") } else { String::new() }
    )).collect::<String>();
    format!(
        "    post:\n      operationId: {}\n      summary: Issue one exact Project command challenge\n      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{request_schema}'\n      responses:\n{responses}",
        operation.operation_id,
    )
}

fn operation_openapi(
    operation: &QueryOperation,
    summary: &str,
    response_schema: &str,
    parameters: &[&str],
) -> String {
    let response_schema = response_schema
        .strip_prefix("generated/")
        .expect("OpenAPI response schemas must be generated artifacts");
    let parameters = if parameters.is_empty() {
        String::new()
    } else {
        format!(
            "      parameters:\n{}",
            parameters
                .iter()
                .map(|name| format!("        - name: {name}\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n"))
                .collect::<String>()
        )
    };
    let responses = operation.responses.iter().map(|(status, description)| format!(
        "        '{status}':\n          description: {description}\n{}",
        if *status == 200 { format!("          content:\n            application/json:\n              schema:\n                $ref: '../{response_schema}'\n") } else { String::new() }
    )).collect::<String>();
    format!(
        "    {}:\n      operationId: {}\n      summary: {summary}\n{parameters}      responses:\n{responses}",
        operation.method.to_ascii_lowercase(),
        operation.operation_id,
    )
}

fn protocol_profile_request_schema() -> Value {
    json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "$id": PROTOCOL_PROFILE_REQUEST_SCHEMA_ID,
           "title": "StoryOS Release 1 Protocol Profile Request", "type": "object",
           "additionalProperties": false, "maxProperties": 0})
}

fn protocol_profile_schema() -> Value {
    typed_schema::<Release1ProtocolProfile>(
        PROTOCOL_PROFILE_SCHEMA_ID,
        "StoryOS Release 1 Protocol Profile",
    )
}

fn typed_schema<T: schemars::JsonSchema>(schema_id: &str, title: &str) -> Value {
    let mut schema =
        serde_json::to_value(schema_for!(T)).expect("contract schema should serialize");
    schema["$id"] = Value::String(schema_id.to_owned());
    schema["title"] = Value::String(title.to_owned());
    if matches!(
        schema_id,
        CREATE_EDITOR_SESSION_RESPONSE_SCHEMA_ID | GET_EDITOR_SESSION_RESPONSE_SCHEMA_ID
    ) {
        apply_u64_wire_constraints(&mut schema);
    }
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

fn canonical_u64_wire_schema() -> Value {
    json!({
        "type": "string",
        "pattern": "^(?:0|[1-9][0-9]{0,18}|1[0-7][0-9]{18}|18[0-3][0-9]{17}|184[0-3][0-9]{16}|1844[0-5][0-9]{15}|18446[0-6][0-9]{14}|184467[0-3][0-9]{13}|1844674[0-3][0-9]{12}|184467440[0-6][0-9]{10}|1844674407[0-2][0-9]{9}|18446744073[0-6][0-9]{8}|1844674407370[0-8][0-9]{6}|18446744073709[0-4][0-9]{5}|184467440737095[0-4][0-9]{3}|1844674407370955[0-9]{2}|18446744073709551[0-5]|1844674407370955160|1844674407370955161[0-5])$"
    })
}

fn challenge_response_schema() -> Value {
    let mut schema = typed_schema::<CreateProjectCommandChallengeResponse>(
        PROJECT_COMMAND_CHALLENGE_RESPONSE_SCHEMA_ID,
        "StoryOS Project Command Challenge Response",
    );
    schema["properties"]["expires_at"]["format"] = Value::String("date-time".to_owned());
    schema
}

fn path_request_schema(schema_id: &str, fields: &[&str]) -> Value {
    let properties = fields
        .iter()
        .map(|field| {
            (
                (*field).to_owned(),
                json!({"type": "string", "format": "uuid"}),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "$id": schema_id,
           "type": "object", "additionalProperties": false, "required": fields,
           "properties": properties})
}

fn schema_catalog_bytes(schemas: &[GeneratedSchema]) -> Vec<u8> {
    json_bytes(&json!({
        "schema_id": "storyos.schema-catalog.v1", "public_protocol_release": PUBLIC_PROTOCOL_RELEASE,
        "implemented_operations": implemented_operation_ids(),
        "schemas": schemas.iter().map(|schema| json!({
            "schema_id": schema.schema_id, "path": schema.path, "sha256": sha256_prefixed(&schema.bytes)
        })).collect::<Vec<_>>()
    }))
}

fn implemented_operation_ids() -> Vec<&'static str> {
    registered_operations()
        .map(|registered| registered.operation.operation_id)
        .collect()
}

fn typescript_client_bytes() -> Vec<u8> {
    let preamble = format!(
        concat!(
            "// @generated by storyos-contracts; do not edit.\n",
            "export const GENERATED_CLIENT_REVISION = \"{}\";\n\n",
            "export class StoryOSProtocolError extends Error {{\n",
            "  constructor(code, message, details = {{}}) {{\n    super(message);\n    this.name = \"StoryOSProtocolError\";\n",
            "    this.code = code;\n    this.status = details.status;\n    this.responseBody = details.responseBody;\n    this.retryAfterSeconds = details.retryAfterSeconds;\n  }}\n}}\n\n",
            "async function queryJson({{ baseUrl, path, queryHeaders = {{}}, fetchImpl = globalThis.fetch, signal }}) {{\n",
            "  if (typeof baseUrl !== \"string\" || baseUrl.length === 0) throw new TypeError(\"StoryOS query requires a non-empty baseUrl\");\n",
            "  if (typeof fetchImpl !== \"function\") throw new TypeError(\"StoryOS query requires a fetch implementation\");\n",
            "  const headers = {{ accept: \"application/json\", ...queryHeaders }};\n",
            "  const response = await fetchImpl(new URL(path, baseUrl), {{ method: \"GET\", headers, credentials: \"same-origin\", signal }});\n",
            "  const responseBody = await response.text();\n",
            "  if (!response.ok) throw new StoryOSProtocolError(\"query_http_error\", `StoryOS query failed with HTTP ${{response.status}}`, {{ status: response.status, responseBody }});\n",
            "  try {{ return JSON.parse(responseBody); }} catch {{\n",
            "    throw new StoryOSProtocolError(\"query_invalid_json\", \"StoryOS query returned invalid JSON\", {{ status: response.status, responseBody }});\n  }}\n}}\n\n",
            "async function queryPostJson({{ baseUrl, path, body, queryHeaders = {{}}, fetchImpl = globalThis.fetch, signal }}) {{\n",
            "  if (typeof baseUrl !== \"string\" || baseUrl.length === 0) throw new TypeError(\"StoryOS query requires a non-empty baseUrl\");\n",
            "  if (typeof fetchImpl !== \"function\") throw new TypeError(\"StoryOS query requires a fetch implementation\");\n",
            "  const headers = {{ accept: \"application/json\", \"content-type\": \"application/json\", ...queryHeaders }};\n",
            "  const response = await fetchImpl(new URL(path, baseUrl), {{ method: \"POST\", headers, credentials: \"same-origin\", body: JSON.stringify(body), signal }});\n",
            "  const responseBody = await response.text();\n",
            "  if (!response.ok) throw new StoryOSProtocolError(\"query_http_error\", `StoryOS query failed with HTTP ${{response.status}}`, {{ status: response.status, responseBody }});\n",
            "  try {{ return JSON.parse(responseBody); }} catch {{\n",
            "    throw new StoryOSProtocolError(\"query_invalid_json\", \"StoryOS query returned invalid JSON\", {{ status: response.status, responseBody }});\n  }}\n}}\n\n",
            "async function queryText({{ baseUrl, path, queryHeaders = {{}}, fetchImpl = globalThis.fetch, signal }}) {{\n",
            "  if (typeof baseUrl !== \"string\" || baseUrl.length === 0) throw new TypeError(\"StoryOS query requires a non-empty baseUrl\");\n",
            "  if (typeof fetchImpl !== \"function\") throw new TypeError(\"StoryOS query requires a fetch implementation\");\n",
            "  const response = await fetchImpl(new URL(path, baseUrl), {{ method: \"GET\", headers: {{ ...queryHeaders }}, credentials: \"same-origin\", signal }});\n",
            "  const responseBody = await response.text();\n",
            "  if (!response.ok) throw new StoryOSProtocolError(\"query_http_error\", `StoryOS query failed with HTTP ${{response.status}}`, {{ status: response.status, responseBody }});\n",
            "  return responseBody;\n}}\n\n",
            "async function commandJson({{ baseUrl, path, body, method = \"POST\", commandHeaders = {{}}, fetchImpl = globalThis.fetch, signal }}) {{\n",
            "  if (typeof baseUrl !== \"string\" || baseUrl.length === 0) throw new TypeError(\"StoryOS command requires a non-empty baseUrl\");\n",
            "  if (typeof fetchImpl !== \"function\") throw new TypeError(\"StoryOS command requires a fetch implementation\");\n",
            "  const response = await fetchImpl(new URL(path, baseUrl), {{ method, headers: {{ accept: \"application/json\", \"content-type\": \"application/json\", ...commandHeaders }}, credentials: \"same-origin\", body: JSON.stringify(body), signal }});\n",
            "  const responseBody = await response.text();\n",
            "  if (!response.ok) {{\n    const retryAfter = response.headers.get(\"retry-after\");\n    const retryAfterSeconds = /^(?:[1-9]|[1-5][0-9]|60)$/.test(retryAfter ?? \"\") ? Number(retryAfter) : undefined;\n    throw new StoryOSProtocolError(\"command_http_error\", `StoryOS command failed with HTTP ${{response.status}}`, {{ status: response.status, responseBody, retryAfterSeconds }});\n  }}\n",
            "  try {{ return JSON.parse(responseBody); }} catch {{ throw new StoryOSProtocolError(\"command_invalid_json\", \"StoryOS command returned invalid JSON\", {{ status: response.status, responseBody }}); }}\n}}\n\n",
            "function canonicalJson(value) {{\n  if (Array.isArray(value)) return value.map(canonicalJson);\n  if (value && typeof value === \"object\") return Object.fromEntries(Object.keys(value).sort().map((key) => [key, canonicalJson(value[key])]));\n  return value;\n}}\n",
        ),
        GENERATED_CLIENT_REVISION,
    );
    let operations = RELEASE1_OPERATIONS
        .iter()
        .map(|artifacts| (artifacts.typescript_client)())
        .collect::<String>();
    (preamble + &operations).into_bytes()
}

fn typescript_declaration_bytes() -> Vec<u8> {
    let config = Config::default();
    let shared_types = [
        ProjectScope::decl(&config),
        ProjectOpenState::decl(&config),
        ControlledProject::decl(&config),
        ManuscriptBlockKind::decl(&config),
        ManuscriptBlock::decl(&config),
        AuthoritativeChapterRevision::decl(&config),
        CurrentChapter::decl(&config),
        DigestAlgorithm::decl(&config),
        DigestValue::decl(&config),
    ]
    .map(|declaration| format!("export {declaration}"));
    let types = shared_types
        .into_iter()
        .chain(
            RELEASE1_OPERATIONS
                .iter()
                .map(|artifacts| (artifacts.typescript_types)()),
        )
        .collect::<Vec<_>>()
        .join("\n\n");
    let functions = RELEASE1_OPERATIONS
        .iter()
        .map(|artifacts| (artifacts.typescript_declarations)())
        .collect::<String>();
    format!(
        concat!(
            "// @generated by storyos-contracts; do not edit.\n{}\n\n",
            "export declare const GENERATED_CLIENT_REVISION: string;\n",
            "export declare class StoryOSProtocolError extends Error {{\n  readonly code: string;\n  readonly status?: number;\n  readonly responseBody?: string;\n  readonly retryAfterSeconds?: number;\n}}\n",
            "export interface StoryOSQueryOptions {{\n  baseUrl: string;\n  fetchImpl?: typeof fetch;\n  signal?: AbortSignal;\n}}\n",
            "{}",
        ),
        types, functions,
    )
    .into_bytes()
}

// Keep this small generator beside the digest and output list that own it. The locked TypeScript
// migration contract permits generator changes only in this module, so extracting it would create
// an unauthorized production seam.
fn release_profile_declaration_bytes() -> Vec<u8> {
    concat!(
        "// @generated by storyos-contracts; do not edit.\n",
        "import type { Release1ProtocolProfile } from \"./client.mjs\";\n",
        "export declare const RELEASE_1_PROTOCOL_PROFILE: ",
        "Readonly<Release1ProtocolProfile>;\n",
    )
    .as_bytes()
    .to_vec()
}

fn golden_profile_bytes(profile: &Release1ProtocolProfile) -> Vec<u8> {
    json_bytes(&serde_json::to_value(profile).expect("protocol profile should serialize"))
}

fn invalid_profile_bytes(profile: &Release1ProtocolProfile) -> Vec<u8> {
    let mut invalid = serde_json::to_value(profile).expect("protocol profile should serialize");
    invalid
        .as_object_mut()
        .expect("protocol profile should be an object")
        .remove("release_identity");
    json_bytes(&invalid)
}

fn boundary_profile_bytes(profile: &Release1ProtocolProfile) -> Vec<u8> {
    let mut boundary = profile.clone();
    boundary.release_identity.generated_client_revision =
        "storyos.typescript-client.release-0.v1".to_owned();
    golden_profile_bytes(&boundary)
}

fn project_fixture() -> Value {
    json!({
        "schema_id": PROJECT_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000005",
        "project_scope": {"owner_user_id": "018f0000-0000-7001-8000-000000000001", "project_id": "018f0000-0000-7001-8000-000000000002"},
        "project": {"project_id": "018f0000-0000-7001-8000-000000000002", "title": "受控项目", "open": {"kind": "current_chapter", "current_chapter_id": "018f0000-0000-7001-8000-000000000003"}}
    })
}

fn chapter_fixture() -> Value {
    json!({
        "schema_id": CHAPTER_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000006",
        "project_scope": {"owner_user_id": "018f0000-0000-7001-8000-000000000001", "project_id": "018f0000-0000-7001-8000-000000000002"},
        "project_activity_position": "0",
        "chapter": {"chapter_id": "018f0000-0000-7001-8000-000000000003", "title": "第一章", "current_revision": {"revision_id": "018f0000-0000-7001-8000-000000000004", "body": "雨落在窗沿。", "blocks": [{"manuscript_block_id": "018f0000-0000-7001-8000-0000000000b1", "block_kind": "paragraph", "text": "雨落在窗沿。"}]}}
    })
}

fn challenge_fixture() -> Value {
    json!({
        "nonce": "4f9f5ad05c4d1294d4114fb15595d831b64e3f4312a17e639213ad36e941ca71",
        "expires_at": "2026-08-12T08:05:00.000Z",
        "limit_profile_revision": LIMIT_PROFILE_REVISION
    })
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

fn invalid_fixture(mut value: Value) -> Vec<u8> {
    value
        .as_object_mut()
        .expect("fixture is an object")
        .remove("project_scope");
    json_bytes(&value)
}

fn boundary_fixture(mut value: Value) -> Vec<u8> {
    value["project_scope"]["project_id"] =
        Value::String("018f0000-0000-7001-8000-000000000102".to_owned());
    json_bytes(&value)
}

fn project_fixture_bytes() -> Vec<u8> {
    json_bytes(&project_fixture())
}
fn invalid_project_fixture_bytes() -> Vec<u8> {
    invalid_fixture(project_fixture())
}
fn boundary_project_fixture_bytes() -> Vec<u8> {
    boundary_fixture(project_fixture())
}
fn chapter_fixture_bytes() -> Vec<u8> {
    json_bytes(&chapter_fixture())
}
fn invalid_chapter_fixture_bytes() -> Vec<u8> {
    invalid_fixture(chapter_fixture())
}
fn boundary_chapter_fixture_bytes() -> Vec<u8> {
    boundary_fixture(chapter_fixture())
}
fn challenge_fixture_bytes() -> Vec<u8> {
    json_bytes(&challenge_fixture())
}
fn invalid_challenge_fixture_bytes() -> Vec<u8> {
    let mut invalid = challenge_fixture();
    invalid.as_object_mut().unwrap().remove("nonce");
    json_bytes(&invalid)
}
fn boundary_challenge_fixture_bytes() -> Vec<u8> {
    let mut boundary = challenge_fixture();
    boundary["expires_at"] = json!("1970-01-01T00:00:00.000Z");
    json_bytes(&boundary)
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
    invalid_fixture(editor_session_fixture(
        CREATE_EDITOR_SESSION_RESPONSE_SCHEMA_ID,
    ))
}
fn boundary_create_editor_session_fixture_bytes() -> Vec<u8> {
    boundary_fixture(editor_session_fixture(
        CREATE_EDITOR_SESSION_RESPONSE_SCHEMA_ID,
    ))
}
fn invalid_get_editor_session_fixture_bytes() -> Vec<u8> {
    invalid_fixture(editor_session_fixture(
        GET_EDITOR_SESSION_RESPONSE_SCHEMA_ID,
    ))
}
fn boundary_get_editor_session_fixture_bytes() -> Vec<u8> {
    boundary_fixture(editor_session_fixture(
        GET_EDITOR_SESSION_RESPONSE_SCHEMA_ID,
    ))
}

pub(super) const PROTOCOL_PROFILE_ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::query(
        &GET_PROTOCOL_PROFILE,
        &["active_public_release_profile"],
    )],
    schemas: || {
        vec![
            GeneratedSchema {
                schema_id: PROTOCOL_PROFILE_REQUEST_SCHEMA_ID,
                path: REQUEST_SCHEMA_PATH,
                bytes: json_bytes(&protocol_profile_request_schema()),
            },
            GeneratedSchema {
                schema_id: PROTOCOL_PROFILE_SCHEMA_ID,
                path: RESPONSE_SCHEMA_PATH,
                bytes: json_bytes(&protocol_profile_schema()),
            },
        ]
    },
    openapi: || {
        method(
            &GET_PROTOCOL_PROFILE,
            operation_openapi(
                &GET_PROTOCOL_PROFILE,
                "Discover the active StoryOS Release 1 protocol profile",
                RESPONSE_SCHEMA_PATH,
                &[],
            ),
        )
    },
    typescript_types: || {
        let config = Config::default();
        format!(
            "export {}\n\nexport {}",
            Release1CompatibilityIdentity::decl(&config),
            Release1ProtocolProfile::decl(&config),
        )
    },
    typescript_client: || {
        format!(
            "\nexport async function getProtocolProfile(options = {{}}) {{\n  return queryJson({{ ...options, path: \"{}\" }});\n}}\n",
            GET_PROTOCOL_PROFILE.path,
        )
    },
    typescript_declarations: || "export declare function getProtocolProfile(options: StoryOSQueryOptions): Promise<Release1ProtocolProfile>;\n",
    fixtures: || {
        fixture_triple(
            [
                GOLDEN_PROFILE_PATH,
                INVALID_PROFILE_PATH,
                BOUNDARY_PROFILE_PATH,
            ],
            &GET_PROTOCOL_PROFILE,
            [
                golden_profile_bytes,
                invalid_profile_bytes,
                boundary_profile_bytes,
            ],
        )
        .into()
    },
};

pub(super) const PROJECT_COMMAND_CHALLENGE_ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::challenge(
        &CREATE_PROJECT_COMMAND_CHALLENGE,
        &[
            "server_derived_project_scope",
            "strict_origin",
            "protected_client_session_binding",
            "route_method_schema_digest_match",
            "closed_command_schema",
        ],
    )],
    schemas: || {
        vec![
            GeneratedSchema {
                schema_id: PROJECT_COMMAND_CHALLENGE_REQUEST_SCHEMA_ID,
                path: CHALLENGE_REQUEST_SCHEMA_PATH,
                bytes: json_bytes(&typed_schema::<CreateProjectCommandChallengeRequest>(
                    PROJECT_COMMAND_CHALLENGE_REQUEST_SCHEMA_ID,
                    "StoryOS Project Command Challenge Request",
                )),
            },
            GeneratedSchema {
                schema_id: PROJECT_COMMAND_CHALLENGE_RESPONSE_SCHEMA_ID,
                path: CHALLENGE_RESPONSE_SCHEMA_PATH,
                bytes: json_bytes(&challenge_response_schema()),
            },
        ]
    },
    openapi: || method(&CREATE_PROJECT_COMMAND_CHALLENGE, challenge_openapi()),
    typescript_types: || {
        let config = Config::default();
        format!(
            "export {}\n\nexport {}",
            CreateProjectCommandChallengeRequest::decl(&config),
            CreateProjectCommandChallengeResponse::decl(&config),
        )
    },
    typescript_client: || {
        format!(
            concat!(
                "\nexport async function createProjectCommandChallenge({{ projectId, request, ...options }} = {{}}) {{\n",
                "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"createProjectCommandChallenge requires projectId\");\n",
                "  if (!request || typeof request !== \"object\") throw new TypeError(\"createProjectCommandChallenge requires request\");\n",
                "  return commandJson({{ ...options, path: `{}`, body: request }});\n}}\n",
            ),
            CREATE_PROJECT_COMMAND_CHALLENGE
                .path
                .replace("{project_id}", "${encodeURIComponent(projectId)}"),
        )
    },
    typescript_declarations: || "export declare function createProjectCommandChallenge(options: StoryOSQueryOptions & { projectId: string; request: CreateProjectCommandChallengeRequest }): Promise<CreateProjectCommandChallengeResponse>;\n",
    fixtures: || {
        fixture_triple(
            CHALLENGE_FIXTURE_PATHS,
            &CREATE_PROJECT_COMMAND_CHALLENGE,
            [
                |_| challenge_fixture_bytes(),
                |_| invalid_challenge_fixture_bytes(),
                |_| boundary_challenge_fixture_bytes(),
            ],
        )
        .into()
    },
};

pub(super) const PROJECT_QUERY_ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::query(
        &GET_PROJECT,
        &["server_derived_project_scope", "project_visibility"],
    )],
    schemas: || {
        vec![
            GeneratedSchema {
                schema_id: PROJECT_REQUEST_SCHEMA_ID,
                path: PROJECT_REQUEST_SCHEMA_PATH,
                bytes: json_bytes(&path_request_schema(
                    PROJECT_REQUEST_SCHEMA_ID,
                    &["project_id"],
                )),
            },
            GeneratedSchema {
                schema_id: PROJECT_RESPONSE_SCHEMA_ID,
                path: PROJECT_RESPONSE_SCHEMA_PATH,
                bytes: json_bytes(&typed_schema::<GetProjectResponse>(
                    PROJECT_RESPONSE_SCHEMA_ID,
                    "StoryOS Project Query Response",
                )),
            },
        ]
    },
    openapi: || {
        method(
            &GET_PROJECT,
            operation_openapi(
                &GET_PROJECT,
                "Read one controlled StoryOS Project",
                PROJECT_RESPONSE_SCHEMA_PATH,
                &["project_id"],
            ),
        )
    },
    typescript_types: || format!("export {}", GetProjectResponse::decl(&Config::default())),
    typescript_client: || {
        format!(
            concat!(
                "\nexport async function getProject({{ projectId, ...options }} = {{}}) {{\n",
                "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"getProject requires projectId\");\n",
                "  return queryJson({{ ...options, path: `{}` }});\n}}\n",
            ),
            GET_PROJECT
                .path
                .replace("{project_id}", "${encodeURIComponent(projectId)}"),
        )
    },
    typescript_declarations: || "export declare function getProject(options: StoryOSQueryOptions & { projectId: string }): Promise<GetProjectResponse>;\n",
    fixtures: || {
        fixture_triple(
            PROJECT_FIXTURE_PATHS,
            &GET_PROJECT,
            [
                |_| project_fixture_bytes(),
                |_| invalid_project_fixture_bytes(),
                |_| boundary_project_fixture_bytes(),
            ],
        )
        .into()
    },
};

pub(super) const EDITOR_SESSION_ARTIFACTS: OperationArtifacts = OperationArtifacts {
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
                bytes: json_bytes(&typed_schema::<CreateEditorSessionRequest>(
                    CREATE_EDITOR_SESSION_REQUEST_SCHEMA_ID,
                    "StoryOS Create Editor Session Request",
                )),
            },
            GeneratedSchema {
                schema_id: CREATE_EDITOR_SESSION_RESPONSE_SCHEMA_ID,
                path: EDITOR_SESSION_CREATE_RESPONSE_SCHEMA_PATH,
                bytes: json_bytes(&typed_schema::<CreateEditorSessionResponse>(
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
                bytes: json_bytes(&typed_schema::<GetEditorSessionResponse>(
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
            operation_openapi(
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

pub(super) const CHAPTER_QUERY_ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::query(
        &GET_CHAPTER,
        &[
            "server_derived_project_scope",
            "chapter_scope_join",
            "canonical_snapshot",
        ],
    )],
    schemas: || {
        vec![
            GeneratedSchema {
                schema_id: CHAPTER_REQUEST_SCHEMA_ID,
                path: CHAPTER_REQUEST_SCHEMA_PATH,
                bytes: json_bytes(&path_request_schema(
                    CHAPTER_REQUEST_SCHEMA_ID,
                    &["project_id", "chapter_id"],
                )),
            },
            GeneratedSchema {
                schema_id: CHAPTER_RESPONSE_SCHEMA_ID,
                path: CHAPTER_RESPONSE_SCHEMA_PATH,
                bytes: json_bytes(&typed_schema::<GetChapterResponse>(
                    CHAPTER_RESPONSE_SCHEMA_ID,
                    "StoryOS Chapter Query Response",
                )),
            },
        ]
    },
    openapi: || {
        method(
            &GET_CHAPTER,
            operation_openapi(
                &GET_CHAPTER,
                "Read the controlled Project current Chapter",
                CHAPTER_RESPONSE_SCHEMA_PATH,
                &["project_id", "chapter_id"],
            ),
        )
    },
    typescript_types: || format!("export {}", GetChapterResponse::decl(&Config::default())),
    typescript_client: || {
        format!(
            concat!(
                "\nexport async function getChapter({{ projectId, chapterId, ...options }} = {{}}) {{\n",
                "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"getChapter requires projectId\");\n",
                "  if (typeof chapterId !== \"string\" || chapterId.length === 0) throw new TypeError(\"getChapter requires chapterId\");\n",
                "  return queryJson({{ ...options, path: `{}` }});\n}}\n",
            ),
            GET_CHAPTER
                .path
                .replace("{project_id}", "${encodeURIComponent(projectId)}")
                .replace("{chapter_id}", "${encodeURIComponent(chapterId)}"),
        )
    },
    typescript_declarations: || "export declare function getChapter(options: StoryOSQueryOptions & { projectId: string; chapterId: string }): Promise<GetChapterResponse>;\n",
    fixtures: || {
        fixture_triple(
            CHAPTER_FIXTURE_PATHS,
            &GET_CHAPTER,
            [
                |_| chapter_fixture_bytes(),
                |_| invalid_chapter_fixture_bytes(),
                |_| boundary_chapter_fixture_bytes(),
            ],
        )
        .into()
    },
};

#[cfg(test)]
#[path = "release1_create_project_artifacts_tests.rs"]
mod create_project_tests;

#[cfg(test)]
#[path = "release1_list_projects_artifacts_tests.rs"]
mod list_projects_tests;

#[cfg(test)]
#[path = "release1_manuscript_tree_artifacts_tests.rs"]
mod manuscript_tree_tests;

#[cfg(test)]
#[path = "release1_manuscript_search_artifacts_tests.rs"]
mod manuscript_search_tests;

#[cfg(test)]
#[path = "release1_manuscript_statistics_artifacts_tests.rs"]
mod manuscript_statistics_tests;

#[cfg(test)]
#[path = "release1_readable_export_artifacts_tests.rs"]
mod readable_export_tests;

#[cfg(test)]
#[path = "release1_readable_export_query_artifacts_tests.rs"]
mod readable_export_query_tests;

#[cfg(test)]
#[path = "release1_project_export_artifacts_tests.rs"]
mod project_export_tests;

#[cfg(test)]
#[path = "release1_project_export_query_artifacts_tests.rs"]
mod project_export_query_tests;

#[cfg(test)]
#[path = "release1_archive_project_artifacts_tests.rs"]
mod archive_project_tests;

#[cfg(test)]
#[path = "release1_create_volume_artifacts_tests.rs"]
mod create_volume_tests;

#[cfg(test)]
#[path = "release1_update_volume_artifacts_tests.rs"]
mod update_volume_tests;

#[cfg(test)]
#[path = "release1_delete_volume_artifacts_tests.rs"]
mod delete_volume_tests;

#[cfg(test)]
#[path = "release1_update_chapter_artifacts_tests.rs"]
mod update_chapter_tests;

#[cfg(test)]
#[path = "release1_delete_chapter_artifacts_tests.rs"]
mod delete_chapter_tests;

#[cfg(test)]
#[path = "release1_create_chapter_artifacts_tests.rs"]
mod create_chapter_tests;

#[cfg(test)]
#[path = "release1_set_current_chapter_artifacts_tests.rs"]
mod set_current_chapter_tests;

#[cfg(test)]
#[path = "release1_undo_latest_author_action_artifacts_tests.rs"]
mod undo_latest_author_action_tests;

#[cfg(test)]
#[path = "release1_update_project_artifacts_tests.rs"]
mod update_project_tests;

#[cfg(test)]
#[path = "release1_project_assistance_artifacts_tests.rs"]
mod project_assistance_tests;

#[cfg(test)]
#[path = "release1_agent_run_artifacts_tests.rs"]
mod agent_run_tests;

#[cfg(test)]
#[path = "release1_agent_run_control_artifacts_tests.rs"]
mod agent_run_control_tests;

#[cfg(test)]
#[path = "release1_proposal_artifacts_tests.rs"]
mod proposal_tests;

#[cfg(test)]
#[path = "release1_accept_proposal_artifacts_tests.rs"]
mod accept_proposal_tests;

#[cfg(test)]
#[path = "release1_reject_proposal_operations_artifacts_tests.rs"]
mod reject_proposal_operations_tests;

#[cfg(test)]
#[path = "release1_reopen_rejected_operations_artifacts_tests.rs"]
mod reopen_rejected_operations_tests;

#[cfg(test)]
#[path = "release1_replan_proposal_artifacts_tests.rs"]
mod replan_proposal_tests;

#[cfg(test)]
#[path = "release1_reopen_withdrawn_proposal_artifacts_tests.rs"]
mod reopen_withdrawn_proposal_tests;

#[cfg(test)]
#[path = "release1_withdraw_proposal_artifacts_tests.rs"]
mod withdraw_proposal_tests;

#[cfg(test)]
#[path = "release1_fixture_corpus_tests.rs"]
mod fixture_corpus_tests;

#[cfg(test)]
#[path = "release1_artifacts_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "release1_refused_edit_draft_artifacts_tests.rs"]
mod refused_edit_draft_artifacts_tests;
