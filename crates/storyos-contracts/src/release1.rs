use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub(super) const API_MAJOR: u32 = 1;
pub(super) const PUBLIC_PROTOCOL_RELEASE: &str = "storyos.public.release.1";
pub(super) const ENVELOPE_VERSION: u32 = 1;
pub(super) const ENVELOPE_PROFILE: &str = "storyos.public.envelope.v1";
pub(super) const PROBLEM_PROFILE: &str = "storyos.problem.v1";
pub(super) const ACTIVITY_PROFILE: &str = "storyos.project-activity.v1";
pub const LIMIT_PROFILE_REVISION: &str = "storyos.foundation.absolute.v1";
pub(super) const COMPATIBILITY_PROFILE: &str = "storyos.public.same-release.v1";
pub(super) const CONTRACT_REVISION: &str = "release1-wire-catalog-2026-08-12";
pub(super) const WEB_CLIENT_CONTRACT_REVISION: &str = "storyos.web-client.release-1.v1";
pub(super) const SERVER_CONTRACT_REVISION: &str = "storyos.server.release-1.v1";
pub(super) const WORKER_CONTRACT_REVISION: &str = "storyos.worker.release-1.v1";
pub(super) const GENERATED_CLIENT_REVISION: &str = "storyos.typescript-client.release-1.v1";
pub(super) const PROTOCOL_PROFILE_REQUEST_SCHEMA_ID: &str =
    "storyos.query.protocol-profile.request.v1";
pub(super) const PROTOCOL_PROFILE_SCHEMA_ID: &str = "storyos.query.protocol-profile.response.v1";
pub(super) const PROJECT_REQUEST_SCHEMA_ID: &str = "storyos.query.project.request.v1";
pub(super) const PROJECT_RESPONSE_SCHEMA_ID: &str = "storyos.query.project.response.v1";
pub(super) const CHAPTER_REQUEST_SCHEMA_ID: &str = "storyos.query.chapter.request.v1";
pub(super) const CHAPTER_RESPONSE_SCHEMA_ID: &str = "storyos.query.chapter.response.v1";
pub(super) const PROJECT_COMMAND_CHALLENGE_REQUEST_SCHEMA_ID: &str =
    "storyos.challenge.project-command-anti-forgery-challenge.request.v1";
pub(super) const PROJECT_COMMAND_CHALLENGE_RESPONSE_SCHEMA_ID: &str =
    "storyos.challenge.project-command-anti-forgery-challenge.response.v1";
pub(super) const RELEASE_IDENTITY_SCHEMA_ID: &str = "storyos.compatibility.release-identity.v1";
pub(super) const POSITIVE_FIXTURE_ID: &str = "storyos.golden.getProtocolProfile.positive.v1";
pub(super) const INVALID_FIXTURE_ID: &str = "storyos.golden.getProtocolProfile.invalid.v1";
pub(super) const BOUNDARY_FIXTURE_ID: &str = "storyos.golden.getProtocolProfile.boundary.v1";

pub(super) struct QueryOperation {
    pub operation_id: &'static str,
    pub method: &'static str,
    pub path: &'static str,
    pub request_schema: &'static str,
    pub response_schema: &'static str,
    pub responses: &'static [(u16, &'static str)],
    pub fixtures: &'static [&'static str],
}

pub(super) const GET_PROTOCOL_PROFILE: QueryOperation = QueryOperation {
    operation_id: "getProtocolProfile",
    method: "GET",
    path: "/api/v1/protocol",
    request_schema: PROTOCOL_PROFILE_REQUEST_SCHEMA_ID,
    response_schema: PROTOCOL_PROFILE_SCHEMA_ID,
    responses: &[
        (200, "Active protocol profile"),
        (400, "Invalid request"),
        (405, "Method not allowed"),
        (409, "Active release conflict"),
        (429, "Rate limited"),
    ],
    fixtures: &[POSITIVE_FIXTURE_ID, INVALID_FIXTURE_ID, BOUNDARY_FIXTURE_ID],
};

