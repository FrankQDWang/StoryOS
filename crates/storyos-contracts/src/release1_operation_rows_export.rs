//! The rows of the export operations in the operation table.

use ts_rs::TS;

use super::{
    Fixture, Headers, NO_PATCHES, OperationRow, Patch, ResponseMedia, SCHEMA_ID, SchemaSource,
    SchemaSources, UUID, raw_schema,
};
use crate::ProjectActivityKind;
use crate::release1_operation_registry::RegisteredOperation;
use crate::release1_project_export::{
    EXPORT_PROJECT_ARCHIVE, ExportProjectArchiveEffect, ExportProjectArchiveInput,
    ExportProjectArchiveRequest, ExportProjectArchiveResponse, ProjectExportRef,
};
use crate::release1_project_export_query::{GET_EXPORT_OPERATION, GetExportOperationResponse};
use crate::release1_readable_export::{
    EXPORT_HUMAN_READABLE_MANUSCRIPT, ExportAcknowledgement, ExportHumanReadableManuscriptEffect,
    ExportHumanReadableManuscriptInput, ExportHumanReadableManuscriptRequest,
    ExportHumanReadableManuscriptResponse, HumanReadableManuscriptExportRef,
};
use crate::release1_readable_export_query::{
    GET_HUMAN_READABLE_MANUSCRIPT_EXPORT, GetHumanReadableManuscriptExportResponse,
};
use crate::release1_route_catalog::{
    ActionClass, Admission, Binding, EditorRule, ErrorProfile, Idempotency, OperationCatalog,
    Owner, Settlement,
};

pub(super) const EXPORT_PROJECT_ARCHIVE_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &EXPORT_PROJECT_ARCHIVE,
        &[
            "server_derived_project_scope",
            "consistent_source_snapshot",
            "archive_path_profile",
            "integrity_protection_before_completion",
        ],
        OperationCatalog::new(
            "project-archive:export",
            Owner::Storage,
            Binding {
                action_class: ActionClass::ExplicitProjectCommand,
                admission: Admission::AuthorCommandOrAsyncOperation,
                editor: EditorRule::Optional,
                idempotency: Idempotency::HeaderAndAdmissionRecord,
                settlement: Settlement::Accepted {
                    query: &GET_EXPORT_OPERATION,
                    operation_ref: "project_export",
                },
                error_profile: ErrorProfile::AsyncCommand,
            },
        )
        .activity(&[ProjectActivityKind::ProjectExportSettled], &[]),
    ),
    summary: "Admit one durable Project Export Archive operation",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "export-project-archive-request",
            raw_schema::<ExportProjectArchiveRequest>,
            "StoryOS Export Project Archive Request",
            EXPORT_PROJECT_ARCHIVE_REQUEST,
        ),
        SchemaSource::typed(
            "export-project-archive-response",
            raw_schema::<ExportProjectArchiveResponse>,
            "StoryOS Export Project Archive Response",
            EXPORT_PROJECT_ARCHIVE_RESPONSE,
        ),
    ),
    typescript: &[
        ExportProjectArchiveInput::decl,
        ExportProjectArchiveRequest::decl,
        ProjectExportRef::decl,
        ExportProjectArchiveEffect::decl,
        ExportProjectArchiveResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "acknowledgement": "accepted",
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000922",
      "command_id": "018f0000-0000-7001-8000-000000000921",
      "correlation_id": "018f0000-0000-7001-8000-000000000920",
      "effect": {
        "archive_path_profile": "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1",
        "archive_profile": "storyos.project-export.v1",
        "export_id": "018f0000-0000-7001-8000-000000000926",
        "kind": "admitted",
        "source_snapshot": {
          "created_at": "2026-09-01T12:00:00.000Z",
          "expires_at": null,
          "project_activity_position": "2",
          "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
          },
          "projection_generations": {},
          "redaction_profile": "storyos.author.v1",
          "replay_generation": "1",
          "schema_profile": "storyos.public.release.1",
          "snapshot_id": "018f0000-0000-7001-8000-000000000040",
          "snapshot_kind": "canonical",
          "source_watermarks": {}
        }
      },
      "operation_ref": {"export_id": "018f0000-0000-7001-8000-000000000926", "kind": "project_export"},
      "project": {
        "open": {"kind": "empty"},
        "project_id": "018f0000-0000-7001-8000-000000000201",
        "title": "Empty Novel"
      },
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
      },
      "schema_id": "storyos.command.export-project-archive.response.v1"
    }"#,
    ),
};

