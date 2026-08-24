use std::fs;
use std::io;
use std::path::Path;

use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::digest::sha256_prefixed;
use crate::release1::{
    ACTIVITY_PROFILE, API_MAJOR, APPLY_AUTHOR_EDIT_REQUEST_SCHEMA_ID,
    APPLY_AUTHOR_EDIT_RESPONSE_SCHEMA_ID, ArtifactDigests, AuthoritativeChapterRevision,
    CHAPTER_REQUEST_SCHEMA_ID, CHAPTER_RESPONSE_SCHEMA_ID, COMPATIBILITY_PROFILE,
    CONTRACT_REVISION, CREATE_EDITOR_SESSION, CREATE_EDITOR_SESSION_REQUEST_SCHEMA_ID,
    CREATE_EDITOR_SESSION_RESPONSE_SCHEMA_ID, CREATE_PROJECT_COMMAND_CHALLENGE, ControlledProject,
    CreateEditorSessionRequest, CreateEditorSessionResponse, CreateProjectCommandChallengeRequest,
    CreateProjectCommandChallengeResponse, CurrentChapter, DigestAlgorithm, DigestValue,
    ENVELOPE_PROFILE, ENVELOPE_VERSION, EditorBaseSnapshot, EditorReadOnlyReason,
    EditorSessionBinding, EditorWriterProjection, GENERATED_CLIENT_REVISION, GET_CHAPTER,
    GET_EDITOR_SESSION, GET_EDITOR_SESSION_REQUEST_SCHEMA_ID,
    GET_EDITOR_SESSION_RESPONSE_SCHEMA_ID, GET_PROJECT, GET_PROTOCOL_PROFILE, GetChapterResponse,
    GetEditorSessionResponse, GetProjectResponse, LIMIT_PROFILE_REVISION, PROBLEM_PROFILE,
    PROJECT_COMMAND_CHALLENGE_REQUEST_SCHEMA_ID, PROJECT_COMMAND_CHALLENGE_RESPONSE_SCHEMA_ID,
    PROJECT_REQUEST_SCHEMA_ID, PROJECT_RESPONSE_SCHEMA_ID, PROTOCOL_PROFILE_REQUEST_SCHEMA_ID,
    PROTOCOL_PROFILE_SCHEMA_ID, PUBLIC_PROTOCOL_RELEASE, ProjectScope, RELEASE_IDENTITY_SCHEMA_ID,
    REQUIRED_CAPABILITIES, Release1CompatibilityIdentity, Release1ProtocolProfile,
    SERVER_CONTRACT_REVISION, WEB_CLIENT_CONTRACT_REVISION, WORKER_CONTRACT_REVISION,
    protocol_profile,
};
use crate::release1_author_edit::APPLY_AUTHOR_EDIT;
use crate::release1_author_edit_artifacts as author_edit_artifacts;
use crate::release1_author_edit_outcome::GET_APPLY_AUTHOR_EDIT_OUTCOME;
use crate::release1_author_edit_outcome_artifacts as author_edit_outcome_artifacts;
use crate::release1_create_project::CREATE_PROJECT_CHALLENGE;
use crate::release1_create_project_artifacts as create_project_artifacts;
use crate::release1_snapshot::{ACTIVITY_STREAM, GET_SNAPSHOT};
use crate::release1_snapshot_artifacts as snapshot_artifacts;
use crate::release1_takeover::TAKE_OVER_PROJECT_WRITER;
use crate::release1_takeover_artifacts as takeover_artifacts;

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
    "sha256:ebd74322ed08d6f049f899ea50af4d307ae75507311985b423c201489daf395e";
