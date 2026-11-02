//! The rows of the Volume commands in the operation table.

use ts_rs::TS;

use super::{
    Edit, Fixture, Headers, OperationRow, Patch, SCHEMA_ID, SchemaSource, SchemaSources, U64, UUID,
    raw_schema,
};
use crate::ProjectActivityKind;
use crate::release1_create_volume::{
    CREATE_VOLUME, CreateVolumeConflictReason, CreateVolumeEffect, CreateVolumeInput,
    CreateVolumeRefusalReason, CreateVolumeRequest, CreateVolumeResponse,
};
use crate::release1_delete_volume::{
    DELETE_VOLUME, DeleteVolumeConflictReason, DeleteVolumeEffect, DeleteVolumeInput,
    DeleteVolumeNoEffectReason, DeleteVolumeRefusalReason, DeleteVolumeRequest,
    DeleteVolumeResponse,
};
use crate::release1_operation_registry::RegisteredOperation;
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};
use crate::release1_update_volume::{
    UPDATE_VOLUME, UpdateVolumeConflictReason, UpdateVolumeEffect, UpdateVolumeInput,
    UpdateVolumeNoEffectReason, UpdateVolumeRefusalReason, UpdateVolumeRequest,
    UpdateVolumeResponse,
};

pub(super) const CREATE_VOLUME_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &CREATE_VOLUME,
        &[
            "server_derived_project_scope",
            "project_active",
            "expected_tree_revision",
        ],
        OperationCatalog::new("volume:create", Owner::Release, Binding::PROJECT_COMMAND)
            .activity(&[ProjectActivityKind::VolumeCreated], &[]),
    ),
    summary: "Create one named Volume",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "create-volume-request",
            raw_schema::<CreateVolumeRequest>,
            "StoryOS Create Volume Request",
            CREATE_VOLUME_REQUEST,
        ),
        SchemaSource::typed(
            "create-volume-response",
            raw_schema::<CreateVolumeResponse>,
            "StoryOS Create Volume Response",
            CREATE_VOLUME_RESPONSE,
        ),
    ),
    typescript: &[
        CreateVolumeInput::decl,
        CreateVolumeRequest::decl,
        CreateVolumeConflictReason::decl,
        CreateVolumeRefusalReason::decl,
        CreateVolumeEffect::decl,
        CreateVolumeResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000812",
      "command_id": "018f0000-0000-7001-8000-000000000811",
      "correlation_id": "018f0000-0000-7001-8000-000000000810",
      "effect": {
        "kind": "authoritative_applied",
        "order": "1",
        "project_activity_position": "2",
        "title": "Volume A",
        "tree_revision": "2",
        "volume_id": "018f0000-0000-7001-8000-000000000815"
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
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000812",
        "authoritative_commit_ids": [],
        "authoritative_revision_ids": [],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.createVolume.jcs.v1",
          "value_hex_lowercase": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
        },
        "command_kind": "createVolume",
        "condition_refs": [],
        "created_at": "2026-08-26T12:00:00.000Z",
        "draft_artifact_refs": [],
        "expected_heads": [],
        "idempotency_key": "018f0000-0000-7001-8000-000000000814",
        "prior_heads": [],
        "producer_cause": "author_command_admission",
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "proposal_revision_ids": [],
        "receipt_id": "018f0000-0000-7001-8000-000000000813",
        "result": "authoritative_applied",
        "resulting_heads": []
      },
      "schema_id": "storyos.command.create-volume.response.v1"
    }"#,
    ),
};

pub(super) const UPDATE_VOLUME_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &UPDATE_VOLUME,
        &[
            "server_derived_project_scope",
            "volume_scope_join",
            "expected_tree_revision",
        ],
        OperationCatalog::new("volume:update", Owner::Release, Binding::PROJECT_COMMAND)
            .activity(&[ProjectActivityKind::VolumeUpdated], &[])
            .action_coverage(&["rename", "reorder"]),
    ),
    summary: "Rename or reorder one named Volume",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "update-volume-request",
            raw_schema::<UpdateVolumeRequest>,
            "StoryOS Update Volume Request",
            UPDATE_VOLUME_REQUEST,
        ),
        SchemaSource::typed(
            "update-volume-response",
            raw_schema::<UpdateVolumeResponse>,
            "StoryOS Update Volume Response",
            UPDATE_VOLUME_RESPONSE,
        ),
    ),
    typescript: &[
        UpdateVolumeInput::decl,
        UpdateVolumeRequest::decl,
        UpdateVolumeNoEffectReason::decl,
        UpdateVolumeConflictReason::decl,
        UpdateVolumeRefusalReason::decl,
        UpdateVolumeEffect::decl,
        UpdateVolumeResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000a12",
      "command_id": "018f0000-0000-7001-8000-000000000a11",
      "correlation_id": "018f0000-0000-7001-8000-000000000a10",
      "effect": {
        "kind": "authoritative_applied",
        "order": "2",
        "project_activity_position": "4",
        "title": "Volume B",
        "tree_revision": "4",
        "volume_id": "018f0000-0000-7001-8000-000000000815"
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
          "profile": "storyos.command.updateVolume.jcs.v1",
          "value_hex_lowercase": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
        },
        "command_kind": "updateVolume",
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
      "schema_id": "storyos.command.update-volume.response.v1"
    }"#,
    ),
};

