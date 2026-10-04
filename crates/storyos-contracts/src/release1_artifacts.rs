use std::fs;
use std::io;
use std::path::Path;
use std::sync::LazyLock;

use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::digest::sha256_prefixed;
use crate::release1::{
    ACTIVITY_PROFILE, API_MAJOR, ArtifactDigests, AuthoritativeChapterRevision,
    COMPATIBILITY_PROFILE, CONTRACT_REVISION, ControlledProject, CurrentChapter, DigestAlgorithm,
    DigestValue, ENVELOPE_PROFILE, ENVELOPE_VERSION, GENERATED_CLIENT_REVISION,
    LIMIT_PROFILE_REVISION, ManuscriptBlock, ManuscriptBlockKind, PROBLEM_PROFILE,
    PUBLIC_PROTOCOL_RELEASE, ProjectOpenState, ProjectScope, QueryOperation,
    RELEASE_IDENTITY_SCHEMA_ID, REQUIRED_CAPABILITIES, Release1ProtocolProfile,
    SERVER_CONTRACT_REVISION, WEB_CLIENT_CONTRACT_REVISION, WORKER_CONTRACT_REVISION,
    protocol_profile,
};
use crate::release1_operation_registry::{
    ContractGraphEntry, GeneratedSchema, OpenApiMethod, OperationKind, RELEASE1_OPERATIONS,
    RegisteredOperation,
};
use crate::release1_wire::{json_bytes, schema_value};
const FIXTURE_DIGEST_PLACEHOLDER: &str = "sha256:self-normalized";
const OPENAPI_PATH: &str = "generated/openapi/storyos-public-release-1.yaml";
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
            json_bytes(&schema_value::<crate::WebAssetManifest>(
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

#[cfg(test)]
#[path = "release1_create_project_artifacts_tests.rs"]
mod create_project_tests;

#[cfg(test)]
#[path = "release1_agent_run_artifacts_tests.rs"]
mod agent_run_tests;

#[cfg(test)]
#[path = "release1_fixture_corpus_tests.rs"]
mod fixture_corpus_tests;

#[cfg(test)]
#[path = "release1_artifacts_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "release1_refused_edit_draft_artifacts_tests.rs"]
mod refused_edit_draft_artifacts_tests;
