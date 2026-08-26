use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ProjectScope, QueryOperation};
use crate::release1_snapshot::SnapshotDescriptor;

pub const GET_MANUSCRIPT_TREE_REQUEST_SCHEMA_ID: &str = "storyos.query.manuscript-tree.request.v1";
pub const GET_MANUSCRIPT_TREE_RESPONSE_SCHEMA_ID: &str =
    "storyos.query.manuscript-tree.response.v1";

pub(super) const GET_MANUSCRIPT_TREE: QueryOperation = QueryOperation {
    operation_id: "getManuscriptTree",
    method: "GET",
    path: "/api/v1/projects/{project_id}/manuscript/tree",
    request_schema: GET_MANUSCRIPT_TREE_REQUEST_SCHEMA_ID,
    response_schema: GET_MANUSCRIPT_TREE_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Ordered canonical manuscript tree"),
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
        "storyos.golden.getManuscriptTree.positive.v1",
        "storyos.golden.getManuscriptTree.invalid.v1",
        "storyos.golden.getManuscriptTree.boundary.v1",
    ],
};

pub const GET_MANUSCRIPT_TREE_PATH: &str = GET_MANUSCRIPT_TREE.path;
pub const GET_MANUSCRIPT_TREE_METHOD: &str = GET_MANUSCRIPT_TREE.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ManuscriptChapterNode {
    pub chapter_id: String,
    pub title: String,
    pub order: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ManuscriptVolumeNode {
    pub volume_id: String,
    pub title: String,
    pub order: String,
    pub chapters: Vec<ManuscriptChapterNode>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GetManuscriptTreeResponse {
    pub schema_id: String,
    pub correlation_id: String,
    #[ts(type = "ProjectScope")]
    pub project_scope: ProjectScope,
    pub tree_revision: String,
    #[ts(type = "SnapshotDescriptor")]
    pub snapshot: SnapshotDescriptor,
    pub volumes: Vec<ManuscriptVolumeNode>,
}
