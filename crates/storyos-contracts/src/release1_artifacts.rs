use std::fs;
use std::io;
use std::path::Path;

use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::digest::sha256_prefixed;
use crate::release1::{
    ACTIVITY_PROFILE, API_MAJOR, ArtifactDigests, COMPATIBILITY_PROFILE, CONTRACT_REVISION,
    ENVELOPE_PROFILE, ENVELOPE_VERSION, GENERATED_CLIENT_REVISION, GET_PROTOCOL_PROFILE,
    LIMIT_PROFILE_REVISION, PROBLEM_PROFILE, PROTOCOL_PROFILE_REQUEST_SCHEMA_ID,
    PROTOCOL_PROFILE_SCHEMA_ID, PUBLIC_PROTOCOL_RELEASE, RELEASE_IDENTITY_SCHEMA_ID,
    REQUIRED_CAPABILITIES, Release1CompatibilityIdentity, Release1ProtocolProfile,
    SERVER_CONTRACT_REVISION, WEB_CLIENT_CONTRACT_REVISION, WORKER_CONTRACT_REVISION,
    protocol_profile,
};

const FIXTURE_DIGEST_PLACEHOLDER: &str = "sha256:self-normalized";
const OPENAPI_PATH: &str = "generated/openapi/storyos-public-release-1.yaml";
const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/protocol-profile-request.schema.json";
const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/protocol-profile-response.schema.json";
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
const REVIEW_CATALOG_PATH: &str = "docs/foundation/versioned-protocol-release-1-route-catalog.json";
const REVIEW_CATALOG_SHA256: &str =
    "sha256:d3d17c028b40d23b22791579c0df9151833baa0297c68370d33f9fbf3e881dde";
const REVIEWED_CONTRACT_GRAPH_SHA256: &str =
    "sha256:0d9a8f181cf956e23d00842dfd88b8bd96eba9b65fa0557745b5ed0965095e85";

