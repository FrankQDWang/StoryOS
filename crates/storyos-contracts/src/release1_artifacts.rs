use std::fs;
use std::io;
use std::path::Path;

use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::digest::sha256_prefixed;
use crate::release1::{
    ACTIVITY_PROFILE, API_MAJOR, ArtifactDigests, AuthoritativeChapterRevision,
    CHAPTER_REQUEST_SCHEMA_ID, CHAPTER_RESPONSE_SCHEMA_ID, COMPATIBILITY_PROFILE,
    CONTRACT_REVISION, CREATE_PROJECT_COMMAND_CHALLENGE, ControlledProject,
    CreateProjectCommandChallengeRequest, CreateProjectCommandChallengeResponse, CurrentChapter,
    DigestAlgorithm, DigestValue, ENVELOPE_PROFILE, ENVELOPE_VERSION, GENERATED_CLIENT_REVISION,
    GET_CHAPTER, GET_PROJECT, GET_PROTOCOL_PROFILE, GetChapterResponse, GetProjectResponse,
    LIMIT_PROFILE_REVISION, PROBLEM_PROFILE, PROJECT_COMMAND_CHALLENGE_REQUEST_SCHEMA_ID,
    PROJECT_COMMAND_CHALLENGE_RESPONSE_SCHEMA_ID, PROJECT_REQUEST_SCHEMA_ID,
    PROJECT_RESPONSE_SCHEMA_ID, PROTOCOL_PROFILE_REQUEST_SCHEMA_ID, PROTOCOL_PROFILE_SCHEMA_ID,
    PUBLIC_PROTOCOL_RELEASE, ProjectScope, RELEASE_IDENTITY_SCHEMA_ID, REQUIRED_CAPABILITIES,
    Release1CompatibilityIdentity, Release1ProtocolProfile, SERVER_CONTRACT_REVISION,
    WEB_CLIENT_CONTRACT_REVISION, WORKER_CONTRACT_REVISION, protocol_profile,
};

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
const TYPESCRIPT_CLIENT_PATH: &str = "generated/typescript/storyos-public-release-1/client.mjs";
const TYPESCRIPT_DECLARATION_PATH: &str =
    "generated/typescript/storyos-public-release-1/client.d.mts";
const RELEASE_PROFILE_MODULE_PATH: &str =
    "generated/typescript/storyos-public-release-1/release-profile.mjs";
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
const REVIEW_CATALOG_PATH: &str = "docs/foundation/versioned-protocol-release-1-route-catalog.json";
const REVIEW_CATALOG_SHA256: &str =
    "sha256:d3d17c028b40d23b22791579c0df9151833baa0297c68370d33f9fbf3e881dde";
const REVIEWED_CONTRACT_GRAPH_SHA256: &str =
    "sha256:b8b8a303a8332af37898fdf8868e98f72f298bb946ce01e9895ebc05f29fc0af";