pub(super) const DELETE_VOLUME_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &DELETE_VOLUME,
        &[
            "server_derived_project_scope",
            "volume_scope_join",
            "expected_tree_revision",
            "child_chapter_policy",
        ],
        OperationCatalog::new("volume:delete", Owner::Release, Binding::PROJECT_COMMAND)
            .activity(&[ProjectActivityKind::VolumeDeleted], &[]),
    ),
    summary: "Remove one named empty Volume",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "delete-volume-request",
            raw_schema::<DeleteVolumeRequest>,
            "StoryOS Delete Volume Request",
            DELETE_VOLUME_REQUEST,
        ),
        SchemaSource::typed(
            "delete-volume-response",
            raw_schema::<DeleteVolumeResponse>,
            "StoryOS Delete Volume Response",
            DELETE_VOLUME_RESPONSE,
        ),
    ),
    typescript: &[
        DeleteVolumeInput::decl,
        DeleteVolumeRequest::decl,
        DeleteVolumeNoEffectReason::decl,
        DeleteVolumeConflictReason::decl,
        DeleteVolumeRefusalReason::decl,
        DeleteVolumeEffect::decl,
        DeleteVolumeResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000c12",
      "command_id": "018f0000-0000-7001-8000-000000000c11",
      "correlation_id": "018f0000-0000-7001-8000-000000000c10",
      "effect": {
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
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000c12",
        "authoritative_commit_ids": [],
        "authoritative_revision_ids": [],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.deleteVolume.jcs.v1",
          "value_hex_lowercase": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
        },
        "command_kind": "deleteVolume",
        "condition_refs": [],
        "created_at": "2026-08-27T12:00:00.000Z",
        "draft_artifact_refs": [],
        "expected_heads": [],
        "idempotency_key": "018f0000-0000-7001-8000-000000000c14",
        "prior_heads": [],
        "producer_cause": "author_command_admission",
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "proposal_revision_ids": [],
        "receipt_id": "018f0000-0000-7001-8000-000000000c13",
        "result": "authoritative_applied",
        "resulting_heads": []
      },
      "schema_id": "storyos.command.delete-volume.response.v1"
    }"#,
    ),
};

const CREATE_VOLUME_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &["/$defs/CreateVolumeInput/properties/correlation_id"],
    ),
    Patch(
        U64,
        &["/$defs/CreateVolumeInput/properties/expected_tree_revision"],
    ),
    Patch(
        Edit::MinLength(1),
        &["/$defs/CreateVolumeInput/properties/title"],
    ),
    Patch(
        Edit::MaxUtf8Bytes(1024),
        &["/$defs/CreateVolumeInput/properties/title"],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const CREATE_VOLUME_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/properties/author_command_admission_id",
            "/properties/command_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];

const UPDATE_VOLUME_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &["/$defs/UpdateVolumeInput/properties/correlation_id"],
    ),
    Patch(
        U64,
        &[
            "/$defs/UpdateVolumeInput/properties/expected_tree_revision",
            "/$defs/UpdateVolumeInput/properties/order",
        ],
    ),
    Patch(
        Edit::MinLength(1),
        &["/$defs/UpdateVolumeInput/properties/title"],
    ),
    Patch(
        Edit::MaxUtf8Bytes(1024),
        &["/$defs/UpdateVolumeInput/properties/title"],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const UPDATE_VOLUME_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/properties/author_command_admission_id",
            "/properties/command_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];

const DELETE_VOLUME_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &["/$defs/DeleteVolumeInput/properties/correlation_id"],
    ),
    Patch(
        U64,
        &["/$defs/DeleteVolumeInput/properties/expected_tree_revision"],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const DELETE_VOLUME_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/properties/author_command_admission_id",
            "/properties/command_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];
