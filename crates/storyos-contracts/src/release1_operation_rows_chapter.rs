//! The rows of the Chapter commands in the operation table.

use ts_rs::TS;

use super::{
    Edit, Fixture, Headers, OperationRow, Patch, SCHEMA_ID, SchemaSource, SchemaSources, U64, UUID,
    raw_schema,
};
use crate::release1_create_chapter::{
    CREATE_CHAPTER, CreateChapterConflictReason, CreateChapterEffect, CreateChapterInput,
    CreateChapterPlacement, CreateChapterRefusalReason, CreateChapterRequest,
    CreateChapterResponse,
};
use crate::release1_delete_chapter::{
    DELETE_CHAPTER, DeleteChapterConflictReason, DeleteChapterEffect, DeleteChapterInput,
    DeleteChapterNoEffectReason, DeleteChapterRefusalReason, DeleteChapterRequest,
    DeleteChapterResponse,
};
use crate::release1_operation_registry::RegisteredOperation;
use crate::release1_project_activity::ProjectActivityKind;
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};
use crate::release1_update_chapter::{
    UPDATE_CHAPTER, UpdateChapterConflictReason, UpdateChapterEffect, UpdateChapterInput,
    UpdateChapterNoEffectReason, UpdateChapterRefusalReason, UpdateChapterRequest,
    UpdateChapterResponse,
};
use crate::release1_wire::U64_WIRE;

pub(super) const CREATE_CHAPTER_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &CREATE_CHAPTER,
        &[
            "server_derived_project_scope",
            "volume_scope_join",
            "project_active",
            "expected_tree_revision",
        ],
        OperationCatalog::new("chapter:create", Owner::Release, Binding::PROJECT_COMMAND)
            .activity(&[ProjectActivityKind::ChapterCreated], &[]),
    ),
    summary: "Create one named Chapter",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "create-chapter-request",
            raw_schema::<CreateChapterRequest>,
            "StoryOS Create Chapter Request",
            CREATE_CHAPTER_REQUEST,
        ),
        SchemaSource::typed(
            "create-chapter-response",
            raw_schema::<CreateChapterResponse>,
            "StoryOS Create Chapter Response",
            CREATE_CHAPTER_RESPONSE,
        ),
    ),
    typescript: &[
        CreateChapterPlacement::decl,
        CreateChapterInput::decl,
        CreateChapterRequest::decl,
        CreateChapterConflictReason::decl,
        CreateChapterRefusalReason::decl,
        CreateChapterEffect::decl,
        CreateChapterResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000912",
      "command_id": "018f0000-0000-7001-8000-000000000911",
      "correlation_id": "018f0000-0000-7001-8000-000000000910",
      "effect": {
        "chapter_id": "018f0000-0000-7001-8000-000000000916",
        "current_chapter_id": "018f0000-0000-7001-8000-000000000916",
        "kind": "authoritative_applied",
        "order": "1",
        "project_activity_position": "3",
        "title": "Chapter A",
        "tree_revision": "3",
        "volume_id": "018f0000-0000-7001-8000-000000000815"
      },
      "project": {
        "open": {
          "current_chapter_id": "018f0000-0000-7001-8000-000000000916",
          "kind": "current_chapter"
        },
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
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000912",
        "authoritative_commit_ids": [],
        "authoritative_revision_ids": [],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.createChapter.jcs.v1",
          "value_hex_lowercase": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
        },
        "command_kind": "createChapter",
        "condition_refs": [],
        "created_at": "2026-08-27T12:00:00.000Z",
        "draft_artifact_refs": [],
        "expected_heads": [],
        "idempotency_key": "018f0000-0000-7001-8000-000000000914",
        "prior_heads": [],
        "producer_cause": "author_command_admission",
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "proposal_revision_ids": [],
        "receipt_id": "018f0000-0000-7001-8000-000000000913",
        "result": "authoritative_applied",
        "resulting_heads": []
      },
      "schema_id": "storyos.command.create-chapter.response.v1"
    }"#,
    ),
};

pub(super) const UPDATE_CHAPTER_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &UPDATE_CHAPTER,
        &[
            "server_derived_project_scope",
            "chapter_scope_join",
            "expected_tree_revision",
        ],
        OperationCatalog::new("chapter:update", Owner::Release, Binding::PROJECT_COMMAND)
            .activity(&[ProjectActivityKind::ChapterUpdated], &[])
            .action_coverage(&["rename", "reorder"]),
    ),
    summary: "Rename or reorder one named Chapter",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "update-chapter-request",
            raw_schema::<UpdateChapterRequest>,
            "StoryOS Update Chapter Request",
            UPDATE_CHAPTER_REQUEST,
        ),
        SchemaSource::typed(
            "update-chapter-response",
            raw_schema::<UpdateChapterResponse>,
            "StoryOS Update Chapter Response",
            UPDATE_CHAPTER_RESPONSE,
        ),
    ),
    typescript: &[
        UpdateChapterInput::decl,
        UpdateChapterRequest::decl,
        UpdateChapterNoEffectReason::decl,
        UpdateChapterConflictReason::decl,
        UpdateChapterRefusalReason::decl,
        UpdateChapterEffect::decl,
        UpdateChapterResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000a12",
      "command_id": "018f0000-0000-7001-8000-000000000a11",
      "correlation_id": "018f0000-0000-7001-8000-000000000a10",
      "effect": {
        "chapter_id": "018f0000-0000-7001-8000-000000000815",
        "kind": "authoritative_applied",
        "order": "2",
        "project_activity_position": "4",
        "title": "Chapter B",
        "tree_revision": "5"
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
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000a12",
        "authoritative_commit_ids": [],
        "authoritative_revision_ids": [],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.updateChapter.jcs.v1",
          "value_hex_lowercase": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
        },
        "command_kind": "updateChapter",
        "condition_refs": [],
        "created_at": "2026-08-27T12:00:00.000Z",
        "draft_artifact_refs": [],
        "expected_heads": [],
        "idempotency_key": "018f0000-0000-7001-8000-000000000a14",
        "prior_heads": [],
        "producer_cause": "author_command_admission",
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "proposal_revision_ids": [],
        "receipt_id": "018f0000-0000-7001-8000-000000000a13",
        "result": "authoritative_applied",
        "resulting_heads": []
      },
      "schema_id": "storyos.command.update-chapter.response.v1"
    }"#,
    ),
};

