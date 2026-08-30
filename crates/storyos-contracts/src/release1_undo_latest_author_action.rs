use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{AuthoritativeChapterRevision, ControlledProject, QueryOperation};
use crate::release1_author_edit::DomainReceipt;

pub const UNDO_LATEST_AUTHOR_ACTION_REQUEST_SCHEMA_ID: &str =
    "storyos.command.undo-latest-author-action.request.v1";
pub const UNDO_LATEST_AUTHOR_ACTION_RESPONSE_SCHEMA_ID: &str =
    "storyos.command.undo-latest-author-action.response.v1";
pub const UNDO_LATEST_AUTHOR_ACTION_DIGEST_PROFILE: &str =
    "storyos.command.undoLatestAuthorAction.jcs.v1";

pub(super) const UNDO_LATEST_AUTHOR_ACTION: QueryOperation = QueryOperation {
    operation_id: "undoLatestAuthorAction",
    method: "POST",
    path: "/api/v1/projects/{project_id}/author-actions/undo",
    request_schema: UNDO_LATEST_AUTHOR_ACTION_REQUEST_SCHEMA_ID,
    response_schema: UNDO_LATEST_AUTHOR_ACTION_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Author Undo settled"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency or frontier conflict"),
        (412, "Session or writer binding refused"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Undo Latest Author Action refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.undoLatestAuthorAction.positive.v1",
        "storyos.golden.undoLatestAuthorAction.invalid.v1",
        "storyos.golden.undoLatestAuthorAction.boundary.v1",
    ],
};

pub const UNDO_LATEST_AUTHOR_ACTION_PATH: &str = UNDO_LATEST_AUTHOR_ACTION.path;
pub const UNDO_LATEST_AUTHOR_ACTION_METHOD: &str = UNDO_LATEST_AUTHOR_ACTION.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UndoLatestAuthorActionInput {
    pub expected_author_undo_frontier_sequence: String,
    pub expected_authoritative_revision_id: String,
    pub editor_session_id: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UndoLatestAuthorActionRequest {
    pub command_schema: String,
    pub undo_latest_author_action_input: UndoLatestAuthorActionInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum UndoLatestAuthorActionConflictReason {
    FrontierMismatch,
    WrongTargetHead,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum UndoLatestAuthorActionUnavailableReason {
    NoFrontier,
    Barrier,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UndoLatestAuthorActionEffect {
    Compensated {
        source_sequence: String,
        author_action_sequence: String,
        authoritative_commit_id: String,
        authoritative_revision: AuthoritativeChapterRevision,
        project_activity_position: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        author_undo_frontier_sequence: Option<String>,
    },
    Conflicted {
        reason: UndoLatestAuthorActionConflictReason,
        #[serde(skip_serializing_if = "Option::is_none")]
        current_author_undo_frontier_sequence: Option<String>,
    },
    Unavailable {
        reason: UndoLatestAuthorActionUnavailableReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UndoLatestAuthorActionResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: DomainReceipt,
    pub project: ControlledProject,
    pub effect: UndoLatestAuthorActionEffect,
}
