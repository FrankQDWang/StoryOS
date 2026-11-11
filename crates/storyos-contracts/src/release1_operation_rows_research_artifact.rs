//! The row of the Research Artifact query in the operation table (S5-07).

use ts_rs::TS;

use super::{
    DATE_TIME, Edit, Fixture, Headers, OperationRow, Patch, SCHEMA_ID, SchemaSource, SchemaSources,
    U64, UUID, raw_schema,
};
use crate::release1_operation_registry::RegisteredOperation;
use crate::release1_research_artifact::{
    GET_RESEARCH_ARTIFACT, GetResearchArtifactResponse, ResearchArtifactInspect,
    ResearchArtifactLineage, ResearchCitationInspect, ResearchClaimInspect,
};
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};
use crate::release1_wire::U64_WIRE;

pub(super) const GET_RESEARCH_ARTIFACT_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
        &GET_RESEARCH_ARTIFACT,
        &[
            "server_derived_project_scope",
            "research_artifact_scope_join",
        ],
        OperationCatalog::new(
            "research-artifact:read",
            Owner::Context,
            Binding::CANONICAL_QUERY,
        ),
    ),
    summary: "Read one Research Artifact with its claims and its citations resolved to locators",
    headers: Headers::NONE,
    schemas: SchemaSources::get(SchemaSource::typed(
        "research-artifact-response",
        raw_schema::<GetResearchArtifactResponse>,
        "StoryOS Research Artifact Response",
        GET_RESEARCH_ARTIFACT_RESPONSE,
    )),
    typescript: &[
        ResearchArtifactLineage::decl,
        ResearchClaimInspect::decl,
        ResearchCitationInspect::decl,
        ResearchArtifactInspect::decl,
        GetResearchArtifactResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "artifact": {
        "citations": [
          {
            "kind": "snapshot",
            "snapshot": {
              "captured_at": "2026-11-11T03:00:00.000Z",
              "content_digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
              "kind": "page",
              "locator": "https://example.org/tides",
              "published": null,
              "result_offset": "41",
              "source_snapshot_id": "018f0000-0000-7001-8000-000000000e04",
              "text_bytes": "20",
              "title": "Liverpool tide table 1890",
              "tool_call_id": "018f0000-0000-7001-8000-000000000e03"
            }
          },
          {
            "gap": {
              "locator": "https://example.org/port",
              "reason": "failed",
              "recorded_at": "2026-11-11T03:00:00.000Z",
              "result_bytes": "45",
              "result_offset": "62",
              "source_gap_id": "018f0000-0000-7001-8000-000000000e05",
              "tool_call_id": "018f0000-0000-7001-8000-000000000e03"
            },
            "kind": "gap"
          }
        ],
        "claims": [
          {"citations": [1], "text": "High water at Liverpool was at 10.42."},
          {"citations": [2], "text": "The port log could not be read."}
        ],
        "content_digest": "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "created_at": "2026-11-11T03:00:02.000Z",
        "lineage": {
          "decision_id": "018f0000-0000-7001-8000-000000000e08",
          "decision_position": "1",
          "model_attempt_id": "018f0000-0000-7001-8000-000000000e07",
          "operation_requirement_id": "018f0000-0000-7001-8000-000000000e06",
          "run_id": "018f0000-0000-7001-8000-000000000e02",
          "tool_call_ids": ["018f0000-0000-7001-8000-000000000e03"]
        },
        "research_artifact_id": "018f0000-0000-7001-8000-000000000e01",
        "summary": "High water was at 10.42. The port log is not available."
      },
      "correlation_id": "018f0000-0000-7001-8000-000000000e00",
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
      },
      "schema_id": "storyos.query.research-artifact.response.v1"
    }"#,
    ),
};

const GET_RESEARCH_ARTIFACT_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/ProjectScope/properties/owner_user_id",
            "/$defs/ProjectScope/properties/project_id",
            "/$defs/ResearchArtifactInspect/properties/research_artifact_id",
            "/$defs/ResearchArtifactLineage/properties/decision_id",
            "/$defs/ResearchArtifactLineage/properties/model_attempt_id",
            "/$defs/ResearchArtifactLineage/properties/operation_requirement_id",
            "/$defs/ResearchArtifactLineage/properties/run_id",
            "/$defs/ResearchArtifactLineage/properties/tool_call_ids/items",
            "/$defs/SourceGapInspect/properties/source_gap_id",
            "/$defs/SourceGapInspect/properties/tool_call_id",
            "/$defs/SourceSnapshotInspect/properties/source_snapshot_id",
            "/$defs/SourceSnapshotInspect/properties/tool_call_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(
        DATE_TIME,
        &[
            "/$defs/ResearchArtifactInspect/properties/created_at",
            "/$defs/SourceGapInspect/properties/recorded_at",
            "/$defs/SourceSnapshotInspect/properties/captured_at",
        ],
    ),
    Patch(
        U64,
        &[
            "/$defs/ResearchArtifactLineage/properties/decision_position",
            "/$defs/SourceSnapshotInspect/properties/result_offset",
            "/$defs/SourceSnapshotInspect/properties/text_bytes",
        ],
    ),
    Patch(
        Edit::NullablePattern(U64_WIRE),
        &[
            "/$defs/SourceGapInspect/properties/result_bytes",
            "/$defs/SourceGapInspect/properties/result_offset",
        ],
    ),
    Patch(
        Edit::Pattern("^sha256:[0-9a-f]{64}$"),
        &[
            "/$defs/ResearchArtifactInspect/properties/content_digest",
            "/$defs/SourceSnapshotInspect/properties/content_digest",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];
