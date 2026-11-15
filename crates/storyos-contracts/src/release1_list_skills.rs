use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{HttpMethod, ProjectScope, QueryOperation};
use crate::release1_agent_run::SkillPackageReference;

pub const LIST_SKILLS_REQUEST_SCHEMA_ID: &str = "storyos.query.skills.request.v1";
pub const LIST_SKILLS_RESPONSE_SCHEMA_ID: &str = "storyos.query.skills.response.v1";

pub(super) const LIST_SKILLS: QueryOperation = QueryOperation {
    operation_id: "listSkills",
    method: HttpMethod::Get,
    path: "/api/v1/projects/{project_id}/skills",
    request_schema: LIST_SKILLS_REQUEST_SCHEMA_ID,
    response_schema: LIST_SKILLS_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "The Skills that the deployment offers"),
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
    fixture: "storyos.golden.listSkills.positive.v1",
};

pub const LIST_SKILLS_PATH: &str = LIST_SKILLS.path;
pub const LIST_SKILLS_METHOD: &str = LIST_SKILLS.method.as_str();

/// One Skill of the deployment for the `$` picker: its reference and its description, with no file.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SkillCatalogEntry {
    pub reference: SkillPackageReference,
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ListSkillsResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: ProjectScope,
    // In the order of the Skill names.
    pub skills: Vec<SkillCatalogEntry>,
}
