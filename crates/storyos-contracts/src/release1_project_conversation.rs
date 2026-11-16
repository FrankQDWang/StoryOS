//! The two queries of the Project Conversations of a Project (ADR 0094 section 10).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{HttpMethod, ProjectScope, QueryOperation};
use crate::release1_agent_run::AgentRunStatus;

pub const LIST_PROJECT_CONVERSATIONS_REQUEST_SCHEMA_ID: &str =
    "storyos.query.project-conversations.request.v1";
pub const LIST_PROJECT_CONVERSATIONS_RESPONSE_SCHEMA_ID: &str =
    "storyos.query.project-conversations.response.v1";
pub const GET_PROJECT_CONVERSATION_REQUEST_SCHEMA_ID: &str =
    "storyos.query.project-conversation.request.v1";
pub const GET_PROJECT_CONVERSATION_RESPONSE_SCHEMA_ID: &str =
    "storyos.query.project-conversation.response.v1";
/// The Host bound of the entries of one Conversation list. It is an initial value.
pub const PROJECT_CONVERSATION_LIST_MAX_ENTRIES: usize = 64;
/// The Host bound of the characters of one Conversation label. It is an initial value.
pub const PROJECT_CONVERSATION_LABEL_MAX_CHARACTERS: usize = 80;
/// The Host bound of the Runs of one Conversation read. It is an initial value.
pub const PROJECT_CONVERSATION_RUNS_MAX_ENTRIES: usize = 64;

pub(super) const LIST_PROJECT_CONVERSATIONS: QueryOperation = QueryOperation {
    operation_id: "listProjectConversations",
    method: HttpMethod::Get,
    path: "/api/v1/projects/{project_id}/conversations",
    request_schema: LIST_PROJECT_CONVERSATIONS_REQUEST_SCHEMA_ID,
    response_schema: LIST_PROJECT_CONVERSATIONS_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Project Conversations of one Project"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Active release conflict"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Request refused"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixture: "storyos.golden.listProjectConversations.positive.v1",
};

pub(super) const GET_PROJECT_CONVERSATION: QueryOperation = QueryOperation {
    operation_id: "getProjectConversation",
    method: HttpMethod::Get,
    path: "/api/v1/projects/{project_id}/conversations/{conversation_id}",
    request_schema: GET_PROJECT_CONVERSATION_REQUEST_SCHEMA_ID,
    response_schema: GET_PROJECT_CONVERSATION_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Runs of one Project Conversation"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Active release conflict"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Request refused"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixture: "storyos.golden.getProjectConversation.positive.v1",
};

pub const LIST_PROJECT_CONVERSATIONS_PATH: &str = LIST_PROJECT_CONVERSATIONS.path;
pub const LIST_PROJECT_CONVERSATIONS_METHOD: &str = LIST_PROJECT_CONVERSATIONS.method.as_str();
pub const GET_PROJECT_CONVERSATION_PATH: &str = GET_PROJECT_CONVERSATION.path;
pub const GET_PROJECT_CONVERSATION_METHOD: &str = GET_PROJECT_CONVERSATION.method.as_str();

/// `truncated` when the Host bound removed the oldest entries or Runs.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProjectConversationCompleteness {
    Complete,
    Truncated,
}

/// One Project Conversation in the list. The times are the Receipt times of the `createAgentRun`
/// commands of its first Run and of its newest Run.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProjectConversationEntry {
    pub conversation_id: String,
    pub label: String,
    pub first_run_created_at: String,
    pub newest_run_created_at: String,
    pub newest_run_status: AgentRunStatus,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ListProjectConversationsResponse {
    pub schema_id: String,
    pub correlation_id: String,
    #[ts(type = "ProjectScope")]
    pub project_scope: ProjectScope,
    pub completeness: ProjectConversationCompleteness,
    pub conversations: Vec<ProjectConversationEntry>,
}

/// One Run of a Project Conversation. `created_at` is the Receipt time of its `createAgentRun`.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProjectConversationRun {
    pub run_id: String,
    pub author_message: String,
    pub chapter_id: String,
    pub status: AgentRunStatus,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GetProjectConversationResponse {
    pub schema_id: String,
    pub correlation_id: String,
    #[ts(type = "ProjectScope")]
    pub project_scope: ProjectScope,
    pub conversation_id: String,
    pub completeness: ProjectConversationCompleteness,
    /// True when the newest Run ended more than the Provider retention before the read.
    pub earlier_turns_expired: bool,
    pub runs: Vec<ProjectConversationRun>,
}