pub(super) const GET_PROJECT: QueryOperation = QueryOperation {
    operation_id: "getProject",
    method: "GET",
    path: "/api/v1/projects/{project_id}",
    request_schema: PROJECT_REQUEST_SCHEMA_ID,
    response_schema: PROJECT_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Current Project"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.getProject.positive.v1",
        "storyos.golden.getProject.invalid.v1",
        "storyos.golden.getProject.boundary.v1",
    ],
};

pub(super) const GET_CHAPTER: QueryOperation = QueryOperation {
    operation_id: "getChapter",
    method: "GET",
    path: "/api/v1/projects/{project_id}/chapters/{chapter_id}",
    request_schema: CHAPTER_REQUEST_SCHEMA_ID,
    response_schema: CHAPTER_RESPONSE_SCHEMA_ID,
    responses: GET_PROJECT.responses,
    fixtures: &[
        "storyos.golden.getChapter.positive.v1",
        "storyos.golden.getChapter.invalid.v1",
        "storyos.golden.getChapter.boundary.v1",
    ],
};

pub(super) const CREATE_PROJECT_COMMAND_CHALLENGE: QueryOperation = QueryOperation {
    operation_id: "createProjectCommandChallenge",
    method: "POST",
    path: "/api/v1/projects/{project_id}/anti-forgery-challenges",
    request_schema: PROJECT_COMMAND_CHALLENGE_REQUEST_SCHEMA_ID,
    response_schema: PROJECT_COMMAND_CHALLENGE_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Project command challenge"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency binding conflict"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Command target refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.createProjectCommandChallenge.positive.v1",
        "storyos.golden.createProjectCommandChallenge.invalid.v1",
        "storyos.golden.createProjectCommandChallenge.boundary.v1",
    ],
};

/// Public route used by the generated client and StoryOS Server.
pub const GET_PROTOCOL_PROFILE_PATH: &str = GET_PROTOCOL_PROFILE.path;
pub const GET_PROTOCOL_PROFILE_METHOD: &str = GET_PROTOCOL_PROFILE.method;
pub const GET_PROJECT_PATH: &str = GET_PROJECT.path;
pub const GET_PROJECT_METHOD: &str = GET_PROJECT.method;
pub const GET_CHAPTER_PATH: &str = GET_CHAPTER.path;
pub const GET_CHAPTER_METHOD: &str = GET_CHAPTER.method;
pub const CREATE_PROJECT_COMMAND_CHALLENGE_PATH: &str = CREATE_PROJECT_COMMAND_CHALLENGE.path;
pub const CREATE_PROJECT_COMMAND_CHALLENGE_METHOD: &str = CREATE_PROJECT_COMMAND_CHALLENGE.method;

