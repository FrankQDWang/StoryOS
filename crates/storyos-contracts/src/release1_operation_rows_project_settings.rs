//! The rows of the Project setting commands and the Project assistance query in the operation table.

use ts_rs::TS;

use super::{
    Edit, Fixture, Headers, OperationRow, Patch, SCHEMA_ID, SchemaSource, SchemaSources, U64, UUID,
    raw_schema,
};
use crate::ProjectActivityKind;
use crate::release1_operation_registry::RegisteredOperation;
use crate::release1_project_assistance::{
    GET_PROJECT_ASSISTANCE, GetProjectAssistanceResponse, ModelRuntimeQualification,
    ProjectAssistanceAvailability, ProjectAssistanceBinding, ProjectModelDestination,
    UPDATE_PROJECT_ASSISTANCE, UpdateProjectAssistanceConflictReason,
    UpdateProjectAssistanceEffect, UpdateProjectAssistanceInput,
    UpdateProjectAssistanceNoEffectReason, UpdateProjectAssistanceRequest,
    UpdateProjectAssistanceResponse,
};
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};
use crate::release1_set_current_chapter::{
    SET_CURRENT_CHAPTER, SetCurrentChapterConflictReason, SetCurrentChapterEffect,
    SetCurrentChapterInput, SetCurrentChapterNoEffectReason, SetCurrentChapterRefusalReason,
    SetCurrentChapterRequest, SetCurrentChapterResponse,
};
use crate::release1_update_project::{
    UPDATE_PROJECT, UpdateProjectConflictReason, UpdateProjectEffect, UpdateProjectInput,
    UpdateProjectNoEffectReason, UpdateProjectRequest, UpdateProjectResponse,
};

pub(super) const UPDATE_PROJECT_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &UPDATE_PROJECT,
        &[
            "server_derived_project_scope",
            "expected_project_revision",
            "project_active",
        ],
        OperationCatalog::new("project:update", Owner::Release, Binding::PROJECT_COMMAND)
            .activity(&[ProjectActivityKind::ProjectUpdated], &[]),
    ),
    summary: "Rename one exact Project",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "update-project-request",
            raw_schema::<UpdateProjectRequest>,
            "StoryOS Update Project Request",
            UPDATE_PROJECT_REQUEST,
        ),
        SchemaSource::typed(
            "update-project-response",
            raw_schema::<UpdateProjectResponse>,
            "StoryOS Update Project Response",
            UPDATE_PROJECT_RESPONSE,
        ),
    ),
    typescript: &[
        UpdateProjectInput::decl,
        UpdateProjectRequest::decl,
        UpdateProjectNoEffectReason::decl,
        UpdateProjectConflictReason::decl,
        UpdateProjectEffect::decl,
        UpdateProjectResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000712",
      "command_id": "018f0000-0000-7001-8000-000000000711",
      "correlation_id": "018f0000-0000-7001-8000-000000000710",
      "effect": {
        "kind": "authoritative_applied",
        "project_activity_position": "1",
        "revision": "2",
        "title": "Renamed Novel"
      },
      "project": {
        "open": {"kind": "empty"},
        "project_id": "018f0000-0000-7001-8000-000000000201",
        "title": "Renamed Novel"
      },
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
      },
      "receipt": {
        "artifact_lifecycle_event_refs": [],
        "author_action_sequence": null,
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000712",
        "authoritative_commit_ids": [],
        "authoritative_revision_ids": [],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.updateProject.jcs.v1",
          "value_hex_lowercase": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
        },
        "command_kind": "updateProject",
        "condition_refs": [],
        "created_at": "2026-08-25T08:05:00.000Z",
        "draft_artifact_refs": [],
        "expected_heads": [],
        "idempotency_key": "018f0000-0000-7001-8000-000000000714",
        "prior_heads": [],
        "producer_cause": "author_command_admission",
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "proposal_revision_ids": [],
        "receipt_id": "018f0000-0000-7001-8000-000000000713",
        "result": "authoritative_applied",
        "resulting_heads": []
      },
      "schema_id": "storyos.command.update-project.response.v1"
    }"#,
    ),
};

