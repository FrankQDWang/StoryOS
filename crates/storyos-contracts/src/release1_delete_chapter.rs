use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, QueryOperation};
use crate::release1_author_edit::DomainReceipt;

pub const DELETE_CHAPTER_REQUEST_SCHEMA_ID: &str = "storyos.command.delete-chapter.request.v1";
pub const DELETE_CHAPTER_RESPONSE_SCHEMA_ID: &str = "storyos.command.delete-chapter.response.v1";
pub const DELETE_CHAPTER_DIGEST_PROFILE: &str = "storyos.command.deleteChapter.jcs.v1";

pub(super) const DELETE_CHAPTER: QueryOperation = QueryOperation {
    operation_id: "deleteChapter",
    method: "DELETE",
    path: "/api/v1/projects/{project_id}/chapters/{chapter_id}",
    request_schema: DELETE_CHAPTER_REQUEST_SCHEMA_ID,
    response_schema: DELETE_CHAPTER_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Chapter removed"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency or Chapter revision conflict"),
        (412, "Session binding refused"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Delete Chapter refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.deleteChapter.positive.v1",
        "storyos.golden.deleteChapter.invalid.v1",
        "storyos.golden.deleteChapter.boundary.v1",
    ],
};

pub const DELETE_CHAPTER_PATH: &str = DELETE_CHAPTER.path;
pub const DELETE_CHAPTER_METHOD: &str = DELETE_CHAPTER.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DeleteChapterInput {
    pub expected_chapter_revision: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DeleteChapterRequest {
    pub command_schema: String,
    pub delete_chapter_input: DeleteChapterInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DeleteChapterNoEffectReason {
    AlreadyRemoved,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DeleteChapterConflictReason {
    StaleChapterRevision,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DeleteChapterRefusalReason {
    ArchivedProject,
    InvalidChapterJoin,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DeleteChapterEffect {
    AuthoritativeApplied {
        chapter_id: String,
        volume_id: String,
        tree_revision: String,
        project_activity_position: String,
    },
    NoEffect {
        reason: DeleteChapterNoEffectReason,
    },
    Conflicted {
        reason: DeleteChapterConflictReason,
    },
    Refused {
        reason: DeleteChapterRefusalReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DeleteChapterResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: DomainReceipt,
    pub project: ControlledProject,
    pub effect: DeleteChapterEffect,
}
