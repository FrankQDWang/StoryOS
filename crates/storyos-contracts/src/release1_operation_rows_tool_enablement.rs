//! The row of the Project Tool Enablement command in the operation table.

use ts_rs::TS;

use super::{
    Edit, Fixture, Headers, OperationRow, Patch, SCHEMA_ID, SchemaSource, SchemaSources, UUID,
    raw_schema,
};
use crate::release1_operation_registry::RegisteredOperation;
use crate::release1_project_activity::ProjectActivityKind;
use crate::release1_project_tool_enablement::{
    SET_PROJECT_TOOL_ENABLEMENT, SetProjectToolEnablementEffect, SetProjectToolEnablementInput,
    SetProjectToolEnablementNoEffectReason, SetProjectToolEnablementRefusalReason,
    SetProjectToolEnablementRequest, SetProjectToolEnablementResponse,
};
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};

pub(super) const SET_PROJECT_TOOL_ENABLEMENT_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &SET_PROJECT_TOOL_ENABLEMENT,
        &[
            "server_derived_project_scope",
            "offered_tool_registration",
            "project_active",
        ],
        OperationCatalog::new(
            "project:tool-enablement",
            Owner::Release,
            Binding::PROJECT_COMMAND,
        )
        .activity(&[ProjectActivityKind::ProjectToolEnablementSet], &[]),
    ),
    summary: "Enable or disable one offered Tool Registration for a Project",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "set-project-tool-enablement-request",
            raw_schema::<SetProjectToolEnablementRequest>,
            "StoryOS Set Project Tool Enablement Request",
            SET_PROJECT_TOOL_ENABLEMENT_REQUEST,
        ),
        SchemaSource::typed(
            "set-project-tool-enablement-response",
            raw_schema::<SetProjectToolEnablementResponse>,
            "StoryOS Set Project Tool Enablement Response",
            SET_PROJECT_TOOL_ENABLEMENT_RESPONSE,
        ),
    ),
    typescript: &[
        SetProjectToolEnablementInput::decl,
        SetProjectToolEnablementRequest::decl,
        SetProjectToolEnablementNoEffectReason::decl,
        SetProjectToolEnablementRefusalReason::decl,
        SetProjectToolEnablementEffect::decl,
        SetProjectToolEnablementResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000933",
      "command_id": "018f0000-0000-7001-8000-000000000932",
      "correlation_id": "018f0000-0000-7001-8000-000000000931",
      "effect": {
        "enabled": true,
        "kind": "authoritative_applied",
        "project_activity_position": "2",
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
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000933",
        "authoritative_commit_ids": [],
        "authoritative_revision_ids": [],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.setProjectToolEnablement.jcs.v1",
          "value_hex_lowercase": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
        },
        "command_kind": "setProjectToolEnablement",
        "condition_refs": [],
        "created_at": "2026-11-04T03:00:00.000Z",
        "draft_artifact_refs": [],
        "expected_heads": [],
        "idempotency_key": "018f0000-0000-7001-8000-000000000935",
        "prior_heads": [],
        "producer_cause": "author_command_admission",
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "proposal_revision_ids": [],
        "receipt_id": "018f0000-0000-7001-8000-000000000934",
        "result": "authoritative_applied",
        "resulting_heads": []
      },
      "schema_id": "storyos.command.set-project-tool-enablement.response.v1"
    }"#,
    ),
};

const SET_PROJECT_TOOL_ENABLEMENT_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &["/$defs/SetProjectToolEnablementInput/properties/correlation_id"],
    ),
    Patch(
        Edit::MinLength(1),
        &["/$defs/SetProjectToolEnablementInput/properties/registration_id"],
    ),
    Patch(
        Edit::MaxUtf8Bytes(128),
        &["/$defs/SetProjectToolEnablementInput/properties/registration_id"],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const SET_PROJECT_TOOL_ENABLEMENT_RESPONSE: &[Patch] = &[
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
