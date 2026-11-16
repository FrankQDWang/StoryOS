//! The rows of the Project Instruction query and its save command in the operation table.

use ts_rs::TS;

use super::{
    Edit, Fixture, Headers, OperationRow, Patch, SCHEMA_ID, SchemaSource, SchemaSources, U64, UUID,
    raw_schema,
};
use crate::release1_operation_registry::RegisteredOperation;
use crate::release1_project_activity::ProjectActivityKind;
use crate::release1_project_instruction::{
    GET_PROJECT_INSTRUCTION, GetProjectInstructionResponse, ProjectInstruction,
    UPDATE_PROJECT_INSTRUCTION, UpdateProjectInstructionConflictReason,
    UpdateProjectInstructionEffect, UpdateProjectInstructionInput,
    UpdateProjectInstructionNoEffectReason, UpdateProjectInstructionRequest,
    UpdateProjectInstructionResponse,
};
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};
use crate::release1_wire::U64_WIRE;

pub(super) const GET_PROJECT_INSTRUCTION_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
        &GET_PROJECT_INSTRUCTION,
        &["server_derived_project_scope", "project_visibility"],
        OperationCatalog::new(
            "project:instruction",
            Owner::Release,
            Binding::PROJECT_QUERY,
        ),
    ),
    summary: "Read the current Revision of the Project Instruction",
    headers: Headers::NONE,
    schemas: SchemaSources::get(SchemaSource::typed(
        "project-instruction-response",
        raw_schema::<GetProjectInstructionResponse>,
        "StoryOS Project Instruction Response",
        GET_PROJECT_INSTRUCTION_RESPONSE,
    )),
    typescript: &[
        ProjectInstruction::decl,
        GetProjectInstructionResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "correlation_id": "018f0000-0000-7001-8000-000000000930",
      "instruction": {
        "instruction_revision_id": "018f0000-0000-7001-8000-000000000931",
        "revision": "1",
        "text": "Write in the past tense."
      },
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
      },
      "schema_id": "storyos.query.project-instruction.response.v1"
    }"#,
    ),
};

pub(super) const UPDATE_PROJECT_INSTRUCTION_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &UPDATE_PROJECT_INSTRUCTION,
        &[
            "server_derived_project_scope",
            "expected_instruction_revision",
            "project_active",
        ],
        OperationCatalog::new(
            "project:instruction",
            Owner::Release,
            Binding::PROJECT_COMMAND,
        )
        .activity(&[ProjectActivityKind::ProjectInstructionUpdated], &[]),
    ),
    summary: "Save the Project Instruction as one new immutable Revision",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "update-project-instruction-request",
            raw_schema::<UpdateProjectInstructionRequest>,
            "StoryOS Update Project Instruction Request",
            UPDATE_PROJECT_INSTRUCTION_REQUEST,
        ),
        SchemaSource::typed(
            "update-project-instruction-response",
            raw_schema::<UpdateProjectInstructionResponse>,
            "StoryOS Update Project Instruction Response",
            UPDATE_PROJECT_INSTRUCTION_RESPONSE,
        ),
    ),
    typescript: &[
        UpdateProjectInstructionInput::decl,
        UpdateProjectInstructionRequest::decl,
        UpdateProjectInstructionNoEffectReason::decl,
        UpdateProjectInstructionConflictReason::decl,
        UpdateProjectInstructionEffect::decl,
        UpdateProjectInstructionResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000933",
      "command_id": "018f0000-0000-7001-8000-000000000932",
      "correlation_id": "018f0000-0000-7001-8000-000000000930",
      "effect": {
        "instruction_revision_id": "018f0000-0000-7001-8000-000000000931",
        "kind": "authoritative_applied",
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
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000933",
        "authoritative_commit_ids": [],
        "authoritative_revision_ids": [],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.updateProjectInstruction.jcs.v1",
          "value_hex_lowercase": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
        },
        "command_kind": "updateProjectInstruction",
        "condition_refs": [],
        "created_at": "2026-11-16T03:00:00.000Z",
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
      "schema_id": "storyos.command.update-project-instruction.response.v1"
    }"#,
    ),
};

const GET_PROJECT_INSTRUCTION_RESPONSE: &[Patch] = &[
    Patch(
        Edit::NullableDefinition("ProjectInstruction"),
        &["/properties/instruction"],
    ),
    Patch(
        UUID,
        &[
            "/$defs/ProjectInstruction/properties/instruction_revision_id",
            "/$defs/ProjectScope/properties/owner_user_id",
            "/$defs/ProjectScope/properties/project_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(U64, &["/$defs/ProjectInstruction/properties/revision"]),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];

const UPDATE_PROJECT_INSTRUCTION_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &["/$defs/UpdateProjectInstructionInput/properties/correlation_id"],
    ),
    Patch(
        U64,
        &["/$defs/UpdateProjectInstructionInput/properties/expected_instruction_revision"],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const UPDATE_PROJECT_INSTRUCTION_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/ProjectScope/properties/owner_user_id",
            "/$defs/ProjectScope/properties/project_id",
            "/properties/author_command_admission_id",
            "/properties/command_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(
        Edit::NullablePattern(U64_WIRE),
        &["/$defs/DomainReceipt/properties/author_action_sequence"],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];