const REVIEWED_CONTRACT_GRAPH_SHA256: &str =
    "sha256:9804868bfbed79199040780242dda1f2428874308b2a7806b2b55fba4e055af0";

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
    let create_project_challenge_request_schema =
        create_project_artifacts::challenge_request_schema_bytes();
    let create_project_challenge_response_schema =
        create_project_artifacts::challenge_response_schema_bytes();
    let editor_create_request_schema = json_bytes(&typed_schema::<CreateEditorSessionRequest>(
        CREATE_EDITOR_SESSION_REQUEST_SCHEMA_ID,
        "StoryOS Create Editor Session Request",
    ));
    let editor_create_response_schema = json_bytes(&typed_schema::<CreateEditorSessionResponse>(
        CREATE_EDITOR_SESSION_RESPONSE_SCHEMA_ID,
        "StoryOS Create Editor Session Response",
    ));
    let editor_get_request_schema = json_bytes(&path_request_schema(
        GET_EDITOR_SESSION_REQUEST_SCHEMA_ID,
        &["project_id", "editor_session_id"],
    ));
    let editor_get_response_schema = json_bytes(&typed_schema::<GetEditorSessionResponse>(
        GET_EDITOR_SESSION_RESPONSE_SCHEMA_ID,
        "StoryOS Get Editor Session Response",
    ));
    let apply_edit_request_schema = author_edit_artifacts::request_schema_bytes();
    let apply_edit_response_schema = author_edit_artifacts::response_schema_bytes();
    let apply_edit_outcome_request_schema = author_edit_outcome_artifacts::request_schema_bytes();
    let apply_edit_outcome_response_schema = author_edit_outcome_artifacts::response_schema_bytes();
    let snapshot_request_schema = snapshot_artifacts::snapshot_request_schema_bytes();
    let snapshot_response_schema = snapshot_artifacts::snapshot_response_schema_bytes();
    let activity_stream_request_schema = snapshot_artifacts::activity_stream_request_schema_bytes();
    let activity_stream_response_schema =
        snapshot_artifacts::activity_stream_response_schema_bytes();
    let takeover_request_schema = takeover_artifacts::request_schema_bytes();
    let takeover_response_schema = takeover_artifacts::response_schema_bytes();
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
        (
            crate::CREATE_PROJECT_CHALLENGE_REQUEST_SCHEMA_ID,
            create_project_artifacts::CHALLENGE_REQUEST_SCHEMA_PATH,
            &create_project_challenge_request_schema,
        ),
        (
            crate::CREATE_PROJECT_CHALLENGE_RESPONSE_SCHEMA_ID,
            create_project_artifacts::CHALLENGE_RESPONSE_SCHEMA_PATH,
            &create_project_challenge_response_schema,
        ),
        (
            CREATE_EDITOR_SESSION_REQUEST_SCHEMA_ID,
            EDITOR_SESSION_CREATE_REQUEST_SCHEMA_PATH,
            &editor_create_request_schema,
        ),
        (
            CREATE_EDITOR_SESSION_RESPONSE_SCHEMA_ID,
            EDITOR_SESSION_CREATE_RESPONSE_SCHEMA_PATH,
            &editor_create_response_schema,
        ),
        (
            GET_EDITOR_SESSION_REQUEST_SCHEMA_ID,
            EDITOR_SESSION_GET_REQUEST_SCHEMA_PATH,
            &editor_get_request_schema,
        ),
        (
            GET_EDITOR_SESSION_RESPONSE_SCHEMA_ID,
            EDITOR_SESSION_GET_RESPONSE_SCHEMA_PATH,
            &editor_get_response_schema,
        ),
        (
            APPLY_AUTHOR_EDIT_REQUEST_SCHEMA_ID,
            author_edit_artifacts::REQUEST_SCHEMA_PATH,
            &apply_edit_request_schema,
        ),
        (
            APPLY_AUTHOR_EDIT_RESPONSE_SCHEMA_ID,
            author_edit_artifacts::RESPONSE_SCHEMA_PATH,
            &apply_edit_response_schema,
        ),
        (
            crate::GET_APPLY_AUTHOR_EDIT_OUTCOME_REQUEST_SCHEMA_ID,
            author_edit_outcome_artifacts::REQUEST_SCHEMA_PATH,
            &apply_edit_outcome_request_schema,
        ),
        (
            crate::GET_APPLY_AUTHOR_EDIT_OUTCOME_RESPONSE_SCHEMA_ID,
            author_edit_outcome_artifacts::RESPONSE_SCHEMA_PATH,
            &apply_edit_outcome_response_schema,
        ),
        (
            crate::GET_SNAPSHOT_REQUEST_SCHEMA_ID,
            snapshot_artifacts::SNAPSHOT_REQUEST_SCHEMA_PATH,
            &snapshot_request_schema,
        ),
        (
            crate::GET_SNAPSHOT_RESPONSE_SCHEMA_ID,
            snapshot_artifacts::SNAPSHOT_RESPONSE_SCHEMA_PATH,
            &snapshot_response_schema,
        ),
        (
            crate::release1_snapshot::ACTIVITY_STREAM_REQUEST_SCHEMA_ID,
            snapshot_artifacts::ACTIVITY_STREAM_REQUEST_SCHEMA_PATH,
            &activity_stream_request_schema,
        ),
        (
            crate::release1_snapshot::ACTIVITY_STREAM_RESPONSE_SCHEMA_ID,
            snapshot_artifacts::ACTIVITY_STREAM_RESPONSE_SCHEMA_PATH,
            &activity_stream_response_schema,
        ),
        (
            crate::TAKE_OVER_PROJECT_WRITER_REQUEST_SCHEMA_ID,
            takeover_artifacts::REQUEST_SCHEMA_PATH,
            &takeover_request_schema,
        ),
        (
            crate::TAKE_OVER_PROJECT_WRITER_RESPONSE_SCHEMA_ID,
            takeover_artifacts::RESPONSE_SCHEMA_PATH,
            &takeover_response_schema,
        ),
    ]);
    let client = typescript_client_bytes();
    let declaration = typescript_declaration_bytes();
    let profile_declaration = release_profile_declaration_bytes();
    let contract_graph = contract_graph_bytes();
    let typescript = [
        client.as_slice(),
        b"\n--declaration--\n",
        declaration.as_slice(),
        b"\n--release-profile-declaration--\n",
        profile_declaration.as_slice(),
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
    let create_project_challenge_request_schema =
        create_project_artifacts::challenge_request_schema_bytes();
    let create_project_challenge_response_schema =
        create_project_artifacts::challenge_response_schema_bytes();
    let editor_create_request_schema = json_bytes(&typed_schema::<CreateEditorSessionRequest>(
        CREATE_EDITOR_SESSION_REQUEST_SCHEMA_ID,
        "StoryOS Create Editor Session Request",
    ));
    let editor_create_response_schema = json_bytes(&typed_schema::<CreateEditorSessionResponse>(
        CREATE_EDITOR_SESSION_RESPONSE_SCHEMA_ID,
        "StoryOS Create Editor Session Response",
    ));
    let editor_get_request_schema = json_bytes(&path_request_schema(
        GET_EDITOR_SESSION_REQUEST_SCHEMA_ID,
        &["project_id", "editor_session_id"],
    ));
    let editor_get_response_schema = json_bytes(&typed_schema::<GetEditorSessionResponse>(
        GET_EDITOR_SESSION_RESPONSE_SCHEMA_ID,
        "StoryOS Get Editor Session Response",
    ));
    let apply_edit_request_schema = author_edit_artifacts::request_schema_bytes();
    let apply_edit_response_schema = author_edit_artifacts::response_schema_bytes();
    let apply_edit_outcome_request_schema = author_edit_outcome_artifacts::request_schema_bytes();
    let apply_edit_outcome_response_schema = author_edit_outcome_artifacts::response_schema_bytes();
    let snapshot_request_schema = snapshot_artifacts::snapshot_request_schema_bytes();
    let snapshot_response_schema = snapshot_artifacts::snapshot_response_schema_bytes();
    let activity_stream_request_schema = snapshot_artifacts::activity_stream_request_schema_bytes();
    let activity_stream_response_schema =
        snapshot_artifacts::activity_stream_response_schema_bytes();
    let takeover_request_schema = takeover_artifacts::request_schema_bytes();
    let takeover_response_schema = takeover_artifacts::response_schema_bytes();
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
        (
            crate::CREATE_PROJECT_CHALLENGE_REQUEST_SCHEMA_ID,
            create_project_artifacts::CHALLENGE_REQUEST_SCHEMA_PATH,
            &create_project_challenge_request_schema,
        ),
        (
            crate::CREATE_PROJECT_CHALLENGE_RESPONSE_SCHEMA_ID,
            create_project_artifacts::CHALLENGE_RESPONSE_SCHEMA_PATH,
            &create_project_challenge_response_schema,
        ),
        (
            CREATE_EDITOR_SESSION_REQUEST_SCHEMA_ID,
            EDITOR_SESSION_CREATE_REQUEST_SCHEMA_PATH,
            &editor_create_request_schema,
        ),
        (
            CREATE_EDITOR_SESSION_RESPONSE_SCHEMA_ID,
            EDITOR_SESSION_CREATE_RESPONSE_SCHEMA_PATH,
            &editor_create_response_schema,
        ),
        (
            GET_EDITOR_SESSION_REQUEST_SCHEMA_ID,
            EDITOR_SESSION_GET_REQUEST_SCHEMA_PATH,
            &editor_get_request_schema,
        ),
        (
            GET_EDITOR_SESSION_RESPONSE_SCHEMA_ID,
            EDITOR_SESSION_GET_RESPONSE_SCHEMA_PATH,
            &editor_get_response_schema,
        ),
        (
            APPLY_AUTHOR_EDIT_REQUEST_SCHEMA_ID,
            author_edit_artifacts::REQUEST_SCHEMA_PATH,
            &apply_edit_request_schema,
        ),
        (
            APPLY_AUTHOR_EDIT_RESPONSE_SCHEMA_ID,
            author_edit_artifacts::RESPONSE_SCHEMA_PATH,
            &apply_edit_response_schema,
        ),
        (
            crate::GET_APPLY_AUTHOR_EDIT_OUTCOME_REQUEST_SCHEMA_ID,
            author_edit_outcome_artifacts::REQUEST_SCHEMA_PATH,
            &apply_edit_outcome_request_schema,
        ),
        (
            crate::GET_APPLY_AUTHOR_EDIT_OUTCOME_RESPONSE_SCHEMA_ID,
            author_edit_outcome_artifacts::RESPONSE_SCHEMA_PATH,
            &apply_edit_outcome_response_schema,
        ),
        (
            crate::GET_SNAPSHOT_REQUEST_SCHEMA_ID,
            snapshot_artifacts::SNAPSHOT_REQUEST_SCHEMA_PATH,
            &snapshot_request_schema,
        ),
        (
            crate::GET_SNAPSHOT_RESPONSE_SCHEMA_ID,
            snapshot_artifacts::SNAPSHOT_RESPONSE_SCHEMA_PATH,
            &snapshot_response_schema,
        ),
        (
            crate::release1_snapshot::ACTIVITY_STREAM_REQUEST_SCHEMA_ID,
            snapshot_artifacts::ACTIVITY_STREAM_REQUEST_SCHEMA_PATH,
            &activity_stream_request_schema,
        ),
        (
            crate::release1_snapshot::ACTIVITY_STREAM_RESPONSE_SCHEMA_ID,
            snapshot_artifacts::ACTIVITY_STREAM_RESPONSE_SCHEMA_PATH,
            &activity_stream_response_schema,
        ),
        (
            crate::TAKE_OVER_PROJECT_WRITER_REQUEST_SCHEMA_ID,
            takeover_artifacts::REQUEST_SCHEMA_PATH,
            &takeover_request_schema,
        ),
        (
            crate::TAKE_OVER_PROJECT_WRITER_RESPONSE_SCHEMA_ID,
            takeover_artifacts::RESPONSE_SCHEMA_PATH,
            &takeover_response_schema,
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
        (
            create_project_artifacts::CHALLENGE_REQUEST_SCHEMA_PATH,
            create_project_challenge_request_schema,
        ),
        (
            create_project_artifacts::CHALLENGE_RESPONSE_SCHEMA_PATH,
            create_project_challenge_response_schema,
        ),
        (
            EDITOR_SESSION_CREATE_REQUEST_SCHEMA_PATH,
            editor_create_request_schema,
        ),
        (
            EDITOR_SESSION_CREATE_RESPONSE_SCHEMA_PATH,
            editor_create_response_schema,
        ),
        (
            EDITOR_SESSION_GET_REQUEST_SCHEMA_PATH,
            editor_get_request_schema,
        ),
        (
            EDITOR_SESSION_GET_RESPONSE_SCHEMA_PATH,
            editor_get_response_schema,
        ),
        (
            author_edit_artifacts::REQUEST_SCHEMA_PATH,
            apply_edit_request_schema,
        ),
        (
            author_edit_artifacts::RESPONSE_SCHEMA_PATH,
            apply_edit_response_schema,
        ),
        (
            author_edit_outcome_artifacts::REQUEST_SCHEMA_PATH,
            apply_edit_outcome_request_schema,
        ),
        (
            author_edit_outcome_artifacts::RESPONSE_SCHEMA_PATH,
            apply_edit_outcome_response_schema,
        ),
        (
            snapshot_artifacts::SNAPSHOT_REQUEST_SCHEMA_PATH,
            snapshot_request_schema,
        ),
        (
            snapshot_artifacts::SNAPSHOT_RESPONSE_SCHEMA_PATH,
            snapshot_response_schema,
        ),
        (
            snapshot_artifacts::ACTIVITY_STREAM_REQUEST_SCHEMA_PATH,
            activity_stream_request_schema,
        ),
        (
            snapshot_artifacts::ACTIVITY_STREAM_RESPONSE_SCHEMA_PATH,
            activity_stream_response_schema,
        ),
        (
            takeover_artifacts::REQUEST_SCHEMA_PATH,
            takeover_request_schema,
        ),
        (
            takeover_artifacts::RESPONSE_SCHEMA_PATH,
            takeover_response_schema,
        ),
        (TYPESCRIPT_CLIENT_PATH, typescript_client_bytes()),
        (TYPESCRIPT_DECLARATION_PATH, typescript_declaration_bytes()),
        (RELEASE_PROFILE_MODULE_PATH, profile_module),
        (
            RELEASE_PROFILE_DECLARATION_PATH,
            release_profile_declaration_bytes(),
        ),
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
        (
            create_project_artifacts::CHALLENGE_FIXTURE_PATHS[0],
            create_project_artifacts::challenge_fixture_bytes(),
        ),
        (
            create_project_artifacts::CHALLENGE_FIXTURE_PATHS[1],
            create_project_artifacts::challenge_invalid_fixture_bytes(),
        ),
        (
            create_project_artifacts::CHALLENGE_FIXTURE_PATHS[2],
            create_project_artifacts::challenge_boundary_fixture_bytes(),
        ),
        (
            CREATE_EDITOR_SESSION_FIXTURE_PATHS[0],
            create_editor_session_fixture_bytes(),
        ),
        (
            CREATE_EDITOR_SESSION_FIXTURE_PATHS[1],
            invalid_create_editor_session_fixture_bytes(),
        ),
        (
            CREATE_EDITOR_SESSION_FIXTURE_PATHS[2],
            boundary_create_editor_session_fixture_bytes(),
        ),
        (
            GET_EDITOR_SESSION_FIXTURE_PATHS[0],
            get_editor_session_fixture_bytes(),
        ),
        (
            GET_EDITOR_SESSION_FIXTURE_PATHS[1],
            invalid_get_editor_session_fixture_bytes(),
        ),
        (
            GET_EDITOR_SESSION_FIXTURE_PATHS[2],
            boundary_get_editor_session_fixture_bytes(),
        ),
        (
            author_edit_artifacts::FIXTURE_PATHS[0],
            author_edit_artifacts::fixture_bytes(),
        ),
        (
            author_edit_artifacts::FIXTURE_PATHS[1],
            author_edit_artifacts::invalid_fixture_bytes(),
        ),
        (
            author_edit_artifacts::FIXTURE_PATHS[2],
            author_edit_artifacts::boundary_fixture_bytes(),
        ),
        (
            author_edit_outcome_artifacts::FIXTURE_PATHS[0],
            author_edit_outcome_artifacts::fixture_bytes(),
        ),
        (
            author_edit_outcome_artifacts::FIXTURE_PATHS[1],
            author_edit_outcome_artifacts::invalid_fixture_bytes(),
        ),
        (
            author_edit_outcome_artifacts::FIXTURE_PATHS[2],
            author_edit_outcome_artifacts::boundary_fixture_bytes(),
        ),
        (
            snapshot_artifacts::SNAPSHOT_FIXTURE_PATHS[0],
            snapshot_artifacts::snapshot_fixture_bytes(),
        ),
        (
            snapshot_artifacts::SNAPSHOT_FIXTURE_PATHS[1],
            snapshot_artifacts::snapshot_invalid_fixture_bytes(),
        ),
        (
            snapshot_artifacts::SNAPSHOT_FIXTURE_PATHS[2],
            snapshot_artifacts::snapshot_boundary_fixture_bytes(),
        ),
        (
            snapshot_artifacts::ACTIVITY_STREAM_FIXTURE_PATHS[0],
            snapshot_artifacts::activity_stream_fixture_bytes(),
        ),
        (
            snapshot_artifacts::ACTIVITY_STREAM_FIXTURE_PATHS[1],
            snapshot_artifacts::activity_stream_invalid_fixture_bytes(),
        ),
        (
            snapshot_artifacts::ACTIVITY_STREAM_FIXTURE_PATHS[2],
            snapshot_artifacts::activity_stream_boundary_fixture_bytes(),
        ),
        (
            takeover_artifacts::FIXTURE_PATHS[0],
            takeover_artifacts::fixture_bytes(),
        ),
        (
            takeover_artifacts::FIXTURE_PATHS[1],
            takeover_artifacts::invalid_fixture_bytes(),
        ),
        (
            takeover_artifacts::FIXTURE_PATHS[2],
            takeover_artifacts::boundary_fixture_bytes(),
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
            challenge_operation_graph(&CREATE_PROJECT_CHALLENGE, &["server_derived_user", "prospective_project_scope", "strict_origin", "protected_client_session_binding", "closed_create_project_schema", "body_idempotency_key"]),
            challenge_operation_graph(&CREATE_PROJECT_COMMAND_CHALLENGE, &["server_derived_project_scope", "strict_origin", "protected_client_session_binding", "route_method_schema_digest_match", "closed_command_schema"]),
            command_operation_graph(&CREATE_EDITOR_SESSION, &["server_derived_project_scope", "strict_origin", "protected_client_session_binding", "project_command_challenge"]),
            operation_graph(&GET_EDITOR_SESSION, &["server_derived_project_scope", "session_scope_join", "protected_client_session_binding"]),
            command_operation_graph(&APPLY_AUTHOR_EDIT, &["server_derived_project_scope", "strict_origin", "protected_client_session_binding", "project_command_challenge", "editor_session_writer_generation", "expected_authoritative_head"]),
            operation_graph(&GET_APPLY_AUTHOR_EDIT_OUTCOME, &["server_derived_project_scope", "sensitive_safe_read_origin", "protected_client_session_binding", "idempotency_key", "project_command_challenge_proof", "receipt_first_settlement_validation"]),
            operation_graph(&GET_SNAPSHOT, &["snapshot_scope_join", "snapshot_signature", "snapshot_lifecycle_available"]),
            stream_operation_graph(&ACTIVITY_STREAM, &["server_derived_project_scope", "snapshot_binding_or_last_event_id", "activity_profile", "replay_generation", "filter_digest", "reauthorize_on_connect"]),
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

fn command_operation_graph(
    operation: &crate::release1::QueryOperation,
    preconditions: &[&str],
) -> Value {
    let mut graph = operation_graph(operation, preconditions);
    graph["kind"] = json!("command");
    graph
}

fn challenge_operation_graph(
    operation: &crate::release1::QueryOperation,
    preconditions: &[&str],
) -> Value {
    let mut graph = operation_graph(operation, preconditions);
    graph["kind"] = json!("challenge");
    graph
}

fn stream_operation_graph(
    operation: &crate::release1::QueryOperation,
    preconditions: &[&str],
) -> Value {
    let mut graph = operation_graph(operation, preconditions);
    graph["kind"] = json!("stream");
    graph
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
        (
            &GET_EDITOR_SESSION,
            "Read one exact Editor Session",
            EDITOR_SESSION_GET_RESPONSE_SCHEMA_PATH,
            &["project_id", "editor_session_id"][..],
        ),
    ];
    let mut paths = operations
        .iter()
        .map(|(operation, summary, response_schema, parameters)| {
            operation_openapi(operation, summary, response_schema, parameters)
        })
        .collect::<String>();
    paths.push_str(&create_project_artifacts::openapi());
    paths.push_str(&challenge_openapi());
    paths.push_str(&editor_session_create_openapi());
    if author_edit_artifacts::IS_IMPLEMENTED {
        paths.push_str(&author_edit_artifacts::openapi());
    }
    paths.push_str(&author_edit_outcome_artifacts::openapi());
    paths.push_str(&snapshot_artifacts::snapshot_openapi());
    paths.push_str(&snapshot_artifacts::activity_stream_openapi());
    paths.push_str(&takeover_artifacts::openapi());
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
        "  {}:\n    post:\n      operationId: {}\n      summary: Create one Editor Session\n      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{request_schema}'\n      responses:\n{responses}",
        CREATE_EDITOR_SESSION.path, CREATE_EDITOR_SESSION.operation_id
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

fn schema_catalog_bytes(schemas: &[(&str, &str, &[u8])]) -> Vec<u8> {
    json_bytes(&json!({
        "schema_id": "storyos.schema-catalog.v1", "public_protocol_release": PUBLIC_PROTOCOL_RELEASE,
        "implemented_operations": implemented_operation_ids(),
        "schemas": schemas.iter().map(|(schema_id, path, bytes)| json!({
            "schema_id": schema_id, "path": path, "sha256": sha256_prefixed(bytes)
        })).collect::<Vec<_>>()
    }))
}

fn implemented_operation_ids() -> Vec<&'static str> {
    let mut operation_ids = vec![
        GET_PROTOCOL_PROFILE.operation_id,
        GET_PROJECT.operation_id,
        GET_CHAPTER.operation_id,
        CREATE_PROJECT_CHALLENGE.operation_id,
        CREATE_PROJECT_COMMAND_CHALLENGE.operation_id,
        CREATE_EDITOR_SESSION.operation_id,
        GET_EDITOR_SESSION.operation_id,
    ];
    if author_edit_artifacts::IS_IMPLEMENTED {
        operation_ids.push(APPLY_AUTHOR_EDIT.operation_id);
    }
    operation_ids.push(GET_APPLY_AUTHOR_EDIT_OUTCOME.operation_id);
    operation_ids.push(GET_SNAPSHOT.operation_id);
    operation_ids.push(ACTIVITY_STREAM.operation_id);
    operation_ids.push(TAKE_OVER_PROJECT_WRITER.operation_id);
    operation_ids
}

fn typescript_client_bytes() -> Vec<u8> {
    let create_project_client = create_project_artifacts::typescript_client_source();
    let author_edit_client = author_edit_artifacts::typescript_client_source();
    let author_edit_outcome_client = author_edit_outcome_artifacts::typescript_client_source();
    format!(concat!(
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
            "async function queryText({{ baseUrl, path, queryHeaders = {{}}, fetchImpl = globalThis.fetch, signal }}) {{\n",
            "  if (typeof baseUrl !== \"string\" || baseUrl.length === 0) throw new TypeError(\"StoryOS query requires a non-empty baseUrl\");\n",
            "  if (typeof fetchImpl !== \"function\") throw new TypeError(\"StoryOS query requires a fetch implementation\");\n",
            "  const response = await fetchImpl(new URL(path, baseUrl), {{ method: \"GET\", headers: {{ ...queryHeaders }}, credentials: \"same-origin\", signal }});\n",
            "  const responseBody = await response.text();\n",
            "  if (!response.ok) throw new StoryOSProtocolError(\"query_http_error\", `StoryOS query failed with HTTP ${{response.status}}`, {{ status: response.status, responseBody }});\n",
            "  return responseBody;\n}}\n\n",
            "async function commandJson({{ baseUrl, path, body, commandHeaders = {{}}, fetchImpl = globalThis.fetch, signal }}) {{\n",
            "  if (typeof baseUrl !== \"string\" || baseUrl.length === 0) throw new TypeError(\"StoryOS command requires a non-empty baseUrl\");\n",
            "  if (typeof fetchImpl !== \"function\") throw new TypeError(\"StoryOS command requires a fetch implementation\");\n",
            "  const response = await fetchImpl(new URL(path, baseUrl), {{ method: \"POST\", headers: {{ accept: \"application/json\", \"content-type\": \"application/json\", ...commandHeaders }}, credentials: \"same-origin\", body: JSON.stringify(body), signal }});\n",
            "  const responseBody = await response.text();\n",
            "  if (!response.ok) {{\n    const retryAfter = response.headers.get(\"retry-after\");\n    const retryAfterSeconds = /^(?:[1-9]|[1-5][0-9]|60)$/.test(retryAfter ?? \"\") ? Number(retryAfter) : undefined;\n    throw new StoryOSProtocolError(\"command_http_error\", `StoryOS command failed with HTTP ${{response.status}}`, {{ status: response.status, responseBody, retryAfterSeconds }});\n  }}\n",
            "  try {{ return JSON.parse(responseBody); }} catch {{ throw new StoryOSProtocolError(\"command_invalid_json\", \"StoryOS command returned invalid JSON\", {{ status: response.status, responseBody }}); }}\n}}\n\n",
            "function canonicalJson(value) {{\n  if (Array.isArray(value)) return value.map(canonicalJson);\n  if (value && typeof value === \"object\") return Object.fromEntries(Object.keys(value).sort().map((key) => [key, canonicalJson(value[key])]));\n  return value;\n}}\n\n",
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
        "{}{}{}{}{}",
    ),
        GENERATED_CLIENT_REVISION,
        GET_PROTOCOL_PROFILE.path,
        GET_PROJECT
            .path
            .replace("{project_id}", "${encodeURIComponent(projectId)}"),
        GET_CHAPTER
            .path
            .replace("{project_id}", "${encodeURIComponent(projectId)}")
            .replace("{chapter_id}", "${encodeURIComponent(chapterId)}"),
        CREATE_PROJECT_COMMAND_CHALLENGE
            .path
            .replace("{project_id}", "${encodeURIComponent(projectId)}"),
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
        create_project_client,
        author_edit_client,
        author_edit_outcome_client,
        snapshot_artifacts::typescript_client_source(),
        takeover_artifacts::typescript_client_source(),
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
    let create_editor_request = CreateEditorSessionRequest::decl(&config);
    let create_editor_response = CreateEditorSessionResponse::decl(&config);
    let get_editor_response = GetEditorSessionResponse::decl(&config);
    let editor_binding = EditorSessionBinding::decl(&config);
    let editor_writer = EditorWriterProjection::decl(&config);
    let editor_reason = EditorReadOnlyReason::decl(&config);
    let editor_snapshot = EditorBaseSnapshot::decl(&config);
    let mut declaration = format!(
        "// @generated by storyos-contracts; do not edit.\nexport {identity}\n\nexport {profile}\n\nexport {project_scope}\n\nexport {controlled_project}\n\nexport {chapter_revision}\n\nexport {current_chapter}\n\nexport {project}\n\nexport {chapter}\n\nexport {digest_algorithm}\n\nexport {digest_value}\n\nexport {challenge_request}\n\nexport {challenge_response}\n\nexport {create_editor_request}\n\nexport {editor_reason}\n\nexport {editor_writer}\n\nexport {editor_binding}\n\nexport {editor_snapshot}\n\nexport {create_editor_response}\n\nexport {get_editor_response}\n\n{}\n\n{}\n\n{}\n\n{}\n\n{}",
        create_project_artifacts::typescript_type_declarations(),
        author_edit_artifacts::typescript_type_declarations(),
        author_edit_outcome_artifacts::typescript_type_declarations(),
        snapshot_artifacts::typescript_type_declarations(),
        takeover_artifacts::typescript_type_declarations(),
    );
    declaration.push_str("\n\n");
    declaration.push_str(
        concat!(
            "export declare const GENERATED_CLIENT_REVISION: string;\n",
            "export declare class StoryOSProtocolError extends Error {\n  readonly code: string;\n  readonly status?: number;\n  readonly responseBody?: string;\n  readonly retryAfterSeconds?: number;\n}\n",
            "export interface StoryOSQueryOptions {\n  baseUrl: string;\n  fetchImpl?: typeof fetch;\n  signal?: AbortSignal;\n}\n",
            "export declare function getProtocolProfile(options: StoryOSQueryOptions): Promise<Release1ProtocolProfile>;\n",
            "export declare function getProject(options: StoryOSQueryOptions & { projectId: string }): Promise<GetProjectResponse>;\n",
            "export declare function getChapter(options: StoryOSQueryOptions & { projectId: string; chapterId: string }): Promise<GetChapterResponse>;\n",
            "export declare function createProjectCommandChallenge(options: StoryOSQueryOptions & { projectId: string; request: CreateProjectCommandChallengeRequest }): Promise<CreateProjectCommandChallengeResponse>;\n",
            "export declare function digestCreateEditorSession(request: CreateEditorSessionRequest, cryptoImpl?: Crypto): Promise<DigestValue>;\n",
            "export declare function createEditorSession(options: StoryOSQueryOptions & { projectId: string; request: CreateEditorSessionRequest; idempotencyKey: string; antiForgery: string }): Promise<CreateEditorSessionResponse>;\n",
            "export declare function getEditorSession(options: StoryOSQueryOptions & { projectId: string; editorSessionId: string }): Promise<GetEditorSessionResponse>;\n",
        )
    );
    declaration.push_str(create_project_artifacts::typescript_declarations());
    declaration.push_str(author_edit_artifacts::typescript_declarations());
    declaration.push_str(author_edit_outcome_artifacts::typescript_declarations());
    declaration.push_str(snapshot_artifacts::typescript_declarations());
    declaration.push_str(takeover_artifacts::typescript_declarations());
    declaration.into_bytes()
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

fn fixture_catalog_bytes(profile: &Release1ProtocolProfile) -> Vec<u8> {
    json_bytes(&json!({
        "schema_id": "storyos.fixture-catalog.release-1.v1", "public_protocol_release": PUBLIC_PROTOCOL_RELEASE,
        "corpus_digest": profile.release_identity.fixture_corpus_digest,
        "digest_scope": {
            "paths": [GOLDEN_PROFILE_PATH, INVALID_PROFILE_PATH, BOUNDARY_PROFILE_PATH,
                PROJECT_FIXTURE_PATHS[0], PROJECT_FIXTURE_PATHS[1], PROJECT_FIXTURE_PATHS[2],
                CHAPTER_FIXTURE_PATHS[0], CHAPTER_FIXTURE_PATHS[1], CHAPTER_FIXTURE_PATHS[2],
                CHALLENGE_FIXTURE_PATHS[0], CHALLENGE_FIXTURE_PATHS[1], CHALLENGE_FIXTURE_PATHS[2],
                create_project_artifacts::CHALLENGE_FIXTURE_PATHS[0], create_project_artifacts::CHALLENGE_FIXTURE_PATHS[1], create_project_artifacts::CHALLENGE_FIXTURE_PATHS[2],
                CREATE_EDITOR_SESSION_FIXTURE_PATHS[0], CREATE_EDITOR_SESSION_FIXTURE_PATHS[1], CREATE_EDITOR_SESSION_FIXTURE_PATHS[2],
                GET_EDITOR_SESSION_FIXTURE_PATHS[0], GET_EDITOR_SESSION_FIXTURE_PATHS[1], GET_EDITOR_SESSION_FIXTURE_PATHS[2],
                author_edit_artifacts::FIXTURE_PATHS[0], author_edit_artifacts::FIXTURE_PATHS[1], author_edit_artifacts::FIXTURE_PATHS[2],
                author_edit_outcome_artifacts::FIXTURE_PATHS[0], author_edit_outcome_artifacts::FIXTURE_PATHS[1], author_edit_outcome_artifacts::FIXTURE_PATHS[2],
                snapshot_artifacts::SNAPSHOT_FIXTURE_PATHS[0], snapshot_artifacts::SNAPSHOT_FIXTURE_PATHS[1], snapshot_artifacts::SNAPSHOT_FIXTURE_PATHS[2],
                snapshot_artifacts::ACTIVITY_STREAM_FIXTURE_PATHS[0], snapshot_artifacts::ACTIVITY_STREAM_FIXTURE_PATHS[1], snapshot_artifacts::ACTIVITY_STREAM_FIXTURE_PATHS[2],
                takeover_artifacts::FIXTURE_PATHS[0], takeover_artifacts::FIXTURE_PATHS[1], takeover_artifacts::FIXTURE_PATHS[2]],
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
            {"fixture_id": CREATE_PROJECT_COMMAND_CHALLENGE.fixtures[2], "classification": "boundary", "operation_id": CREATE_PROJECT_COMMAND_CHALLENGE.operation_id, "path": CHALLENGE_FIXTURE_PATHS[2]},
            {"fixture_id": CREATE_PROJECT_CHALLENGE.fixtures[0], "classification": "positive", "operation_id": CREATE_PROJECT_CHALLENGE.operation_id, "path": create_project_artifacts::CHALLENGE_FIXTURE_PATHS[0]},
            {"fixture_id": CREATE_PROJECT_CHALLENGE.fixtures[1], "classification": "invalid", "operation_id": CREATE_PROJECT_CHALLENGE.operation_id, "path": create_project_artifacts::CHALLENGE_FIXTURE_PATHS[1]},
            {"fixture_id": CREATE_PROJECT_CHALLENGE.fixtures[2], "classification": "boundary", "operation_id": CREATE_PROJECT_CHALLENGE.operation_id, "path": create_project_artifacts::CHALLENGE_FIXTURE_PATHS[2]},
            {"fixture_id": CREATE_EDITOR_SESSION.fixtures[0], "classification": "positive", "operation_id": CREATE_EDITOR_SESSION.operation_id, "path": CREATE_EDITOR_SESSION_FIXTURE_PATHS[0]},
            {"fixture_id": CREATE_EDITOR_SESSION.fixtures[1], "classification": "invalid", "operation_id": CREATE_EDITOR_SESSION.operation_id, "path": CREATE_EDITOR_SESSION_FIXTURE_PATHS[1]},
            {"fixture_id": CREATE_EDITOR_SESSION.fixtures[2], "classification": "boundary", "operation_id": CREATE_EDITOR_SESSION.operation_id, "path": CREATE_EDITOR_SESSION_FIXTURE_PATHS[2]},
            {"fixture_id": GET_EDITOR_SESSION.fixtures[0], "classification": "positive", "operation_id": GET_EDITOR_SESSION.operation_id, "path": GET_EDITOR_SESSION_FIXTURE_PATHS[0]},
            {"fixture_id": GET_EDITOR_SESSION.fixtures[1], "classification": "invalid", "operation_id": GET_EDITOR_SESSION.operation_id, "path": GET_EDITOR_SESSION_FIXTURE_PATHS[1]},
            {"fixture_id": GET_EDITOR_SESSION.fixtures[2], "classification": "boundary", "operation_id": GET_EDITOR_SESSION.operation_id, "path": GET_EDITOR_SESSION_FIXTURE_PATHS[2]},
            {"fixture_id": APPLY_AUTHOR_EDIT.fixtures[0], "classification": "positive", "operation_id": APPLY_AUTHOR_EDIT.operation_id, "path": author_edit_artifacts::FIXTURE_PATHS[0]},
            {"fixture_id": APPLY_AUTHOR_EDIT.fixtures[1], "classification": "invalid", "operation_id": APPLY_AUTHOR_EDIT.operation_id, "path": author_edit_artifacts::FIXTURE_PATHS[1]},
            {"fixture_id": APPLY_AUTHOR_EDIT.fixtures[2], "classification": "boundary", "operation_id": APPLY_AUTHOR_EDIT.operation_id, "path": author_edit_artifacts::FIXTURE_PATHS[2]},
            {"fixture_id": GET_APPLY_AUTHOR_EDIT_OUTCOME.fixtures[0], "classification": "positive", "operation_id": GET_APPLY_AUTHOR_EDIT_OUTCOME.operation_id, "path": author_edit_outcome_artifacts::FIXTURE_PATHS[0]},
            {"fixture_id": GET_APPLY_AUTHOR_EDIT_OUTCOME.fixtures[1], "classification": "invalid", "operation_id": GET_APPLY_AUTHOR_EDIT_OUTCOME.operation_id, "path": author_edit_outcome_artifacts::FIXTURE_PATHS[1]},
            {"fixture_id": GET_APPLY_AUTHOR_EDIT_OUTCOME.fixtures[2], "classification": "boundary", "operation_id": GET_APPLY_AUTHOR_EDIT_OUTCOME.operation_id, "path": author_edit_outcome_artifacts::FIXTURE_PATHS[2]},
            {"fixture_id": GET_SNAPSHOT.fixtures[0], "classification": "positive", "operation_id": GET_SNAPSHOT.operation_id, "path": snapshot_artifacts::SNAPSHOT_FIXTURE_PATHS[0]},
            {"fixture_id": GET_SNAPSHOT.fixtures[1], "classification": "invalid", "operation_id": GET_SNAPSHOT.operation_id, "path": snapshot_artifacts::SNAPSHOT_FIXTURE_PATHS[1]},
            {"fixture_id": GET_SNAPSHOT.fixtures[2], "classification": "boundary", "operation_id": GET_SNAPSHOT.operation_id, "path": snapshot_artifacts::SNAPSHOT_FIXTURE_PATHS[2]},
            {"fixture_id": ACTIVITY_STREAM.fixtures[0], "classification": "positive", "operation_id": ACTIVITY_STREAM.operation_id, "path": snapshot_artifacts::ACTIVITY_STREAM_FIXTURE_PATHS[0]},
            {"fixture_id": ACTIVITY_STREAM.fixtures[1], "classification": "invalid", "operation_id": ACTIVITY_STREAM.operation_id, "path": snapshot_artifacts::ACTIVITY_STREAM_FIXTURE_PATHS[1]},
            {"fixture_id": ACTIVITY_STREAM.fixtures[2], "classification": "boundary", "operation_id": ACTIVITY_STREAM.operation_id, "path": snapshot_artifacts::ACTIVITY_STREAM_FIXTURE_PATHS[2]},
            {"fixture_id": TAKE_OVER_PROJECT_WRITER.fixtures[0], "classification": "positive", "operation_id": TAKE_OVER_PROJECT_WRITER.operation_id, "path": takeover_artifacts::FIXTURE_PATHS[0]},
            {"fixture_id": TAKE_OVER_PROJECT_WRITER.fixtures[1], "classification": "invalid", "operation_id": TAKE_OVER_PROJECT_WRITER.operation_id, "path": takeover_artifacts::FIXTURE_PATHS[1]},
            {"fixture_id": TAKE_OVER_PROJECT_WRITER.fixtures[2], "classification": "boundary", "operation_id": TAKE_OVER_PROJECT_WRITER.operation_id, "path": takeover_artifacts::FIXTURE_PATHS[2]}
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
        create_project_artifacts::challenge_fixture_bytes(),
        create_project_artifacts::challenge_invalid_fixture_bytes(),
        create_project_artifacts::challenge_boundary_fixture_bytes(),
        create_editor_session_fixture_bytes(),
        invalid_create_editor_session_fixture_bytes(),
        boundary_create_editor_session_fixture_bytes(),
        get_editor_session_fixture_bytes(),
        invalid_get_editor_session_fixture_bytes(),
        boundary_get_editor_session_fixture_bytes(),
        author_edit_artifacts::fixture_bytes(),
        author_edit_artifacts::invalid_fixture_bytes(),
        author_edit_artifacts::boundary_fixture_bytes(),
        author_edit_outcome_artifacts::fixture_bytes(),
        author_edit_outcome_artifacts::invalid_fixture_bytes(),
        author_edit_outcome_artifacts::boundary_fixture_bytes(),
        snapshot_artifacts::snapshot_fixture_bytes(),
        snapshot_artifacts::snapshot_invalid_fixture_bytes(),
        snapshot_artifacts::snapshot_boundary_fixture_bytes(),
        snapshot_artifacts::activity_stream_fixture_bytes(),
        snapshot_artifacts::activity_stream_invalid_fixture_bytes(),
        snapshot_artifacts::activity_stream_boundary_fixture_bytes(),
        takeover_artifacts::fixture_bytes(),
        takeover_artifacts::invalid_fixture_bytes(),
        takeover_artifacts::boundary_fixture_bytes(),
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
        "project_activity_position": "0",
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
            "materialized_revision": {"revision_id": "018f0000-0000-7001-8000-000000000004", "body": "雨落在窗沿。"},
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

fn json_bytes(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("contract JSON should serialize");
    bytes.push(b'\n');
    bytes
}

#[cfg(test)]
#[path = "release1_create_project_artifacts_tests.rs"]
mod create_project_tests;

#[cfg(test)]
#[path = "release1_artifacts_tests.rs"]
mod tests;