pub(super) const DELETE_CHAPTER_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &DELETE_CHAPTER,
        &[
            "server_derived_project_scope",
            "chapter_scope_join",
            "expected_tree_revision",
        ],
        OperationCatalog::new("chapter:delete", Owner::Release, Binding::PROJECT_COMMAND)
            .activity(&[ProjectActivityKind::ChapterDeleted], &[]),
    ),
    summary: "Remove one named Chapter",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "delete-chapter-request",
            raw_schema::<DeleteChapterRequest>,
            "StoryOS Delete Chapter Request",
            DELETE_CHAPTER_REQUEST,
        ),
        SchemaSource::typed(
            "delete-chapter-response",
            raw_schema::<DeleteChapterResponse>,
            "StoryOS Delete Chapter Response",
            DELETE_CHAPTER_RESPONSE,
        ),
    ),
    typescript: &[
        DeleteChapterInput::decl,
        DeleteChapterRequest::decl,
        DeleteChapterNoEffectReason::decl,
        DeleteChapterConflictReason::decl,
        DeleteChapterRefusalReason::decl,
        DeleteChapterEffect::decl,
        DeleteChapterResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000b12",
      "command_id": "018f0000-0000-7001-8000-000000000b11",
      "correlation_id": "018f0000-0000-7001-8000-000000000b10",
      "effect": {
        "chapter_id": "018f0000-0000-7001-8000-000000000917",
        "kind": "authoritative_applied",
        "project_activity_position": "5",
        "tree_revision": "5",
        "volume_id": "018f0000-0000-7001-8000-000000000815"
      },
      "project": {
        "open": {
          "current_chapter_id": "018f0000-0000-7001-8000-000000000916",
          "kind": "current_chapter"
        },
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
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000b12",
        "authoritative_commit_ids": [],
        "authoritative_revision_ids": [],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.deleteChapter.jcs.v1",
          "value_hex_lowercase": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
        },
        "command_kind": "deleteChapter",
        "condition_refs": [],
        "created_at": "2026-08-27T12:00:00.000Z",
        "draft_artifact_refs": [],
        "expected_heads": [],
        "idempotency_key": "018f0000-0000-7001-8000-000000000b14",
        "prior_heads": [],
        "producer_cause": "author_command_admission",
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "proposal_revision_ids": [],
        "receipt_id": "018f0000-0000-7001-8000-000000000b13",
        "result": "authoritative_applied",
        "resulting_heads": []
      },
      "schema_id": "storyos.command.delete-chapter.response.v1"
    }"#,
    ),
};

const CREATE_CHAPTER_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/CreateChapterInput/properties/correlation_id",
            "/$defs/CreateChapterPlacement/oneOf/0/properties/chapter_id",
            "/$defs/CreateChapterPlacement/oneOf/1/properties/chapter_id",
        ],
    ),
    Patch(
        U64,
        &["/$defs/CreateChapterInput/properties/expected_tree_revision"],
    ),
    Patch(
        Edit::NotNull,
        &["/$defs/CreateChapterInput/properties/placement"],
    ),
    Patch(
        Edit::MinLength(1),
        &["/$defs/CreateChapterInput/properties/title"],
    ),
    Patch(
        Edit::MaxUtf8Bytes(1024),
        &["/$defs/CreateChapterInput/properties/title"],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const CREATE_CHAPTER_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/properties/author_command_admission_id",
            "/properties/command_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
    Patch(
        Edit::NullablePattern(U64_WIRE),
        &["/$defs/DomainReceipt/properties/author_action_sequence"],
    ),
];

const UPDATE_CHAPTER_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &["/$defs/UpdateChapterInput/properties/correlation_id"],
    ),
    Patch(
        U64,
        &[
            "/$defs/UpdateChapterInput/properties/expected_tree_revision",
            "/$defs/UpdateChapterInput/properties/order",
        ],
    ),
    Patch(
        Edit::MinLength(1),
        &["/$defs/UpdateChapterInput/properties/title"],
    ),
    Patch(
        Edit::MaxUtf8Bytes(1024),
        &["/$defs/UpdateChapterInput/properties/title"],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const UPDATE_CHAPTER_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/properties/author_command_admission_id",
            "/properties/command_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
    Patch(
        Edit::NullablePattern(U64_WIRE),
        &["/$defs/DomainReceipt/properties/author_action_sequence"],
    ),
];

const DELETE_CHAPTER_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &["/$defs/DeleteChapterInput/properties/correlation_id"],
    ),
    Patch(
        U64,
        &["/$defs/DeleteChapterInput/properties/expected_tree_revision"],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const DELETE_CHAPTER_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/properties/author_command_admission_id",
            "/properties/command_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
    Patch(
        Edit::NullablePattern(U64_WIRE),
        &["/$defs/DomainReceipt/properties/author_action_sequence"],
    ),
];
