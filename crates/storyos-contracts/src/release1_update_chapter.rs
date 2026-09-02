use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, QueryOperation};
use crate::release1_author_edit::DomainReceipt;

pub const UPDATE_CHAPTER_REQUEST_SCHEMA_ID: &str = "storyos.command.update-chapter.request.v1";
pub const UPDATE_CHAPTER_RESPONSE_SCHEMA_ID: &str = "storyos.command.update-chapter.response.v1";
pub const UPDATE_CHAPTER_DIGEST_PROFILE: &str = "storyos.command.updateChapter.jcs.v1";

pub(super) const UPDATE_CHAPTER: QueryOperation = QueryOperation {
    operation_id: "updateChapter",
    method: "PATCH",
    path: "/api/v1/projects/{project_id}/chapters/{chapter_id}",
    request_schema: UPDATE_CHAPTER_REQUEST_SCHEMA_ID,
    response_schema: UPDATE_CHAPTER_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Chapter renamed or reordered"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency or tree revision conflict"),
        (412, "Session binding refused"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Update Chapter refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.updateChapter.positive.v1",
        "storyos.golden.updateChapter.invalid.v1",
        "storyos.golden.updateChapter.boundary.v1",
    ],
};

pub const UPDATE_CHAPTER_PATH: &str = UPDATE_CHAPTER.path;
pub const UPDATE_CHAPTER_METHOD: &str = UPDATE_CHAPTER.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UpdateChapterInput {
    pub title: String,
    pub order: String,
    pub expected_tree_revision: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UpdateChapterRequest {
    pub command_schema: String,
    pub update_chapter_input: UpdateChapterInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum UpdateChapterNoEffectReason {
    Unchanged,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum UpdateChapterConflictReason {
    StaleTreeRevision,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum UpdateChapterRefusalReason {
    ArchivedProject,
    InvalidTitle,
    InvalidOrder,
    InvalidChapterJoin,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UpdateChapterEffect {
    AuthoritativeApplied {
        chapter_id: String,
        title: String,
        tree_revision: String,
        order: String,
        project_activity_position: String,
    },
    NoEffect {
        reason: UpdateChapterNoEffectReason,
    },
    Conflicted {
        reason: UpdateChapterConflictReason,
    },
    Refused {
        reason: UpdateChapterRefusalReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UpdateChapterResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: DomainReceipt,
    pub project: ControlledProject,
    pub effect: UpdateChapterEffect,
}