type GeneratedFile = (&'static str, Vec<u8>);

/// Build the one active Release 1 protocol profile from the Rust contract source.
pub fn release1_protocol_profile() -> Release1ProtocolProfile {
    let openapi = openapi_bytes();
    let request_schema = json_bytes(&protocol_profile_request_schema());
    let response_schema = json_bytes(&protocol_profile_schema());
    let schema_catalog = schema_catalog_bytes(&request_schema, &response_schema);
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
    let schema_catalog = schema_catalog_bytes(&request_schema, &response_schema);
    let profile = release1_protocol_profile();
    let profile_json =
        serde_json::to_string_pretty(&profile).expect("protocol profile should serialize");
    let profile_module = format!("// @generated by storyos-contracts; do not edit.\nexport const RELEASE_1_PROTOCOL_PROFILE = Object.freeze({profile_json});\n").into_bytes();
    vec![
        (OPENAPI_PATH, openapi_bytes()),
        (REQUEST_SCHEMA_PATH, request_schema),
        (RESPONSE_SCHEMA_PATH, response_schema),
        (TYPESCRIPT_CLIENT_PATH, typescript_client_bytes()),
        (TYPESCRIPT_DECLARATION_PATH, typescript_declaration_bytes()),
        (RELEASE_PROFILE_MODULE_PATH, profile_module),
        (SCHEMA_CATALOG_PATH, schema_catalog),
        (FIXTURE_CATALOG_PATH, fixture_catalog_bytes(&profile)),
        (GOLDEN_PROFILE_PATH, golden_profile_bytes(&profile)),
        (INVALID_PROFILE_PATH, invalid_profile_bytes(&profile)),
        (BOUNDARY_PROFILE_PATH, boundary_profile_bytes(&profile)),
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
        "operation": {
            "operation_id": GET_PROTOCOL_PROFILE.operation_id, "kind": "query", "method": GET_PROTOCOL_PROFILE.method,
            "path": GET_PROTOCOL_PROFILE.path, "request_schema": GET_PROTOCOL_PROFILE.request_schema,
            "response_schema": GET_PROTOCOL_PROFILE.response_schema, "preconditions": ["active_public_release_profile"],
            "http_statuses": GET_PROTOCOL_PROFILE.responses.iter().map(|(status, _)| status).collect::<Vec<_>>(),
            "fixtures": GET_PROTOCOL_PROFILE.fixtures, "generated": ["openapi", "json_schema", "typescript_client", "golden_wire"]
        },
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

fn openapi_bytes() -> Vec<u8> {
    let operation = &GET_PROTOCOL_PROFILE;
    let responses = operation.responses.iter().map(|(status, description)| format!(
        "        '{status}':\n          description: {description}\n{}",
        if *status == 200 { "          content:\n            application/json:\n              schema:\n                $ref: '../json-schema/storyos-public-release-1/protocol-profile-response.schema.json'\n" } else { "" }
    )).collect::<String>();
    format!(
        "openapi: 3.1.0\ninfo:\n  title: StoryOS Public Release 1\n  version: {PUBLIC_PROTOCOL_RELEASE}\n  x-storyos-contract-revision: {CONTRACT_REVISION}\n  x-storyos-implemented-slice: {}\npaths:\n  {}:\n    {}:\n      operationId: {}\n      summary: Discover the active StoryOS Release 1 protocol profile\n      responses:\n{responses}components: {{}}\n",
        operation.operation_id, operation.path, operation.method.to_ascii_lowercase(),
        operation.operation_id,
    ).into_bytes()
}

fn protocol_profile_request_schema() -> Value {
    json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "$id": PROTOCOL_PROFILE_REQUEST_SCHEMA_ID,
           "title": "StoryOS Release 1 Protocol Profile Request", "type": "object",
           "additionalProperties": false, "maxProperties": 0})
}

fn protocol_profile_schema() -> Value {
    let mut schema = serde_json::to_value(schema_for!(Release1ProtocolProfile))
        .expect("the Release 1 profile schema should serialize");
    schema["$id"] = Value::String(PROTOCOL_PROFILE_SCHEMA_ID.to_owned());
    schema["title"] = Value::String("StoryOS Release 1 Protocol Profile".to_owned());
    schema
}

fn schema_catalog_bytes(request_schema: &[u8], response_schema: &[u8]) -> Vec<u8> {
    json_bytes(&json!({
        "schema_id": "storyos.schema-catalog.v1", "public_protocol_release": PUBLIC_PROTOCOL_RELEASE,
        "implemented_operations": [GET_PROTOCOL_PROFILE.operation_id],
        "schemas": [
            {"schema_id": PROTOCOL_PROFILE_REQUEST_SCHEMA_ID, "path": REQUEST_SCHEMA_PATH,
             "sha256": sha256_prefixed(request_schema)},
            {"schema_id": PROTOCOL_PROFILE_SCHEMA_ID, "path": RESPONSE_SCHEMA_PATH,
             "sha256": sha256_prefixed(response_schema)}
        ]
    }))
}

fn typescript_client_bytes() -> Vec<u8> {
    [concat!(
        "// @generated by storyos-contracts; do not edit.\n",
        "export const GENERATED_CLIENT_REVISION = \"storyos.typescript-client.release-1.v1\";\n\n",
        "export class StoryOSProtocolError extends Error {\n",
        "  constructor(code, message, details = {}) {\n    super(message);\n    this.name = \"StoryOSProtocolError\";\n",
        "    this.code = code;\n    this.status = details.status;\n    this.responseBody = details.responseBody;\n  }\n}\n\n",
        "export async function getProtocolProfile({ baseUrl, fetchImpl = globalThis.fetch, signal } = {}) {\n",
        "  if (typeof baseUrl !== \"string\" || baseUrl.length === 0) {\n    throw new TypeError(\"getProtocolProfile requires a non-empty baseUrl\");\n  }\n",
        "  if (typeof fetchImpl !== \"function\") {\n    throw new TypeError(\"getProtocolProfile requires a fetch implementation\");\n  }\n\n",
        "  const endpoint = new URL(\"",
    ), GET_PROTOCOL_PROFILE.path, concat!(
        "\", baseUrl);\n",
        "  const response = await fetchImpl(endpoint, {\n    method: \"",
    ), GET_PROTOCOL_PROFILE.method, concat!(
        "\",\n    headers: { accept: \"application/json\" },\n    signal,\n  });\n",
        "  const responseBody = await response.text();\n  if (!response.ok) {\n",
        "    throw new StoryOSProtocolError(\n      \"protocol_http_error\",\n      `Protocol discovery failed with HTTP ${response.status}`,\n      { status: response.status, responseBody },\n    );\n  }\n",
        "  try {\n    return JSON.parse(responseBody);\n  } catch {\n",
        "    throw new StoryOSProtocolError(\n      \"protocol_invalid_json\",\n      \"Protocol discovery returned invalid JSON\",\n      { status: response.status, responseBody },\n    );\n  }\n}\n",
    )].concat().into_bytes()
}

fn typescript_declaration_bytes() -> Vec<u8> {
    let config = Config::default();
    let identity = Release1CompatibilityIdentity::decl(&config);
    let profile = Release1ProtocolProfile::decl(&config);
    format!(
        "// @generated by storyos-contracts; do not edit.\nexport {identity}\n\nexport {profile}\n\n{}",
        concat!(
            "export declare const GENERATED_CLIENT_REVISION: string;\n",
            "export declare class StoryOSProtocolError extends Error {\n  readonly code: string;\n  readonly status?: number;\n  readonly responseBody?: string;\n}\n",
            "export declare function getProtocolProfile(options: {\n  baseUrl: string;\n  fetchImpl?: typeof fetch;\n  signal?: AbortSignal;\n}): Promise<Release1ProtocolProfile>;\n",
        )
    )
    .into_bytes()
}

fn fixture_catalog_bytes(profile: &Release1ProtocolProfile) -> Vec<u8> {
    json_bytes(&json!({
        "schema_id": "storyos.fixture-catalog.release-1.v1", "public_protocol_release": PUBLIC_PROTOCOL_RELEASE,
        "corpus_digest": profile.release_identity.fixture_corpus_digest,
        "digest_scope": {
            "paths": [GOLDEN_PROFILE_PATH, INVALID_PROFILE_PATH, BOUNDARY_PROFILE_PATH],
            "normalization": format!("replace every release_identity.fixture_corpus_digest with {FIXTURE_DIGEST_PLACEHOLDER}")
        },
        "fixtures": [
            {"fixture_id": crate::release1::POSITIVE_FIXTURE_ID, "classification": "positive",
             "operation_id": GET_PROTOCOL_PROFILE.operation_id, "path": GOLDEN_PROFILE_PATH},
            {"fixture_id": crate::release1::INVALID_FIXTURE_ID, "classification": "invalid",
             "operation_id": GET_PROTOCOL_PROFILE.operation_id, "path": INVALID_PROFILE_PATH},
            {"fixture_id": crate::release1::BOUNDARY_FIXTURE_ID, "classification": "boundary",
             "operation_id": GET_PROTOCOL_PROFILE.operation_id, "path": BOUNDARY_PROFILE_PATH}
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
    ]
    .concat()
}

fn json_bytes(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("contract JSON should serialize");
    bytes.push(b'\n');
    bytes
}

#[cfg(test)]
#[path = "release1_artifacts_tests.rs"]
mod tests;