pub(super) const EXPORT_HUMAN_READABLE_MANUSCRIPT_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &EXPORT_HUMAN_READABLE_MANUSCRIPT,
        &[
            "server_derived_project_scope",
            "consistent_source_snapshot",
            "deterministic_volume_chapter_order",
            "unavailable_content_representation_profile",
        ],
        OperationCatalog::new(
            "manuscript-export:human-readable",
            Owner::Release,
            Binding {
                action_class: ActionClass::ExplicitProjectCommand,
                admission: Admission::AuthorCommandOrAsyncOperation,
                editor: EditorRule::Optional,
                idempotency: Idempotency::HeaderAndAdmissionRecord,
                settlement: Settlement::Accepted {
                    query: &GET_HUMAN_READABLE_MANUSCRIPT_EXPORT,
                    operation_ref: "human_readable_manuscript_export",
                },
                error_profile: ErrorProfile::AsyncCommand,
            },
        )
        .activity(
            &[ProjectActivityKind::HumanReadableManuscriptExportSettled],
            &[],
        )
        .notes(EXPORT_HUMAN_READABLE_MANUSCRIPT_NOTES),
    ),
    summary: "Admit one durable human-readable manuscript export",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "export-human-readable-manuscript-request",
            raw_schema::<ExportHumanReadableManuscriptRequest>,
            "StoryOS Export Human-Readable Manuscript Request",
            EXPORT_HUMAN_READABLE_MANUSCRIPT_REQUEST,
        ),
        SchemaSource::typed(
            "export-human-readable-manuscript-response",
            raw_schema::<ExportHumanReadableManuscriptResponse>,
            "StoryOS Export Human-Readable Manuscript Response",
            EXPORT_HUMAN_READABLE_MANUSCRIPT_RESPONSE,
        ),
    ),
    typescript: &[
        ExportHumanReadableManuscriptInput::decl,
        ExportHumanReadableManuscriptRequest::decl,
        ExportAcknowledgement::decl,
        HumanReadableManuscriptExportRef::decl,
        ExportHumanReadableManuscriptEffect::decl,
        ExportHumanReadableManuscriptResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "acknowledgement": "accepted",
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000912",
      "command_id": "018f0000-0000-7001-8000-000000000911",
      "correlation_id": "018f0000-0000-7001-8000-000000000910",
      "effect": {
        "export_id": "018f0000-0000-7001-8000-000000000916",
        "export_profile": "storyos.readable-export.utf8-lf.v1",
        "kind": "admitted",
        "source_snapshot": {
          "created_at": "2026-08-31T12:00:00.000Z",
          "expires_at": null,
          "project_activity_position": "2",
          "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
          },
          "projection_generations": {},
          "redaction_profile": "storyos.author.v1",
          "replay_generation": "1",
          "schema_profile": "storyos.public.release.1",
          "snapshot_id": "018f0000-0000-7001-8000-000000000040",
          "snapshot_kind": "canonical",
          "source_watermarks": {}
        }
      },
      "operation_ref": {
        "export_id": "018f0000-0000-7001-8000-000000000916",
        "kind": "human_readable_manuscript_export"
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
      "schema_id": "storyos.command.export-human-readable-manuscript.response.v1"
    }"#,
    ),
};