pub(super) const GET_PROJECT_ASSISTANCE_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
        &GET_PROJECT_ASSISTANCE,
        &["server_derived_project_scope", "project_visibility"],
        OperationCatalog::new("project:assistance", Owner::Release, Binding::PROJECT_QUERY),
    ),
    summary: "Inspect Project assistance and its Host-owned fake binding",
    headers: Headers::NONE,
    schemas: SchemaSources::new(
        SchemaSource::no_parameters(
            "project-assistance-request",
            "StoryOS Project Assistance Request",
        ),
        SchemaSource::typed(
            "project-assistance-response",
            raw_schema::<GetProjectAssistanceResponse>,
            "StoryOS Project Assistance Response",
            GET_PROJECT_ASSISTANCE_RESPONSE,
        ),
    ),
    typescript: &[
        ProjectAssistanceAvailability::decl,
        ProjectModelDestination::decl,
        ModelRuntimeQualification::decl,
        ProjectAssistanceBinding::decl,
        GetProjectAssistanceResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "assistance": {
        "availability": "available",
        "destination": "volcengine_agent_plan",
        "external_compatibility_decision": "018f0000-0000-7001-8000-000000000904",
        "model_registration_revision": "018f0000-0000-7001-8000-000000000901",
        "processing_destination_identity": "018f0000-0000-7001-8000-000000000902",
        "processing_destination_identity_evidence_revision": "1",
        "project_model_use_binding_revision": "018f0000-0000-7001-8000-000000000903",
        "revision": "1",
        "runtime_qualification": "pending"
      },
      "correlation_id": "018f0000-0000-7001-8000-000000000920",
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
      },
      "schema_id": "storyos.query.project-assistance.response.v1"
    }"#,
    ),
};

pub(super) const UPDATE_PROJECT_ASSISTANCE_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &UPDATE_PROJECT_ASSISTANCE,
        &[
            "server_derived_project_scope",
            "expected_assistance_revision",
            "project_active",
        ],
        OperationCatalog::new(
            "project:assistance",
            Owner::Release,
            Binding::PROJECT_COMMAND,
        )
        .activity(&[ProjectActivityKind::ProjectAssistanceUpdated], &[]),
    ),
    summary: "Prepare or change Project assistance availability",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "update-project-assistance-request",
            raw_schema::<UpdateProjectAssistanceRequest>,
            "StoryOS Update Project Assistance Request",
            UPDATE_PROJECT_ASSISTANCE_REQUEST,
        ),
        SchemaSource::typed(
            "update-project-assistance-response",
            raw_schema::<UpdateProjectAssistanceResponse>,
            "StoryOS Update Project Assistance Response",
            UPDATE_PROJECT_ASSISTANCE_RESPONSE,
        ),
    ),
    typescript: &[
        UpdateProjectAssistanceInput::decl,
        UpdateProjectAssistanceRequest::decl,
        UpdateProjectAssistanceNoEffectReason::decl,
        UpdateProjectAssistanceConflictReason::decl,
        UpdateProjectAssistanceEffect::decl,
        UpdateProjectAssistanceResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "assistance": {
        "availability": "available",
        "destination": "volcengine_agent_plan",
        "external_compatibility_decision": "018f0000-0000-7001-8000-000000000904",
        "model_registration_revision": "018f0000-0000-7001-8000-000000000901",
        "processing_destination_identity": "018f0000-0000-7001-8000-000000000902",
        "processing_destination_identity_evidence_revision": "1",
        "project_model_use_binding_revision": "018f0000-0000-7001-8000-000000000903",
        "revision": "1",
        "runtime_qualification": "pending"
      },
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000923",
      "command_id": "018f0000-0000-7001-8000-000000000922",
      "correlation_id": "018f0000-0000-7001-8000-000000000921",
      "effect": {
        "availability": "available",
        "kind": "initialized",
        "project_activity_position": "1",
        "revision": "1"
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
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000923",
        "authoritative_commit_ids": [],
        "authoritative_revision_ids": [],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.updateProjectAssistance.jcs.v1",
          "value_hex_lowercase": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
        },
        "command_kind": "updateProjectAssistance",
        "condition_refs": [],
        "created_at": "2026-09-16T03:00:00.000Z",
        "draft_artifact_refs": [],
        "expected_heads": [],
        "idempotency_key": "018f0000-0000-7001-8000-000000000925",
        "prior_heads": [],
        "producer_cause": "author_command_admission",
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "proposal_revision_ids": [],
        "receipt_id": "018f0000-0000-7001-8000-000000000924",
        "result": "authoritative_applied",
        "resulting_heads": []
      },
      "schema_id": "storyos.command.update-project-assistance.response.v1"
    }"#,
    ),
};

