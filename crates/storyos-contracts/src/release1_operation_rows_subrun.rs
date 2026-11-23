//! The row of the query of one Subrun in the operation table (ADR 0103 section 3A).

use ts_rs::TS;

use super::{
    Edit, Fixture, Headers, OperationRow, Patch, SCHEMA_ID, SchemaSource, SchemaSources, UUID,
    raw_schema,
};
use crate::release1_operation_registry::RegisteredOperation;
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};
use crate::release1_subrun::{GET_SUBRUN, GetSubrunResponse, SubrunStepInspect};

pub(super) const GET_SUBRUN_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
        &GET_SUBRUN,
        &["server_derived_project_scope", "subrun_request_scope_join"],
        OperationCatalog::new("subrun:read", Owner::Core, Binding::CANONICAL_QUERY),
    ),
    summary: "Read the record of one Subrun of a Run of the author",
    headers: Headers::NONE,
    schemas: SchemaSources::get(SchemaSource::typed(
        "subrun-response",
        raw_schema::<GetSubrunResponse>,
        "StoryOS Subrun Response",
        GET_SUBRUN_RESPONSE,
    )),
    typescript: &[SubrunStepInspect::decl, GetSubrunResponse::decl],
    fixture: Fixture::Json(
        r#"{
      "answer": "第一章有灯。",
      "brief": "读第一章，说出灯在哪里。",
      "correlation_id": "018f0000-0000-7001-8000-000000000e01",
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
      },
      "schema_id": "storyos.query.subrun.response.v1",
      "state": "completed",
      "step_count": 2,
      "steps": [
        {
          "kind": "text",
          "text": "先读第一章。"
        },
        {
          "chapter_id": "018f0000-0000-7001-8000-000000000301",
          "kind": "chapter_read",
          "title": "渡口"
        }
      ],
      "subrun_id": "018f0000-0000-7001-8000-000000000e02",
      "title": "读第一章"
    }"#,
    ),
};

const GET_SUBRUN_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/ProjectScope/properties/owner_user_id",
            "/$defs/ProjectScope/properties/project_id",
            "/$defs/SubrunStepInspect/oneOf/1/properties/chapter_id",
            "/properties/correlation_id",
            "/properties/subrun_id",
        ],
    ),
    Patch(
        Edit::MinLength(1),
        &["/properties/title", "/properties/brief"],
    ),
    Patch(Edit::MaxLength(40), &["/properties/title"]),
    Patch(Edit::MaxLength(4000), &["/properties/brief"]),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];
