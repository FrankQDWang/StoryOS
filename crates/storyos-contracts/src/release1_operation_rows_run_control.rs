//! The rows of the AgentRun control commands in the operation table.

use ts_rs::TS;

use super::{
    Edit, Fixture, Headers, OperationRow, Patch, SCHEMA_ID, SchemaSource, SchemaSources, UUID,
    raw_schema,
};
use crate::ProjectActivityKind;
use crate::release1_agent_run_control::{
    CANCEL_AGENT_RUN, CancelAgentRunConflictReason, CancelAgentRunEffect, CancelAgentRunInput,
    CancelAgentRunNoEffectReason, CancelAgentRunRequest, CancelAgentRunResponse, PAUSE_AGENT_RUN,
    PauseAgentRunConflictReason, PauseAgentRunEffect, PauseAgentRunInput,
    PauseAgentRunNoEffectReason, PauseAgentRunRequest, PauseAgentRunResponse, STEER_AGENT_RUN,
    SteerAgentRunEffect, SteerAgentRunInput, SteerAgentRunRequest, SteerAgentRunResponse,
};
use crate::release1_operation_registry::RegisteredOperation;
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};

pub(super) const STEER_AGENT_RUN_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &STEER_AGENT_RUN,
        &[
            "server_derived_project_scope",
            "run_scope_join",
            "exact_conversation",
            "nonterminal_run",
            "bounded_author_input",
        ],
        OperationCatalog::new(
            "agent-run:steer",
            Owner::Context,
            Binding::AGENT_RUN_CONTROL,
        )
        .activity(&[ProjectActivityKind::AgentRunSteeringRetained], &[]),
    ),
    summary: "Retain guidance for the next safe decision",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "steer-agent-run-request",
            raw_schema::<SteerAgentRunRequest>,
            "StoryOS Steering Input Request",
            STEER_AGENT_RUN_REQUEST,
        ),
        SchemaSource::typed(
            "steer-agent-run-response",
            raw_schema::<SteerAgentRunResponse>,
            "StoryOS Steering Input Response",
            STEER_AGENT_RUN_RESPONSE,
        ),
    ),
    typescript: &[
        SteerAgentRunInput::decl,
        SteerAgentRunRequest::decl,
        SteerAgentRunEffect::decl,
        SteerAgentRunResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000b03",
      "command_id": "018f0000-0000-7001-8000-000000000b02",
      "correlation_id": "018f0000-0000-7001-8000-000000000b01",
      "effect": {
        "input_position": "1",
        "kind": "retained",
        "run_id": "018f0000-0000-7001-8000-000000000b01",
        "steering_input_id": "018f0000-0000-7001-8000-000000000b02"
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
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000b03",
        "authoritative_commit_ids": [],
        "authoritative_revision_ids": [],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.steerAgentRun.jcs.v1",
          "value_hex_lowercase": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
        },
        "command_kind": "steerAgentRun",
        "condition_refs": [],
        "created_at": "2026-09-20T01:00:00.000Z",
        "draft_artifact_refs": [],
        "expected_heads": [],
        "idempotency_key": "018f0000-0000-7001-8000-000000000b05",
        "prior_heads": [],
        "producer_cause": "author_command_admission",
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "proposal_revision_ids": [],
        "receipt_id": "018f0000-0000-7001-8000-000000000b04",
        "result": "no_effect",
        "resulting_heads": []
      },
      "schema_id": "storyos.command.steer-agent-run.response.v1"
    }"#,
    ),
};

pub(super) const PAUSE_AGENT_RUN_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &PAUSE_AGENT_RUN,
        &[
            "server_derived_project_scope",
            "run_scope_join",
            "current_run_state_pauseable",
            "current_fence_generation",
        ],
        OperationCatalog::new(
            "agent-run:pause",
            Owner::Context,
            Binding::AGENT_RUN_CONTROL,
        )
        .activity(&[ProjectActivityKind::AgentRunPaused], &[]),
    ),
    summary: "Pause one AgentRun without cancelling it",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "pause-agent-run-request",
            raw_schema::<PauseAgentRunRequest>,
            "StoryOS Pause Agent Run Request",
            PAUSE_AGENT_RUN_REQUEST,
        ),
        SchemaSource::typed(
            "pause-agent-run-response",
            raw_schema::<PauseAgentRunResponse>,
            "StoryOS Pause Agent Run Response",
            PAUSE_AGENT_RUN_RESPONSE,
        ),
    ),
    typescript: &[
        PauseAgentRunInput::decl,
        PauseAgentRunRequest::decl,
        PauseAgentRunNoEffectReason::decl,
        PauseAgentRunConflictReason::decl,
        PauseAgentRunEffect::decl,
        PauseAgentRunResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000b03",
      "command_id": "018f0000-0000-7001-8000-000000000b02",
      "correlation_id": "018f0000-0000-7001-8000-000000000b01",
      "effect": {
        "fence_generation": "1",
        "kind": "applied",
        "project_activity_position": "2",
        "run_id": "018f0000-0000-7001-8000-000000000a34",
        "status": "paused"
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
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000b03",
        "authoritative_commit_ids": [],
        "authoritative_revision_ids": [],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.pauseAgentRun.jcs.v1",
          "value_hex_lowercase": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
        },
        "command_kind": "pauseAgentRun",
        "condition_refs": [],
        "created_at": "2026-09-20T01:00:00.000Z",
        "draft_artifact_refs": [],
        "expected_heads": [],
        "idempotency_key": "018f0000-0000-7001-8000-000000000b05",
        "prior_heads": [],
        "producer_cause": "author_command_admission",
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "proposal_revision_ids": [],
        "receipt_id": "018f0000-0000-7001-8000-000000000b04",
        "result": "authoritative_applied",
        "resulting_heads": []
      },
      "schema_id": "storyos.command.pause-agent-run.response.v1"
    }"#,
    ),
};

