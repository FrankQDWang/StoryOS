use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ProjectOpenState, ProjectScope, QueryOperation};

pub const LIST_PROJECTS_REQUEST_SCHEMA_ID: &str = "storyos.query.project-list.request.v1";
pub const LIST_PROJECTS_RESPONSE_SCHEMA_ID: &str = "storyos.query.project-list.response.v1";

pub(super) const LIST_PROJECTS: QueryOperation = QueryOperation {
    operation_id: "listProjects",
    method: "GET",
    path: "/api/v1/projects",
    request_schema: LIST_PROJECTS_REQUEST_SCHEMA_ID,
    response_schema: LIST_PROJECTS_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Current User Project library"),
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
    fixtures: &[
        "storyos.golden.listProjects.positive.v1",
        "storyos.golden.listProjects.invalid.v1",
        "storyos.golden.listProjects.boundary.v1",
    ],
};

pub const LIST_PROJECTS_PATH: &str = LIST_PROJECTS.path;
pub const LIST_PROJECTS_METHOD: &str = LIST_PROJECTS.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProjectLifecycleState {
    Active,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProjectListItem {
    pub project_scope: ProjectScope,
    pub title: String,
    pub lifecycle: ProjectLifecycleState,
    pub revision: String,
    pub open: ProjectOpenState,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ListProjectsResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub owner_user_id: String,
    pub projects: Vec<ProjectListItem>,
}
