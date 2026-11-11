use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{HttpMethod, ProjectScope, QueryOperation};
use crate::{SourceGapInspect, SourceSnapshotInspect};

pub const GET_RESEARCH_ARTIFACT_REQUEST_SCHEMA_ID: &str =
    "storyos.query.research-artifact.request.v1";
pub const GET_RESEARCH_ARTIFACT_RESPONSE_SCHEMA_ID: &str =
    "storyos.query.research-artifact.response.v1";

pub(super) const GET_RESEARCH_ARTIFACT: QueryOperation = QueryOperation {
    operation_id: "getResearchArtifact",
    method: HttpMethod::Get,
    path: "/api/v1/projects/{project_id}/research-artifacts/{research_artifact_id}",
    request_schema: GET_RESEARCH_ARTIFACT_REQUEST_SCHEMA_ID,
    response_schema: GET_RESEARCH_ARTIFACT_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "One Research Artifact with its citations"),
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
    fixture: "storyos.golden.getResearchArtifact.positive.v1",
};

pub const GET_RESEARCH_ARTIFACT_PATH: &str = GET_RESEARCH_ARTIFACT.path;
pub const GET_RESEARCH_ARTIFACT_METHOD: &str = GET_RESEARCH_ARTIFACT.method.as_str();

/// The producer of a Research Artifact: one RunStep of one AgentRun.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ResearchArtifactLineage {
    pub run_id: String,
    pub decision_position: String,
    pub operation_requirement_id: String,
    pub model_attempt_id: String,
    pub decision_id: String,
    // The ToolCalls whose results the RunStep read, in item order.
    pub tool_call_ids: Vec<String>,
}

/// One claim of the synthesis with the numbers of its citations.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ResearchClaimInspect {
    pub text: String,
    // Each number is the position, from 1, of one entry of `citations` of the Artifact.
    pub citations: Vec<u32>,
}

/// One cited source, resolved to its locator: a Source Snapshot revision or a Source Gap.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResearchCitationInspect {
    Snapshot { snapshot: SourceSnapshotInspect },
    Gap { gap: SourceGapInspect },
}

/// One immutable Research Artifact: the synthesis of the model with its claims. It has no support,
/// conflict, or quality classification, because the Host computes none.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ResearchArtifactInspect {
    pub research_artifact_id: String,
    pub created_at: String,
    pub content_digest: String,
    pub lineage: ResearchArtifactLineage,
    pub summary: String,
    pub claims: Vec<ResearchClaimInspect>,
    // Each cited source one time, in the order of its first citation.
    pub citations: Vec<ResearchCitationInspect>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GetResearchArtifactResponse {
    pub schema_id: String,
    pub correlation_id: String,
    #[ts(type = "ProjectScope")]
    pub project_scope: ProjectScope,
    pub artifact: ResearchArtifactInspect,
}