pub(super) const CANCEL_AGENT_RUN_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &CANCEL_AGENT_RUN,
        &[
            "server_derived_project_scope",
            "run_scope_join",
            "current_run_state_cancellable",
            "current_fence_generation",
        ],
        OperationCatalog::new(
            "agent-run:cancel",
            Owner::Context,
            Binding::AGENT_RUN_CONTROL,
        )
        .activity(&[ProjectActivityKind::AgentRunCancelled], &[]),
    ),
    summary: "Cancel one AgentRun after a durable fence",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "cancel-agent-run-request",
            raw_schema::<CancelAgentRunRequest>,
            "StoryOS Cancel Agent Run Request",
            CANCEL_AGENT_RUN_REQUEST,
        ),
        SchemaSource::typed(
            "cancel-agent-run-response",
            raw_schema::<CancelAgentRunResponse>,
            "StoryOS Cancel Agent Run Response",
            CANCEL_AGENT_RUN_RESPONSE,
        ),
    ),
    typescript: &[
        CancelAgentRunInput::decl,
        CancelAgentRunRequest::decl,
        CancelAgentRunNoEffectReason::decl,
        CancelAgentRunConflictReason::decl,
        CancelAgentRunEffect::decl,
        CancelAgentRunResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000b03",
      "command_id": "018f0000-0000-7001-8000-000000000b02",
      "correlation_id": "018f0000-0000-7001-8000-000000000b11",
      "effect": {
        "fence_generation": "1",
        "kind": "applied",
        "project_activity_position": "2",
        "run_id": "018f0000-0000-7001-8000-000000000a34",
        "status": "cancelled"
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
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000b03",
        "authoritative_commit_ids": [],
        "authoritative_revision_ids": [],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.cancelAgentRun.jcs.v1",
          "value_hex_lowercase": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
        },
        "command_kind": "cancelAgentRun",
        "condition_refs": [],
        "created_at": "2026-09-20T01:00:00.000Z",
        "draft_artifact_refs": [],
        "expected_heads": [],
        "idempotency_key": "018f0000-0000-7001-8000-000000000b05",
        "prior_heads": [],
        "producer_cause": "author_command_admission",
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "proposal_revision_ids": [],
        "receipt_id": "018f0000-0000-7001-8000-000000000b04",
        "result": "authoritative_applied",
        "resulting_heads": []
      },
      "schema_id": "storyos.command.cancel-agent-run.response.v1"
    }"#,
    ),
};

const STEER_AGENT_RUN_REQUEST: &[Patch] = &[
    Patch(
        Edit::MaxLength(8000),
        &["/$defs/AuthorMessage/properties/text"],
    ),
    Patch(
        Edit::MinLength(1),
        &["/$defs/AuthorMessage/properties/text"],
    ),
    Patch(
        UUID,
        &[
            "/$defs/SelectionPoint/properties/manuscript_block_id",
            "/$defs/SelectionReference/oneOf/0/properties/base_authoritative_revision_id",
            "/$defs/SelectionReference/oneOf/0/properties/chapter_id",
            "/$defs/SelectionReference/oneOf/1/properties/chapter_id",
            "/$defs/SelectionReference/oneOf/1/properties/operation_id",
            "/$defs/SelectionReference/oneOf/1/properties/proposal_id",
            "/$defs/SelectionReference/oneOf/1/properties/revision_id",
            "/$defs/SteerAgentRunInput/properties/conversation_id",
            "/$defs/SteerAgentRunInput/properties/correlation_id",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const STEER_AGENT_RUN_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/ProjectScope/properties/owner_user_id",
            "/$defs/ProjectScope/properties/project_id",
            "/$defs/SteerAgentRunEffect/oneOf/0/properties/run_id",
            "/$defs/SteerAgentRunEffect/oneOf/0/properties/steering_input_id",
            "/properties/author_command_admission_id",
            "/properties/command_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];

const PAUSE_AGENT_RUN_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &["/$defs/PauseAgentRunInput/properties/correlation_id"],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const PAUSE_AGENT_RUN_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/PauseAgentRunEffect/oneOf/0/properties/run_id",
            "/$defs/ProjectScope/properties/owner_user_id",
            "/$defs/ProjectScope/properties/project_id",
            "/properties/author_command_admission_id",
            "/properties/command_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];

const CANCEL_AGENT_RUN_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &["/$defs/CancelAgentRunInput/properties/correlation_id"],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const CANCEL_AGENT_RUN_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/CancelAgentRunEffect/oneOf/0/properties/run_id",
            "/$defs/ProjectScope/properties/owner_user_id",
            "/$defs/ProjectScope/properties/project_id",
            "/properties/author_command_admission_id",
            "/properties/command_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];
