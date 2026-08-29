use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, QueryOperation};
use crate::release1_author_edit::DomainReceipt;

pub const SET_CURRENT_CHAPTER_REQUEST_SCHEMA_ID: &str =
    "storyos.command.set-current-chapter.request.v1";
pub const SET_CURRENT_CHAPTER_RESPONSE_SCHEMA_ID: &str =
    "storyos.command.set-current-chapter.response.v1";
pub const SET_CURRENT_CHAPTER_DIGEST_PROFILE: &str = "storyos.command.setCurrentChapter.jcs.v1";

pub(super) const SET_CURRENT_CHAPTER: QueryOperation = QueryOperation {
    operation_id: "setCurrentChapter",
    method: "PUT",
    path: "/api/v1/projects/{project_id}/current-chapter",
    request_schema: SET_CURRENT_CHAPTER_REQUEST_SCHEMA_ID,
    response_schema: SET_CURRENT_CHAPTER_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Current Chapter updated"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency, current Chapter, or Head conflict"),
        (412, "Session or writer binding refused"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Set Current Chapter refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.setCurrentChapter.positive.v1",
        "storyos.golden.setCurrentChapter.invalid.v1",
        "storyos.golden.setCurrentChapter.boundary.v1",
    ],
};

pub const SET_CURRENT_CHAPTER_PATH: &str = SET_CURRENT_CHAPTER.path;
pub const SET_CURRENT_CHAPTER_METHOD: &str = SET_CURRENT_CHAPTER.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SetCurrentChapterInput {
    pub chapter_id: String,
    pub expected_current_chapter_id: String,
    pub expected_target_revision_id: String,
    pub editor_session_id: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SetCurrentChapterRequest {
    pub command_schema: String,
    pub set_current_chapter_input: SetCurrentChapterInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SetCurrentChapterNoEffectReason {
    AlreadyCurrent,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SetCurrentChapterConflictReason {
    StaleCurrentChapter,
    WrongTargetHead,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SetCurrentChapterRefusalReason {
    ArchivedProject,
    InvalidChapterJoin,
    EmptyProject,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SetCurrentChapterEffect {
    AuthoritativeApplied {
        current_chapter_id: String,
        base_snapshot_id: String,
        project_activity_position: String,
    },
    NoEffect {
        reason: SetCurrentChapterNoEffectReason,
    },
    Conflicted {
        reason: SetCurrentChapterConflictReason,
    },
    Refused {
        reason: SetCurrentChapterRefusalReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SetCurrentChapterResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: DomainReceipt,
    pub project: ControlledProject,
    pub effect: SetCurrentChapterEffect,
}