pub(super) const REQUIRED_CAPABILITIES: [&str; 14] = [
    "project_lifecycle",
    "volume_chapter_lifecycle",
    "chapter_navigation",
    "manuscript_search",
    "direct_author_edit",
    "single_match_replace",
    "multi_match_cross_location_proposal",
    "manuscript_statistics",
    "human_readable_manuscript_export",
    "portable_project_archive",
    "editor_session_takeover",
    "snapshot_resync",
    "proposal_draft_lifecycle",
    "project_activity",
];

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Release1ProtocolProfile {
    pub schema_id: String,
    pub api_major: u32,
    pub public_protocol_release: String,
    pub envelope_version: u32,
    pub problem_profile: String,
    pub activity_profile: String,
    pub limit_profile_revision: String,
    pub compatibility_profile: String,
    pub contract_revision: String,
    pub required_capabilities: Vec<String>,
    pub release_identity: Release1CompatibilityIdentity,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Release1CompatibilityIdentity {
    pub schema_id: String,
    pub api_major: u32,
    pub public_protocol_release: String,
    pub envelope_profile: String,
    pub contract_graph_revision: String,
    pub contract_graph_digest: String,
    pub web_client_contract_revision: String,
    pub server_contract_revision: String,
    pub worker_contract_revision: String,
    pub generated_client_revision: String,
    pub openapi_digest: String,
    pub json_schema_catalog_digest: String,
    pub typescript_artifact_digest: String,
    pub fixture_corpus_digest: String,
    pub activity_profile: String,
    pub limit_profile_revision: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProjectScope {
    pub owner_user_id: String,
    pub project_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CreateProjectCommandChallengeRequest {
    pub method: String,
    pub route_template: String,
    pub command_schema: String,
    pub canonical_command_digest: DigestValue,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DigestValue {
    pub algorithm: DigestAlgorithm,
    pub profile: String,
    pub value_hex_lowercase: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub enum DigestAlgorithm {
    #[serde(rename = "sha256")]
    #[ts(rename = "sha256")]
    Sha256,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CreateProjectCommandChallengeResponse {
    pub nonce: String,
    pub expires_at: String,
    pub limit_profile_revision: String,
}

pub use crate::project_command_targets::project_command_kind;
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ControlledProject {
    pub project_id: String,
    pub title: String,
    pub current_chapter_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GetProjectResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: ProjectScope,
    pub project: ControlledProject,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AuthoritativeChapterRevision {
    pub revision_id: String,
    pub body: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CurrentChapter {
    pub chapter_id: String,
    pub title: String,
    pub current_revision: AuthoritativeChapterRevision,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GetChapterResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: ProjectScope,
    pub chapter: CurrentChapter,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct StoryOSProblem {
    pub schema_id: String,
    pub code: String,
    pub message: String,
}

pub(super) struct ArtifactDigests {
    pub contract_graph: String,
    pub openapi: String,
    pub json_schema_catalog: String,
    pub typescript: String,
    pub fixture_corpus: String,
}

pub(super) fn protocol_profile(digests: ArtifactDigests) -> Release1ProtocolProfile {
    Release1ProtocolProfile {
        schema_id: PROTOCOL_PROFILE_SCHEMA_ID.to_owned(),
        api_major: API_MAJOR,
        public_protocol_release: PUBLIC_PROTOCOL_RELEASE.to_owned(),
        envelope_version: ENVELOPE_VERSION,
        problem_profile: PROBLEM_PROFILE.to_owned(),
        activity_profile: ACTIVITY_PROFILE.to_owned(),
        limit_profile_revision: LIMIT_PROFILE_REVISION.to_owned(),
        compatibility_profile: COMPATIBILITY_PROFILE.to_owned(),
        contract_revision: CONTRACT_REVISION.to_owned(),
        required_capabilities: REQUIRED_CAPABILITIES.map(str::to_owned).to_vec(),
        release_identity: Release1CompatibilityIdentity {
            schema_id: RELEASE_IDENTITY_SCHEMA_ID.to_owned(),
            api_major: API_MAJOR,
            public_protocol_release: PUBLIC_PROTOCOL_RELEASE.to_owned(),
            envelope_profile: ENVELOPE_PROFILE.to_owned(),
            contract_graph_revision: CONTRACT_REVISION.to_owned(),
            contract_graph_digest: digests.contract_graph,
            web_client_contract_revision: WEB_CLIENT_CONTRACT_REVISION.to_owned(),
            server_contract_revision: SERVER_CONTRACT_REVISION.to_owned(),
            worker_contract_revision: WORKER_CONTRACT_REVISION.to_owned(),
            generated_client_revision: GENERATED_CLIENT_REVISION.to_owned(),
            openapi_digest: digests.openapi,
            json_schema_catalog_digest: digests.json_schema_catalog,
            typescript_artifact_digest: digests.typescript,
            fixture_corpus_digest: digests.fixture_corpus,
            activity_profile: ACTIVITY_PROFILE.to_owned(),
            limit_profile_revision: LIMIT_PROFILE_REVISION.to_owned(),
        },
    }
}

#[cfg(test)]
#[path = "release1_tests.rs"]
mod tests;