pub(super) const SET_CURRENT_CHAPTER_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &SET_CURRENT_CHAPTER,
        &[
            "server_derived_project_scope",
            "project_active",
            "editor_session_writer_generation",
            "expected_current_chapter",
            "expected_authoritative_head",
        ],
        OperationCatalog::new(
            "project:set-current-chapter",
            Owner::Core,
            Binding::EDITOR_COMMAND,
        )
        .activity(&[ProjectActivityKind::CurrentChapterSet], &[]),
    ),
    summary: "Make one eligible Chapter current",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "set-current-chapter-request",
            raw_schema::<SetCurrentChapterRequest>,
            "StoryOS Set Current Chapter Request",
            SET_CURRENT_CHAPTER_REQUEST,
        ),
        SchemaSource::typed(
            "set-current-chapter-response",
            raw_schema::<SetCurrentChapterResponse>,
            "StoryOS Set Current Chapter Response",
            SET_CURRENT_CHAPTER_RESPONSE,
        ),
    ),
    typescript: &[
        SetCurrentChapterInput::decl,
        SetCurrentChapterRequest::decl,
        SetCurrentChapterNoEffectReason::decl,
        SetCurrentChapterConflictReason::decl,
        SetCurrentChapterRefusalReason::decl,
        SetCurrentChapterEffect::decl,
        SetCurrentChapterResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000922",
      "command_id": "018f0000-0000-7001-8000-000000000921",
      "correlation_id": "018f0000-0000-7001-8000-000000000920",
      "effect": {
        "base_snapshot_id": "018f0000-0000-7001-8000-000000000925",
        "current_chapter_id": "018f0000-0000-7001-8000-000000000803",
        "kind": "authoritative_applied",
        "project_activity_position": "4"
      },
      "project": {
        "open": {
          "current_chapter_id": "018f0000-0000-7001-8000-000000000803",
          "kind": "current_chapter"
        },
        "project_id": "018f0000-0000-7001-8000-000000000002",
        "title": "Project A"
      },
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000002"
      },
      "receipt": {
        "artifact_lifecycle_event_refs": [],
        "author_action_sequence": null,
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000922",
        "authoritative_commit_ids": [],
        "authoritative_revision_ids": [],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.setCurrentChapter.jcs.v1",
          "value_hex_lowercase": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
        },
        "command_kind": "setCurrentChapter",
        "condition_refs": [],
        "created_at": "2026-08-29T12:00:00.000Z",
        "draft_artifact_refs": [],
        "expected_heads": ["018f0000-0000-7001-8000-000000000805"],
        "idempotency_key": "018f0000-0000-7001-8000-000000000924",
        "prior_heads": ["018f0000-0000-7001-8000-000000000805"],
        "producer_cause": "author_command_admission",
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000002"
        },
        "proposal_revision_ids": [],
        "receipt_id": "018f0000-0000-7001-8000-000000000923",
        "result": "authoritative_applied",
        "resulting_heads": ["018f0000-0000-7001-8000-000000000805"]
      },
      "schema_id": "storyos.command.set-current-chapter.response.v1"
    }"#,
    ),
};

const UPDATE_PROJECT_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &["/$defs/UpdateProjectInput/properties/correlation_id"],
    ),
    Patch(
        U64,
        &["/$defs/UpdateProjectInput/properties/expected_project_revision"],
    ),
    Patch(
        Edit::MinLength(1),
        &["/$defs/UpdateProjectInput/properties/title"],
    ),
    Patch(
        Edit::MaxUtf8Bytes(1024),
        &["/$defs/UpdateProjectInput/properties/title"],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const UPDATE_PROJECT_RESPONSE: &[Patch] = &[
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

const GET_PROJECT_ASSISTANCE_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/ProjectAssistanceBinding/properties/external_compatibility_decision",
            "/$defs/ProjectAssistanceBinding/properties/model_registration_revision",
            "/$defs/ProjectAssistanceBinding/properties/processing_destination_identity",
            "/$defs/ProjectAssistanceBinding/properties/project_model_use_binding_revision",
            "/$defs/ProjectScope/properties/owner_user_id",
            "/$defs/ProjectScope/properties/project_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(
        U64,
        &[
            "/$defs/ProjectAssistanceBinding/properties/processing_destination_identity_evidence_revision",
            "/$defs/ProjectAssistanceBinding/properties/revision",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];

const UPDATE_PROJECT_ASSISTANCE_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &["/$defs/UpdateProjectAssistanceInput/properties/correlation_id"],
    ),
    Patch(
        U64,
        &["/$defs/UpdateProjectAssistanceInput/properties/expected_assistance_revision"],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const UPDATE_PROJECT_ASSISTANCE_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/ProjectAssistanceBinding/properties/external_compatibility_decision",
            "/$defs/ProjectAssistanceBinding/properties/model_registration_revision",
            "/$defs/ProjectAssistanceBinding/properties/processing_destination_identity",
            "/$defs/ProjectAssistanceBinding/properties/project_model_use_binding_revision",
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
            "/$defs/ProjectAssistanceBinding/properties/processing_destination_identity_evidence_revision",
            "/$defs/ProjectAssistanceBinding/properties/revision",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];

const SET_CURRENT_CHAPTER_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/SetCurrentChapterInput/properties/chapter_id",
            "/$defs/SetCurrentChapterInput/properties/correlation_id",
            "/$defs/SetCurrentChapterInput/properties/editor_session_id",
            "/$defs/SetCurrentChapterInput/properties/expected_current_chapter_id",
            "/$defs/SetCurrentChapterInput/properties/expected_target_revision_id",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const SET_CURRENT_CHAPTER_RESPONSE: &[Patch] = &[
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
