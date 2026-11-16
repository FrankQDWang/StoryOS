//! The row of the query that lists the current Memory Entries of a Project in the operation table.

use ts_rs::TS;

use super::{
    DATE_TIME, Edit, Fixture, Headers, OperationRow, Patch, SCHEMA_ID, SchemaSource, SchemaSources,
    U64, UUID, raw_schema,
};
use crate::release1_memory_entries::{
    LIST_MEMORY_ENTRIES, ListMemoryEntriesResponse, MemoryEntryInspect, MemoryEntryOriginInspect,
    MemoryEntrySourceInspect, MemoryEntryTypeInspect, MemorySourceAvailability,
};
use crate::release1_operation_registry::RegisteredOperation;
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};

pub(super) const LIST_MEMORY_ENTRIES_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
        &LIST_MEMORY_ENTRIES,
        &["server_derived_project_scope", "project_visibility"],
        OperationCatalog::new("project:memory", Owner::Release, Binding::PROJECT_QUERY),
    ),
    summary: "List the current Memory Entries of a Project in the order of the Memory Index",
    headers: Headers::NONE,
    schemas: SchemaSources::get(SchemaSource::typed(
        "memory-entries-response",
        raw_schema::<ListMemoryEntriesResponse>,
        "StoryOS Memory Entries Response",
        LIST_MEMORY_ENTRIES_RESPONSE,
    )),
    typescript: &[
        MemoryEntryTypeInspect::decl,
        MemoryEntryOriginInspect::decl,
        MemorySourceAvailability::decl,
        MemoryEntrySourceInspect::decl,
        MemoryEntryInspect::decl,
        ListMemoryEntriesResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "correlation_id": "018f0000-0000-7001-8000-000000000940",
      "entries": [
        {
          "changed_at": "2026-11-16T03:00:00.000Z",
          "description": "The ferry worker, six years older than Lin Zhou.",
          "entry_number": "1",
          "entry_type": "character",
          "memory_entry_id": "018f0000-0000-7001-8000-000000000941",
          "origin": "author_request",
          "source": {
            "availability": "available",
            "conversation_id": "018f0000-0000-7001-8000-000000000942",
            "kind": "conversation"
          },
          "text": "Lin Xue keeps a brass compass.",
          "title": "Lin Xue",
          "version": "1"
        }
      ],
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
      },
      "schema_id": "storyos.query.memory-entries.response.v1"
    }"#,
    ),
};

const LIST_MEMORY_ENTRIES_RESPONSE: &[Patch] = &[
    Patch(
        Edit::NullableDefinition("MemoryEntrySourceInspect"),
        &["/$defs/MemoryEntryInspect/properties/source"],
    ),
    Patch(
        UUID,
        &[
            "/$defs/MemoryEntryInspect/properties/memory_entry_id",
            "/$defs/MemoryEntrySourceInspect/oneOf/0/properties/conversation_id",
            "/$defs/MemoryEntrySourceInspect/oneOf/1/properties/chapter_id",
            "/$defs/ProjectScope/properties/owner_user_id",
            "/$defs/ProjectScope/properties/project_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(
        U64,
        &[
            "/$defs/MemoryEntryInspect/properties/entry_number",
            "/$defs/MemoryEntryInspect/properties/version",
        ],
    ),
    Patch(
        DATE_TIME,
        &["/$defs/MemoryEntryInspect/properties/changed_at"],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];
