use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, QueryOperation};
use crate::release1_author_edit::DomainReceipt;

pub const CREATE_CHAPTER_REQUEST_SCHEMA_ID: &str = "storyos.command.create-chapter.request.v1";
pub const CREATE_CHAPTER_RESPONSE_SCHEMA_ID: &str = "storyos.command.create-chapter.response.v1";
pub const CREATE_CHAPTER_DIGEST_PROFILE: &str = "storyos.command.createChapter.jcs.v1";

pub(super) const CREATE_CHAPTER: QueryOperation = QueryOperation {
    operation_id: "createChapter",
    method: "POST",
    path: "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters",
    request_schema: CREATE_CHAPTER_REQUEST_SCHEMA_ID,
    response_schema: CREATE_CHAPTER_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Chapter created"),
        (201, "Chapter created"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency or tree revision conflict"),
        (412, "Session binding refused"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Create Chapter refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.createChapter.positive.v1",
        "storyos.golden.createChapter.invalid.v1",
        "storyos.golden.createChapter.boundary.v1",
    ],
};

pub const CREATE_CHAPTER_PATH: &str = CREATE_CHAPTER.path;
pub const CREATE_CHAPTER_METHOD: &str = CREATE_CHAPTER.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CreateChapterInput {
    pub title: String,
    pub expected_tree_revision: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CreateChapterRequest {
    pub command_schema: String,
    pub create_chapter_input: CreateChapterInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CreateChapterConflictReason {
    StaleTreeRevision,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CreateChapterRefusalReason {
    ArchivedProject,
    InvalidTitle,
    InvalidVolumeJoin,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CreateChapterEffect {
    AuthoritativeApplied {
        volume_id: String,
        chapter_id: String,
        title: String,
        tree_revision: String,
        order: String,
        current_chapter_id: String,
        project_activity_position: String,
    },
    Conflicted {
        reason: CreateChapterConflictReason,
    },
    Refused {
        reason: CreateChapterRefusalReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CreateChapterResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: DomainReceipt,
    pub project: ControlledProject,
    pub effect: CreateChapterEffect,
}