pub(super) const GET_EXPORT_OPERATION_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
        &GET_EXPORT_OPERATION,
        &[
            "export_scope_join",
            "export_operation_visibility",
            "redaction_profile",
        ],
        OperationCatalog::new(
            "project-archive:export-status",
            Owner::Storage,
            Binding::CANONICAL_QUERY,
        ),
    ),
    summary: "Inspect or download one settled Project Export Archive",
    headers: Headers::NONE,
    schemas: SchemaSources::new(
        SchemaSource::titled_path_parameters(
            "export-operation-request",
            "StoryOS Export Operation Request",
        ),
        SchemaSource::typed(
            "export-operation-response",
            raw_schema::<GetExportOperationResponse>,
            "StoryOS Export Operation Response",
            NO_PATCHES,
        ),
    )
    .response_media(ResponseMedia::JsonOrProjectArchive),
    typescript: &[GetExportOperationResponse::decl],
    fixture: Fixture::Json(
        r#"{
      "archive_path_profile": "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1",
      "archive_profile": "storyos.project-export.v1",
      "correlation_id": "018f0000-0000-7001-8000-000000000082",
      "export_id": "018f0000-0000-7001-8000-000000000926",
      "immutable_root": "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
      },
      "query_id": "018f0000-0000-7001-8000-000000000081",
      "schema_id": "storyos.query.export-operation.response.v1",
      "source_snapshot": {
        "created_at": "2026-09-01T12:00:00.000Z",
        "expires_at": null,
        "project_activity_position": "2",
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "projection_generations": {},
        "redaction_profile": "storyos.author.v1",
        "replay_generation": "1",
        "schema_profile": "storyos.public.release.1",
        "snapshot_id": "018f0000-0000-7001-8000-000000000040",
        "snapshot_kind": "canonical",
        "source_watermarks": {}
      },
      "status": "ready"
    }"#,
    ),
};

pub(super) const GET_HUMAN_READABLE_MANUSCRIPT_EXPORT_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
        &GET_HUMAN_READABLE_MANUSCRIPT_EXPORT,
        &[
            "server_derived_project_scope",
            "export_scope_join",
            "export_operation_visibility",
            "deterministic_order_profile",
            "unavailable_content_representation_profile",
        ],
        OperationCatalog::new(
            "manuscript-export:human-readable-status",
            Owner::Release,
            Binding::CANONICAL_QUERY,
        )
        .notes(GET_HUMAN_READABLE_MANUSCRIPT_EXPORT_NOTES),
    ),
    summary: "Inspect one human-readable manuscript export",
    headers: Headers::NONE,
    schemas: SchemaSources::new(
        SchemaSource::titled_path_parameters(
            "human-readable-manuscript-export-request",
            "StoryOS Human-Readable Manuscript Export Request",
        ),
        SchemaSource::typed(
            "human-readable-manuscript-export-response",
            raw_schema::<GetHumanReadableManuscriptExportResponse>,
            "StoryOS Human-Readable Manuscript Export Response",
            NO_PATCHES,
        ),
    ),
    typescript: &[GetHumanReadableManuscriptExportResponse::decl],
    fixture: Fixture::Json(
        r##"{
      "content_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "correlation_id": "018f0000-0000-7001-8000-000000000072",
      "export_id": "018f0000-0000-7001-8000-000000000916",
      "export_profile": "storyos.readable-export.utf8-lf.v1",
      "manuscript_utf8": "# Volume A\n\n## Chapter A\n\nHello world\n",
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
      },
      "query_id": "018f0000-0000-7001-8000-000000000071",
      "schema_id": "storyos.query.human-readable-manuscript-export.response.v1",
      "source_snapshot": {
        "created_at": "2026-08-31T12:00:00.000Z",
        "expires_at": null,
        "project_activity_position": "2",
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "projection_generations": {},
        "redaction_profile": "storyos.author.v1",
        "replay_generation": "1",
        "schema_profile": "storyos.public.release.1",
        "snapshot_id": "018f0000-0000-7001-8000-000000000040",
        "snapshot_kind": "canonical",
        "source_watermarks": {}
      },
      "status": "ready"
    }"##,
    ),
};

const EXPORT_HUMAN_READABLE_MANUSCRIPT_NOTES: &str = "This is the human-readable manuscript delivery required by the Release 1 editor baseline. It is separate from the portable Project Export Archive: output follows deterministic volume/chapter order and explicitly represents unavailable content; archive manifest, entry, and integrity semantics do not apply.";

const GET_HUMAN_READABLE_MANUSCRIPT_EXPORT_NOTES: &str = "This is the exact asynchronous settlement/status query for exportHumanReadableManuscript. It is not getExportOperation, whose shape and owner remain the portable Project Export Archive contract.";

const EXPORT_PROJECT_ARCHIVE_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &["/$defs/ExportProjectArchiveInput/properties/correlation_id"],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const EXPORT_PROJECT_ARCHIVE_RESPONSE: &[Patch] = &[
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

const EXPORT_HUMAN_READABLE_MANUSCRIPT_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &["/$defs/ExportHumanReadableManuscriptInput/properties/correlation_id"],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const EXPORT_HUMAN_READABLE_MANUSCRIPT_RESPONSE: &[Patch] = &[
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
