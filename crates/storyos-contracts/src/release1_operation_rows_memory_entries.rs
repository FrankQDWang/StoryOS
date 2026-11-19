//! The rows of the query that lists the current Memory Entries of a Project and of the command that
//! deletes one entry in the operation table.

use ts_rs::TS;

use super::{
    DATE_TIME, Edit, Fixture, Headers, OperationRow, Patch, SCHEMA_ID, SchemaSource, SchemaSources,
    U64, UUID, raw_schema,
};
use crate::release1_memory_entries::{
    DELETE_MEMORY_ENTRY, DeleteMemoryEntryConflictReason, DeleteMemoryEntryEffect,
    DeleteMemoryEntryInput, DeleteMemoryEntryNoEffectReason, DeleteMemoryEntryRequest,
    DeleteMemoryEntryResponse, LIST_MEMORY_ENTRIES, ListMemoryEntriesResponse,
    MemoryEntryFlagInspect, MemoryEntryInspect, MemoryEntryOriginInspect, MemoryEntrySourceInspect,
    MemoryEntryTypeInspect, MemorySourceAvailability,
};
use crate::release1_operation_registry::RegisteredOperation;
use crate::release1_project_activity::ProjectActivityKind;
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
        MemoryEntryFlagInspect::decl,
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
        &[
            "/$defs/MemoryEntryFlagInspect/properties/flagged_at",
            "/$defs/MemoryEntryInspect/properties/changed_at",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];

pub(super) const DELETE_MEMORY_ENTRY_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &DELETE_MEMORY_ENTRY,
        &[
            "server_derived_project_scope",
            "memory_entry_in_project",
            "expected_memory_entry_version",
            "project_active",
        ],
        OperationCatalog::new("project:memory", Owner::Release, Binding::PROJECT_COMMAND)
            .activity(&[ProjectActivityKind::MemoryEntryDeleted], &[]),
    ),
    summary: "Delete one Memory Entry as one new version with the state deleted",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "delete-memory-entry-request",
            raw_schema::<DeleteMemoryEntryRequest>,
            "StoryOS Delete Memory Entry Request",
            DELETE_MEMORY_ENTRY_REQUEST,
        ),
        SchemaSource::typed(
            "delete-memory-entry-response",
            raw_schema::<DeleteMemoryEntryResponse>,
            "StoryOS Delete Memory Entry Response",
            DELETE_MEMORY_ENTRY_RESPONSE,
        ),
    ),
    typescript: &[
        DeleteMemoryEntryInput::decl,
        DeleteMemoryEntryRequest::decl,
        DeleteMemoryEntryNoEffectReason::decl,
        DeleteMemoryEntryConflictReason::decl,
        DeleteMemoryEntryEffect::decl,
        DeleteMemoryEntryResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000953",
      "command_id": "018f0000-0000-7001-8000-000000000952",
      "correlation_id": "018f0000-0000-7001-8000-000000000950",
      "effect": {
        "kind": "authoritative_applied",
        "memory_entry_id": "018f0000-0000-7001-8000-000000000941",
        "memory_revision": "2",
        "project_activity_position": "3",
        "version": "2"
      },
      "project": {
        "open": {"kind": "empty"},
        "project_id": "018f0000-0000-7001-8000-000000000201",
        "title": "Empty Novel"
      },
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
      },
      "receipt": {
        "artifact_lifecycle_event_refs": [],
        "author_action_sequence": null,
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000953",
        "authoritative_commit_ids": [],
        "authoritative_revision_ids": [],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.deleteMemoryEntry.jcs.v1",
          "value_hex_lowercase": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
        },
        "command_kind": "deleteMemoryEntry",
        "condition_refs": [],
        "created_at": "2026-11-17T03:00:00.000Z",
        "draft_artifact_refs": [],
        "expected_heads": [],
        "idempotency_key": "018f0000-0000-7001-8000-000000000955",
        "prior_heads": [],
        "producer_cause": "author_command_admission",
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "proposal_revision_ids": [],
        "receipt_id": "018f0000-0000-7001-8000-000000000954",
        "result": "authoritative_applied",
        "resulting_heads": []
      },
      "schema_id": "storyos.command.delete-memory-entry.response.v1"
    }"#,
    ),
};

const DELETE_MEMORY_ENTRY_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &["/$defs/DeleteMemoryEntryInput/properties/correlation_id"],
    ),
    Patch(
        U64,
        &["/$defs/DeleteMemoryEntryInput/properties/expected_version"],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const DELETE_MEMORY_ENTRY_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/DeleteMemoryEntryEffect/oneOf/0/properties/memory_entry_id",
            "/$defs/ProjectScope/properties/owner_user_id",
            "/$defs/ProjectScope/properties/project_id",
            "/properties/author_command_admission_id",
            "/properties/command_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(
        U64,
        &[
            "/$defs/DeleteMemoryEntryEffect/oneOf/0/properties/memory_revision",
            "/$defs/DeleteMemoryEntryEffect/oneOf/0/properties/version",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];