type GeneratedFile = (&'static str, Vec<u8>);

/// Build the one active Release 1 protocol profile from the Rust contract source.
pub fn release1_protocol_profile() -> Release1ProtocolProfile {
    let openapi = openapi_bytes();
    let request_schema = json_bytes(&protocol_profile_request_schema());
    let response_schema = json_bytes(&protocol_profile_schema());
    let project_request_schema = json_bytes(&path_request_schema(
        PROJECT_REQUEST_SCHEMA_ID,
        &["project_id"],
    ));
    let project_response_schema = json_bytes(&typed_schema::<GetProjectResponse>(
        PROJECT_RESPONSE_SCHEMA_ID,
        "StoryOS Project Query Response",
    ));
    let chapter_request_schema = json_bytes(&path_request_schema(
        CHAPTER_REQUEST_SCHEMA_ID,
        &["project_id", "chapter_id"],
    ));
    let chapter_response_schema = json_bytes(&typed_schema::<GetChapterResponse>(
        CHAPTER_RESPONSE_SCHEMA_ID,
        "StoryOS Chapter Query Response",
    ));
    let challenge_request_schema =
        json_bytes(&typed_schema::<CreateProjectCommandChallengeRequest>(
            PROJECT_COMMAND_CHALLENGE_REQUEST_SCHEMA_ID,
            "StoryOS Project Command Challenge Request",
        ));
    let challenge_response_schema = json_bytes(&challenge_response_schema());
    let schema_catalog = schema_catalog_bytes(&[
        (
            PROTOCOL_PROFILE_REQUEST_SCHEMA_ID,
            REQUEST_SCHEMA_PATH,
            &request_schema,
        ),
        (
            PROTOCOL_PROFILE_SCHEMA_ID,
            RESPONSE_SCHEMA_PATH,
            &response_schema,
        ),
        (
            PROJECT_REQUEST_SCHEMA_ID,
            PROJECT_REQUEST_SCHEMA_PATH,
            &project_request_schema,
        ),
        (
            PROJECT_RESPONSE_SCHEMA_ID,
            PROJECT_RESPONSE_SCHEMA_PATH,
            &project_response_schema,
        ),
        (
            CHAPTER_REQUEST_SCHEMA_ID,
            CHAPTER_REQUEST_SCHEMA_PATH,
            &chapter_request_schema,
        ),
        (
            CHAPTER_RESPONSE_SCHEMA_ID,
            CHAPTER_RESPONSE_SCHEMA_PATH,
            &chapter_response_schema,
        ),
        (
            PROJECT_COMMAND_CHALLENGE_REQUEST_SCHEMA_ID,
            CHALLENGE_REQUEST_SCHEMA_PATH,
            &challenge_request_schema,
        ),
        (
            PROJECT_COMMAND_CHALLENGE_RESPONSE_SCHEMA_ID,
            CHALLENGE_RESPONSE_SCHEMA_PATH,
            &challenge_response_schema,
        ),
    ]);
    let client = typescript_client_bytes();
    let declaration = typescript_declaration_bytes();
    let contract_graph = contract_graph_bytes();
    let typescript = [
        client.as_slice(),
        b"\n--declaration--\n",
        declaration.as_slice(),
    ]
    .concat();

    let mut profile = protocol_profile(ArtifactDigests {
        contract_graph: sha256_prefixed(contract_graph),
        openapi: sha256_prefixed(openapi),
        json_schema_catalog: sha256_prefixed(schema_catalog),
        typescript: sha256_prefixed(typescript),
        fixture_corpus: FIXTURE_DIGEST_PLACEHOLDER.to_owned(),
    });
    profile.release_identity.fixture_corpus_digest =
        sha256_prefixed(fixture_corpus_bytes(&profile));
    profile
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
    let request_schema = json_bytes(&protocol_profile_request_schema());
    let response_schema = json_bytes(&protocol_profile_schema());
    let project_request_schema = json_bytes(&path_request_schema(
        PROJECT_REQUEST_SCHEMA_ID,
        &["project_id"],
    ));
    let project_response_schema = json_bytes(&typed_schema::<GetProjectResponse>(
        PROJECT_RESPONSE_SCHEMA_ID,
        "StoryOS Project Query Response",
    ));
    let chapter_request_schema = json_bytes(&path_request_schema(
        CHAPTER_REQUEST_SCHEMA_ID,
        &["project_id", "chapter_id"],
    ));
    let chapter_response_schema = json_bytes(&typed_schema::<GetChapterResponse>(
        CHAPTER_RESPONSE_SCHEMA_ID,
        "StoryOS Chapter Query Response",
    ));
    let challenge_request_schema =
        json_bytes(&typed_schema::<CreateProjectCommandChallengeRequest>(
            PROJECT_COMMAND_CHALLENGE_REQUEST_SCHEMA_ID,
            "StoryOS Project Command Challenge Request",
        ));
    let challenge_response_schema = json_bytes(&challenge_response_schema());
    let schema_catalog = schema_catalog_bytes(&[
        (
            PROTOCOL_PROFILE_REQUEST_SCHEMA_ID,
            REQUEST_SCHEMA_PATH,
            &request_schema,
        ),
        (
            PROTOCOL_PROFILE_SCHEMA_ID,
            RESPONSE_SCHEMA_PATH,
            &response_schema,
        ),
        (
            PROJECT_REQUEST_SCHEMA_ID,
            PROJECT_REQUEST_SCHEMA_PATH,
            &project_request_schema,
        ),
        (
            PROJECT_RESPONSE_SCHEMA_ID,
            PROJECT_RESPONSE_SCHEMA_PATH,
            &project_response_schema,
        ),
        (
            CHAPTER_REQUEST_SCHEMA_ID,
            CHAPTER_REQUEST_SCHEMA_PATH,
            &chapter_request_schema,
        ),
        (
            CHAPTER_RESPONSE_SCHEMA_ID,
            CHAPTER_RESPONSE_SCHEMA_PATH,
            &chapter_response_schema,
        ),
        (
            PROJECT_COMMAND_CHALLENGE_REQUEST_SCHEMA_ID,
            CHALLENGE_REQUEST_SCHEMA_PATH,
            &challenge_request_schema,
        ),
        (
            PROJECT_COMMAND_CHALLENGE_RESPONSE_SCHEMA_ID,
            CHALLENGE_RESPONSE_SCHEMA_PATH,
            &challenge_response_schema,
        ),
    ]);
    let profile = release1_protocol_profile();
    let profile_json =
        serde_json::to_string_pretty(&profile).expect("protocol profile should serialize");
    let profile_module = format!("// @generated by storyos-contracts; do not edit.\nexport const RELEASE_1_PROTOCOL_PROFILE = Object.freeze({profile_json});\n").into_bytes();
    vec![
        (OPENAPI_PATH, openapi_bytes()),
        (REQUEST_SCHEMA_PATH, request_schema),
        (RESPONSE_SCHEMA_PATH, response_schema),
        (PROJECT_REQUEST_SCHEMA_PATH, project_request_schema),
        (PROJECT_RESPONSE_SCHEMA_PATH, project_response_schema),
        (CHAPTER_REQUEST_SCHEMA_PATH, chapter_request_schema),
        (CHAPTER_RESPONSE_SCHEMA_PATH, chapter_response_schema),
        (CHALLENGE_REQUEST_SCHEMA_PATH, challenge_request_schema),
        (CHALLENGE_RESPONSE_SCHEMA_PATH, challenge_response_schema),
        (TYPESCRIPT_CLIENT_PATH, typescript_client_bytes()),
        (TYPESCRIPT_DECLARATION_PATH, typescript_declaration_bytes()),
        (RELEASE_PROFILE_MODULE_PATH, profile_module),
        (SCHEMA_CATALOG_PATH, schema_catalog),
        (FIXTURE_CATALOG_PATH, fixture_catalog_bytes(&profile)),
        (GOLDEN_PROFILE_PATH, golden_profile_bytes(&profile)),
        (INVALID_PROFILE_PATH, invalid_profile_bytes(&profile)),
        (BOUNDARY_PROFILE_PATH, boundary_profile_bytes(&profile)),
        (PROJECT_FIXTURE_PATHS[0], project_fixture_bytes()),
        (PROJECT_FIXTURE_PATHS[1], invalid_project_fixture_bytes()),
        (PROJECT_FIXTURE_PATHS[2], boundary_project_fixture_bytes()),
        (CHAPTER_FIXTURE_PATHS[0], chapter_fixture_bytes()),
        (CHAPTER_FIXTURE_PATHS[1], invalid_chapter_fixture_bytes()),
        (CHAPTER_FIXTURE_PATHS[2], boundary_chapter_fixture_bytes()),
        (CHALLENGE_FIXTURE_PATHS[0], challenge_fixture_bytes()),
        (
            CHALLENGE_FIXTURE_PATHS[1],
            invalid_challenge_fixture_bytes(),
        ),
        (
            CHALLENGE_FIXTURE_PATHS[2],
            boundary_challenge_fixture_bytes(),
        ),
    ]
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
    serde_json::to_vec(&json!({
        "schema_id": "storyos.contract-graph.release-1-profile-slice.v1",
        "contract_revision": CONTRACT_REVISION,
        "review_catalog_sha256": REVIEW_CATALOG_SHA256,
        "operations": [
            operation_graph(&GET_PROTOCOL_PROFILE, &["active_public_release_profile"]),
            operation_graph(&GET_PROJECT, &["server_derived_project_scope", "project_visibility"]),
            operation_graph(&GET_CHAPTER, &["server_derived_project_scope", "chapter_scope_join", "canonical_snapshot"]),
            challenge_operation_graph(),
        ],
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

fn challenge_operation_graph() -> Value {
    let operation = &CREATE_PROJECT_COMMAND_CHALLENGE;
    json!({
        "operation_id": operation.operation_id, "kind": "challenge", "method": operation.method,
        "path": operation.path, "request_schema": operation.request_schema,
        "response_schema": operation.response_schema,
        "preconditions": ["server_derived_project_scope", "strict_origin", "protected_client_session_binding", "route_method_schema_digest_match", "closed_command_schema"],
        "http_statuses": operation.responses.iter().map(|(status, _)| status).collect::<Vec<_>>(),
        "fixtures": operation.fixtures,
        "generated": ["openapi", "json_schema", "typescript_client", "golden_wire"]
    })
}

fn operation_graph(operation: &crate::release1::QueryOperation, preconditions: &[&str]) -> Value {
    json!({
        "operation_id": operation.operation_id, "kind": "query", "method": operation.method,
        "path": operation.path, "request_schema": operation.request_schema,
        "response_schema": operation.response_schema, "preconditions": preconditions,
        "http_statuses": operation.responses.iter().map(|(status, _)| status).collect::<Vec<_>>(),
        "fixtures": operation.fixtures,
        "generated": ["openapi", "json_schema", "typescript_client", "golden_wire"]
    })
}

fn openapi_bytes() -> Vec<u8> {
    let operations = [
        (
            &GET_PROTOCOL_PROFILE,
            "Discover the active StoryOS Release 1 protocol profile",
            RESPONSE_SCHEMA_PATH,
            &[][..],
        ),
        (
            &GET_PROJECT,
            "Read one controlled StoryOS Project",
            PROJECT_RESPONSE_SCHEMA_PATH,
            &["project_id"][..],
        ),
        (
            &GET_CHAPTER,
            "Read the controlled Project current Chapter",
            CHAPTER_RESPONSE_SCHEMA_PATH,
            &["project_id", "chapter_id"][..],
        ),
    ];
    let mut paths = operations
        .iter()
        .map(|(operation, summary, response_schema, parameters)| {
            operation_openapi(operation, summary, response_schema, parameters)
        })
        .collect::<String>();
    paths.push_str(&challenge_openapi());
    format!(
        "openapi: 3.1.0\ninfo:\n  title: StoryOS Public Release 1\n  version: {PUBLIC_PROTOCOL_RELEASE}\n  x-storyos-contract-revision: {CONTRACT_REVISION}\n  x-storyos-implemented-slice: getProtocolProfile,getProject,getChapter,createProjectCommandChallenge\npaths:\n{paths}components: {{}}\n",
    ).into_bytes()
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
        "  {}:\n    post:\n      operationId: {}\n      summary: Issue one exact Project command challenge\n      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{request_schema}'\n      responses:\n{responses}",
        operation.path, operation.operation_id,
    )
}

fn operation_openapi(
    operation: &crate::release1::QueryOperation,
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
        "  {}:\n    {}:\n      operationId: {}\n      summary: {summary}\n{parameters}      responses:\n{responses}",
        operation.path,
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
    schema
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

fn schema_catalog_bytes(schemas: &[(&str, &str, &[u8])]) -> Vec<u8> {
    json_bytes(&json!({
        "schema_id": "storyos.schema-catalog.v1", "public_protocol_release": PUBLIC_PROTOCOL_RELEASE,
        "implemented_operations": [GET_PROTOCOL_PROFILE.operation_id, GET_PROJECT.operation_id, GET_CHAPTER.operation_id, CREATE_PROJECT_COMMAND_CHALLENGE.operation_id],
        "schemas": schemas.iter().map(|(schema_id, path, bytes)| json!({
            "schema_id": schema_id, "path": path, "sha256": sha256_prefixed(bytes)
        })).collect::<Vec<_>>()
    }))
}

fn typescript_client_bytes() -> Vec<u8> {
    format!(concat!(
        "// @generated by storyos-contracts; do not edit.\n",
        "export const GENERATED_CLIENT_REVISION = \"storyos.typescript-client.release-1.v1\";\n\n",
        "export class StoryOSProtocolError extends Error {{\n",
        "  constructor(code, message, details = {{}}) {{\n    super(message);\n    this.name = \"StoryOSProtocolError\";\n",
        "    this.code = code;\n    this.status = details.status;\n    this.responseBody = details.responseBody;\n    this.retryAfterSeconds = details.retryAfterSeconds;\n  }}\n}}\n\n",
        "async function queryJson({{ baseUrl, path, fetchImpl = globalThis.fetch, signal }}) {{\n",
        "  if (typeof baseUrl !== \"string\" || baseUrl.length === 0) throw new TypeError(\"StoryOS query requires a non-empty baseUrl\");\n",
        "  if (typeof fetchImpl !== \"function\") throw new TypeError(\"StoryOS query requires a fetch implementation\");\n",
        "  const headers = {{ accept: \"application/json\" }};\n",
        "  const response = await fetchImpl(new URL(path, baseUrl), {{ method: \"GET\", headers, credentials: \"same-origin\", signal }});\n",
        "  const responseBody = await response.text();\n",
        "  if (!response.ok) throw new StoryOSProtocolError(\"query_http_error\", `StoryOS query failed with HTTP ${{response.status}}`, {{ status: response.status, responseBody }});\n",
        "  try {{ return JSON.parse(responseBody); }} catch {{\n",
        "    throw new StoryOSProtocolError(\"query_invalid_json\", \"StoryOS query returned invalid JSON\", {{ status: response.status, responseBody }});\n  }}\n}}\n\n",
        "async function commandJson({{ baseUrl, path, body, fetchImpl = globalThis.fetch, signal }}) {{\n",
        "  if (typeof baseUrl !== \"string\" || baseUrl.length === 0) throw new TypeError(\"StoryOS command requires a non-empty baseUrl\");\n",
        "  if (typeof fetchImpl !== \"function\") throw new TypeError(\"StoryOS command requires a fetch implementation\");\n",
        "  const response = await fetchImpl(new URL(path, baseUrl), {{ method: \"POST\", headers: {{ accept: \"application/json\", \"content-type\": \"application/json\" }}, credentials: \"same-origin\", body: JSON.stringify(body), signal }});\n",
        "  const responseBody = await response.text();\n",
        "  if (!response.ok) {{\n    const retryAfter = response.headers.get(\"retry-after\");\n    const retryAfterSeconds = /^(?:[1-9]|[1-5][0-9]|60)$/.test(retryAfter ?? \"\") ? Number(retryAfter) : undefined;\n    throw new StoryOSProtocolError(\"command_http_error\", `StoryOS command failed with HTTP ${{response.status}}`, {{ status: response.status, responseBody, retryAfterSeconds }});\n  }}\n",
        "  try {{ return JSON.parse(responseBody); }} catch {{ throw new StoryOSProtocolError(\"command_invalid_json\", \"StoryOS command returned invalid JSON\", {{ status: response.status, responseBody }}); }}\n}}\n\n",
        "export async function getProtocolProfile(options = {{}}) {{\n  return queryJson({{ ...options, path: \"{}\" }});\n}}\n\n",
        "export async function getProject({{ projectId, ...options }} = {{}}) {{\n",
        "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"getProject requires projectId\");\n",
        "  return queryJson({{ ...options, path: `{}` }});\n}}\n\n",
        "export async function getChapter({{ projectId, chapterId, ...options }} = {{}}) {{\n",
        "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"getChapter requires projectId\");\n",
        "  if (typeof chapterId !== \"string\" || chapterId.length === 0) throw new TypeError(\"getChapter requires chapterId\");\n",
        "  return queryJson({{ ...options, path: `{}` }});\n}}\n",
        "\nexport async function createProjectCommandChallenge({{ projectId, request, ...options }} = {{}}) {{\n",
        "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"createProjectCommandChallenge requires projectId\");\n",
        "  if (!request || typeof request !== \"object\") throw new TypeError(\"createProjectCommandChallenge requires request\");\n",
        "  return commandJson({{ ...options, path: `{}`, body: request }});\n}}\n",
    ),
        GET_PROTOCOL_PROFILE.path,
        GET_PROJECT.path.replace("{project_id}", "${encodeURIComponent(projectId)}"),
        GET_CHAPTER.path
            .replace("{project_id}", "${encodeURIComponent(projectId)}")
            .replace("{chapter_id}", "${encodeURIComponent(chapterId)}"),
        CREATE_PROJECT_COMMAND_CHALLENGE.path
            .replace("{project_id}", "${encodeURIComponent(projectId)}"),
    ).into_bytes()
}

fn typescript_declaration_bytes() -> Vec<u8> {
    let config = Config::default();
    let identity = Release1CompatibilityIdentity::decl(&config);
    let profile = Release1ProtocolProfile::decl(&config);
    let project_scope = ProjectScope::decl(&config);
    let controlled_project = ControlledProject::decl(&config);
    let chapter_revision = AuthoritativeChapterRevision::decl(&config);
    let current_chapter = CurrentChapter::decl(&config);
    let project = GetProjectResponse::decl(&config);
    let chapter = GetChapterResponse::decl(&config);
    let challenge_request = CreateProjectCommandChallengeRequest::decl(&config);
    let challenge_response = CreateProjectCommandChallengeResponse::decl(&config);
    let digest_value = DigestValue::decl(&config);
    let digest_algorithm = DigestAlgorithm::decl(&config);
    format!(
        "// @generated by storyos-contracts; do not edit.\nexport {identity}\n\nexport {profile}\n\nexport {project_scope}\n\nexport {controlled_project}\n\nexport {chapter_revision}\n\nexport {current_chapter}\n\nexport {project}\n\nexport {chapter}\n\nexport {digest_algorithm}\n\nexport {digest_value}\n\nexport {challenge_request}\n\nexport {challenge_response}\n\n{}",
        concat!(
            "export declare const GENERATED_CLIENT_REVISION: string;\n",
            "export declare class StoryOSProtocolError extends Error {\n  readonly code: string;\n  readonly status?: number;\n  readonly responseBody?: string;\n  readonly retryAfterSeconds?: number;\n}\n",
            "export interface StoryOSQueryOptions {\n  baseUrl: string;\n  fetchImpl?: typeof fetch;\n  signal?: AbortSignal;\n}\n",
            "export declare function getProtocolProfile(options: StoryOSQueryOptions): Promise<Release1ProtocolProfile>;\n",
            "export declare function getProject(options: StoryOSQueryOptions & { projectId: string }): Promise<GetProjectResponse>;\n",
            "export declare function getChapter(options: StoryOSQueryOptions & { projectId: string; chapterId: string }): Promise<GetChapterResponse>;\n",
            "export declare function createProjectCommandChallenge(options: StoryOSQueryOptions & { projectId: string; request: CreateProjectCommandChallengeRequest }): Promise<CreateProjectCommandChallengeResponse>;\n",
        )
    )
    .into_bytes()
}

fn fixture_catalog_bytes(profile: &Release1ProtocolProfile) -> Vec<u8> {
    json_bytes(&json!({
        "schema_id": "storyos.fixture-catalog.release-1.v1", "public_protocol_release": PUBLIC_PROTOCOL_RELEASE,
        "corpus_digest": profile.release_identity.fixture_corpus_digest,
        "digest_scope": {
            "paths": [GOLDEN_PROFILE_PATH, INVALID_PROFILE_PATH, BOUNDARY_PROFILE_PATH,
                PROJECT_FIXTURE_PATHS[0], PROJECT_FIXTURE_PATHS[1], PROJECT_FIXTURE_PATHS[2],
                CHAPTER_FIXTURE_PATHS[0], CHAPTER_FIXTURE_PATHS[1], CHAPTER_FIXTURE_PATHS[2],
                CHALLENGE_FIXTURE_PATHS[0], CHALLENGE_FIXTURE_PATHS[1], CHALLENGE_FIXTURE_PATHS[2]],
            "normalization": format!("replace every release_identity.fixture_corpus_digest with {FIXTURE_DIGEST_PLACEHOLDER}")
        },
        "fixtures": [
            {"fixture_id": crate::release1::POSITIVE_FIXTURE_ID, "classification": "positive",
             "operation_id": GET_PROTOCOL_PROFILE.operation_id, "path": GOLDEN_PROFILE_PATH},
            {"fixture_id": crate::release1::INVALID_FIXTURE_ID, "classification": "invalid",
             "operation_id": GET_PROTOCOL_PROFILE.operation_id, "path": INVALID_PROFILE_PATH},
            {"fixture_id": crate::release1::BOUNDARY_FIXTURE_ID, "classification": "boundary",
             "operation_id": GET_PROTOCOL_PROFILE.operation_id, "path": BOUNDARY_PROFILE_PATH},
            {"fixture_id": GET_PROJECT.fixtures[0], "classification": "positive", "operation_id": GET_PROJECT.operation_id, "path": PROJECT_FIXTURE_PATHS[0]},
            {"fixture_id": GET_PROJECT.fixtures[1], "classification": "invalid", "operation_id": GET_PROJECT.operation_id, "path": PROJECT_FIXTURE_PATHS[1]},
            {"fixture_id": GET_PROJECT.fixtures[2], "classification": "boundary", "operation_id": GET_PROJECT.operation_id, "path": PROJECT_FIXTURE_PATHS[2]},
            {"fixture_id": GET_CHAPTER.fixtures[0], "classification": "positive", "operation_id": GET_CHAPTER.operation_id, "path": CHAPTER_FIXTURE_PATHS[0]},
            {"fixture_id": GET_CHAPTER.fixtures[1], "classification": "invalid", "operation_id": GET_CHAPTER.operation_id, "path": CHAPTER_FIXTURE_PATHS[1]},
            {"fixture_id": GET_CHAPTER.fixtures[2], "classification": "boundary", "operation_id": GET_CHAPTER.operation_id, "path": CHAPTER_FIXTURE_PATHS[2]},
            {"fixture_id": CREATE_PROJECT_COMMAND_CHALLENGE.fixtures[0], "classification": "positive", "operation_id": CREATE_PROJECT_COMMAND_CHALLENGE.operation_id, "path": CHALLENGE_FIXTURE_PATHS[0]},
            {"fixture_id": CREATE_PROJECT_COMMAND_CHALLENGE.fixtures[1], "classification": "invalid", "operation_id": CREATE_PROJECT_COMMAND_CHALLENGE.operation_id, "path": CHALLENGE_FIXTURE_PATHS[1]},
            {"fixture_id": CREATE_PROJECT_COMMAND_CHALLENGE.fixtures[2], "classification": "boundary", "operation_id": CREATE_PROJECT_COMMAND_CHALLENGE.operation_id, "path": CHALLENGE_FIXTURE_PATHS[2]}
        ]
    }))
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

fn fixture_corpus_bytes(profile: &Release1ProtocolProfile) -> Vec<u8> {
    [
        golden_profile_bytes(profile),
        invalid_profile_bytes(profile),
        boundary_profile_bytes(profile),
        project_fixture_bytes(),
        invalid_project_fixture_bytes(),
        boundary_project_fixture_bytes(),
        chapter_fixture_bytes(),
        invalid_chapter_fixture_bytes(),
        boundary_chapter_fixture_bytes(),
        challenge_fixture_bytes(),
        invalid_challenge_fixture_bytes(),
        boundary_challenge_fixture_bytes(),
    ]
    .concat()
}

fn project_fixture() -> Value {
    json!({
        "schema_id": PROJECT_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000005",
        "project_scope": {"owner_user_id": "018f0000-0000-7001-8000-000000000001", "project_id": "018f0000-0000-7001-8000-000000000002"},
        "project": {"project_id": "018f0000-0000-7001-8000-000000000002", "title": "受控项目", "current_chapter_id": "018f0000-0000-7001-8000-000000000003"}
    })
}

fn chapter_fixture() -> Value {
    json!({
        "schema_id": CHAPTER_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000006",
        "project_scope": {"owner_user_id": "018f0000-0000-7001-8000-000000000001", "project_id": "018f0000-0000-7001-8000-000000000002"},
        "chapter": {"chapter_id": "018f0000-0000-7001-8000-000000000003", "title": "第一章", "current_revision": {"revision_id": "018f0000-0000-7001-8000-000000000004", "body": "雨落在窗沿。"}}
    })
}

fn challenge_fixture() -> Value {
    json!({
        "nonce": "4f9f5ad05c4d1294d4114fb15595d831b64e3f4312a17e639213ad36e941ca71",
        "expires_at": "2026-08-12T08:05:00.000Z",
        "limit_profile_revision": LIMIT_PROFILE_REVISION
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

fn json_bytes(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("contract JSON should serialize");
    bytes.push(b'\n');
    bytes
}

#[cfg(test)]
#[path = "release1_artifacts_tests.rs"]
mod tests;
