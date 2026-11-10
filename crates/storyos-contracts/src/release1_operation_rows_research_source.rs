//! The rows of the research source queries in the operation table (S5-06).

use ts_rs::TS;

use super::{
    DATE_TIME, Edit, Fixture, Headers, OperationRow, Patch, SCHEMA_ID, SchemaSource, SchemaSources,
    U64, UUID, raw_schema,
};
use crate::release1_wire::U64_WIRE;
use crate::release1_operation_registry::RegisteredOperation;
use crate::release1_research_source::{
    GET_RESEARCH_SOURCE, GetResearchSourceResponse, LIST_RESEARCH_SOURCES,
    ListResearchSourcesResponse, SourceGapInspect, SourceGapReason, SourceSnapshotInspect,
    SourceSnapshotKind,
};
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};

pub(super) const LIST_RESEARCH_SOURCES_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
        &LIST_RESEARCH_SOURCES,
        &["server_derived_project_scope", "run_scope_join"],
        OperationCatalog::new(
            "research-source:list",
            Owner::Context,
            Binding::CANONICAL_QUERY,
        ),
    ),
    summary: "List the Source Snapshots and Source Gaps of one AgentRun, with no captured text",
    headers: Headers::NONE,
    schemas: SchemaSources::get(SchemaSource::typed(
        "research-sources-response",
        raw_schema::<ListResearchSourcesResponse>,
        "StoryOS Research Sources Response",
        LIST_RESEARCH_SOURCES_RESPONSE,
    )),
    typescript: &[
        SourceSnapshotKind::decl,
        SourceGapReason::decl,
        SourceSnapshotInspect::decl,
        SourceGapInspect::decl,
        ListResearchSourcesResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "correlation_id": "018f0000-0000-7001-8000-000000000d01",
      "gaps": [
        {
          "locator": "https://example.org/port",
          "reason": "failed",
          "recorded_at": "2026-11-10T03:00:01.000Z",
          "result_bytes": "45",
          "result_offset": "0",
          "source_gap_id": "018f0000-0000-7001-8000-000000000d05",
          "tool_call_id": "018f0000-0000-7001-8000-000000000d03"
        }
      ],
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
      },
      "run_id": "018f0000-0000-7001-8000-000000000a34",
      "schema_id": "storyos.query.research-sources.response.v1",
      "snapshots": [
        {
          "captured_at": "2026-11-10T03:00:00.000Z",
          "content_digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
          "kind": "search_result",
          "locator": "https://example.org/tides",
          "published": "1890-01-01T00:00:00.000Z",
          "result_offset": "88",
          "source_snapshot_id": "018f0000-0000-7001-8000-000000000d04",
          "text_bytes": "20",
          "title": "Liverpool tide table 1890",
          "tool_call_id": "018f0000-0000-7001-8000-000000000d02"
        }
      ]
    }"#,
    ),
};

pub(super) const GET_RESEARCH_SOURCE_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
        &GET_RESEARCH_SOURCE,
        &["server_derived_project_scope", "research_source_scope_join"],
        OperationCatalog::new(
            "research-source:read",
            Owner::Context,
            Binding::CANONICAL_QUERY,
        ),
    ),
    summary: "Read one Source Snapshot revision with its captured text",
    headers: Headers::NONE,
    schemas: SchemaSources::get(SchemaSource::typed(
        "research-source-response",
        raw_schema::<GetResearchSourceResponse>,
        "StoryOS Research Source Response",
        GET_RESEARCH_SOURCE_RESPONSE,
    )),
    typescript: &[GetResearchSourceResponse::decl],
    fixture: Fixture::Json(
        r#"{
      "correlation_id": "018f0000-0000-7001-8000-000000000d06",
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
      },
      "schema_id": "storyos.query.research-source.response.v1",
      "snapshot": {
        "captured_at": "2026-11-10T03:00:00.000Z",
        "content_digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "kind": "page",
        "locator": "https://example.org/tides",
        "published": null,
        "result_offset": "41",
        "source_snapshot_id": "018f0000-0000-7001-8000-000000000d07",
        "text_bytes": "20",
        "title": null,
        "tool_call_id": "018f0000-0000-7001-8000-000000000d08"
      },
      "text": "High water at 10.42."
    }"#,
    ),
};

const DIGEST: Edit = Edit::Pattern("^sha256:[0-9a-f]{64}$");

const LIST_RESEARCH_SOURCES_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/ProjectScope/properties/owner_user_id",
            "/$defs/ProjectScope/properties/project_id",
            "/$defs/SourceGapInspect/properties/source_gap_id",
            "/$defs/SourceGapInspect/properties/tool_call_id",
            "/$defs/SourceSnapshotInspect/properties/source_snapshot_id",
            "/$defs/SourceSnapshotInspect/properties/tool_call_id",
            "/properties/correlation_id",
            "/properties/run_id",
        ],
    ),
    Patch(
        DATE_TIME,
        &[
            "/$defs/SourceGapInspect/properties/recorded_at",
            "/$defs/SourceSnapshotInspect/properties/captured_at",
        ],
    ),
    Patch(
        U64,
        &[
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
        DIGEST,
        &["/$defs/SourceSnapshotInspect/properties/content_digest"],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];

const GET_RESEARCH_SOURCE_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/ProjectScope/properties/owner_user_id",
            "/$defs/ProjectScope/properties/project_id",
            "/$defs/SourceSnapshotInspect/properties/source_snapshot_id",
            "/$defs/SourceSnapshotInspect/properties/tool_call_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(
        DATE_TIME,
        &["/$defs/SourceSnapshotInspect/properties/captured_at"],
    ),
    Patch(
        U64,
        &[
            "/$defs/SourceSnapshotInspect/properties/result_offset",
            "/$defs/SourceSnapshotInspect/properties/text_bytes",
        ],
    ),
    Patch(
        DIGEST,
        &["/$defs/SourceSnapshotInspect/properties/content_digest"],
    ),
    Patch(Edit::MaxUtf8Bytes(262_144), &["/properties/text"]),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];
