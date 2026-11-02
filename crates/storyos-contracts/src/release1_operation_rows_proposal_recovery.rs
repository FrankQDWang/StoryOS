//! The rows of the Proposal replan and reopen commands in the operation table.

use ts_rs::TS;

use super::{
    Edit, Fixture, Headers, OperationRow, Patch, SCHEMA_ID, SchemaSource, SchemaSources, UUID,
    raw_schema,
};
use crate::release1_operation_registry::RegisteredOperation;
use crate::release1_reopen_rejected_operations::{
    REOPEN_REJECTED_OPERATIONS, ReopenReceipt, ReopenReceiptResult,
    ReopenRejectedOperationsConflictReason, ReopenRejectedOperationsEffect,
    ReopenRejectedOperationsInput, ReopenRejectedOperationsRefusalReason,
    ReopenRejectedOperationsRequest, ReopenRejectedOperationsResponse,
};
use crate::release1_reopen_withdrawn_proposal::{
    REOPEN_WITHDRAWN_PROPOSAL, ReopenWithdrawnProposalConflictReason,
    ReopenWithdrawnProposalEffect, ReopenWithdrawnProposalInput,
    ReopenWithdrawnProposalNoEffectReason, ReopenWithdrawnProposalRefusalReason,
    ReopenWithdrawnProposalRequest, ReopenWithdrawnProposalResponse, ReopenWithdrawnReceipt,
    ReopenWithdrawnReceiptResult,
};
use crate::release1_replan_proposal::{
    REPLAN_PROPOSAL, ReplanProposalConflictReason, ReplanProposalEffect, ReplanProposalInput,
    ReplanProposalRefusalReason, ReplanProposalRequest, ReplanProposalResponse, ReplanReceipt,
    ReplanReceiptResult, ReplanSourceCondition,
};
use crate::release1_route_catalog::{
    AuthorCause, Binding, CauseProfiles, CurrentProducerCause, OperationCatalog, Owner,
};

pub(super) const REPLAN_PROPOSAL_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &REPLAN_PROPOSAL,
        &[
            "server_derived_project_scope",
            "project_active",
            "editor_session_writer_generation",
            "exact_conflict_or_recovery_conflict_source",
            "expected_current_proposal_head",
            "expected_current_target_revisions",
            "explicit_editor_control",
        ],
        OperationCatalog::conditional_by_cause(
            "proposal:replan",
            Owner::Core,
            &REPLAN_PROPOSAL_CAUSES,
        ),
    ),
    summary: "Replan a conflicted Proposal",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "replan-proposal-request",
            raw_schema::<ReplanProposalRequest>,
            "StoryOS Replan Proposal Request",
            REPLAN_PROPOSAL_REQUEST,
        ),
        SchemaSource::typed(
            "replan-proposal-response",
            raw_schema::<ReplanProposalResponse>,
            "StoryOS Replan Proposal Response",
            REPLAN_PROPOSAL_RESPONSE,
        ),
    ),
    typescript: &[
        ReplanSourceCondition::decl,
        ReplanProposalInput::decl,
        ReplanProposalRequest::decl,
        ReplanReceiptResult::decl,
        ReplanReceipt::decl,
        ReplanProposalRefusalReason::decl,
        ReplanProposalConflictReason::decl,
        ReplanProposalEffect::decl,
        ReplanProposalResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000e22",
      "command_id": "018f0000-0000-7001-8000-000000000e21",
      "correlation_id": "018f0000-0000-7001-8000-000000000e20",
      "effect": {
        "author_action_sequence": "5",
        "kind": "resolved",
        "preserved_closure": "open",
        "preserved_generation": "ready",
        "resulting_proposal_revision_id": "018f0000-0000-7001-8000-000000000e26",
        "resulting_validation": "pending",
        "source_condition": {
          "kind": "proposal_conflict",
          "proposal_conflict_ref": "018f0000-0000-7001-8000-000000000e25"
        },
        "state_event_refs": ["018f0000-0000-7001-8000-000000000e27"],
        "undo_disposition": "forward"
      },
      "project": {
        "open": {
          "current_chapter_id": "018f0000-0000-7001-8000-000000000301",
          "kind": "current_chapter"
        },
        "project_id": "018f0000-0000-7001-8000-000000000201",
        "title": "Open Proposal Novel"
      },
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
      },
      "receipt": {
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000e22",
        "authoritative_commit_ids": [],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.replanProposal.jcs.v1",
          "value_hex_lowercase": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
        },
        "created_at": "2026-09-27T12:00:00.000Z",
        "expected_current_target_revisions": ["018f0000-0000-7001-8000-000000000b06"],
        "idempotency_key": "018f0000-0000-7001-8000-000000000e24",
        "prior_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000b06"],
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "proposal_id": "018f0000-0000-7001-8000-000000000b02",
        "receipt_id": "018f0000-0000-7001-8000-000000000e23",
        "result": "resolved",
        "resulting_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000b06"],
        "resulting_proposal_revision_id": "018f0000-0000-7001-8000-000000000e26",
        "source_proposal_revision_id": "018f0000-0000-7001-8000-000000000b03"
      },
      "schema_id": "storyos.command.replan-proposal.response.v1"
    }"#,
    ),
};

pub(super) const REOPEN_WITHDRAWN_PROPOSAL_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &REOPEN_WITHDRAWN_PROPOSAL,
        &[
            "proposal_scope_join",
            "current_closure_withdrawn",
            "exact_withdrawal_event_ref",
            "expected_target_revisions",
        ],
        OperationCatalog::new(
            "proposal:reopen-withdrawn",
            Owner::Core,
            Binding::EDITOR_COMMAND,
        ),
    ),
    summary: "Reopen a withdrawn Proposal",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "reopen-withdrawn-proposal-request",
            raw_schema::<ReopenWithdrawnProposalRequest>,
            "StoryOS Reopen Withdrawn Proposal Request",
            REOPEN_WITHDRAWN_PROPOSAL_REQUEST,
        ),
        SchemaSource::typed(
            "reopen-withdrawn-proposal-response",
            raw_schema::<ReopenWithdrawnProposalResponse>,
            "StoryOS Reopen Withdrawn Proposal Response",
            REOPEN_WITHDRAWN_PROPOSAL_RESPONSE,
        ),
    ),
    typescript: &[
        ReopenWithdrawnProposalInput::decl,
        ReopenWithdrawnProposalRequest::decl,
        ReopenWithdrawnReceiptResult::decl,
        ReopenWithdrawnReceipt::decl,
        ReopenWithdrawnProposalRefusalReason::decl,
        ReopenWithdrawnProposalConflictReason::decl,
        ReopenWithdrawnProposalNoEffectReason::decl,
        ReopenWithdrawnProposalEffect::decl,
        ReopenWithdrawnProposalResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000e22",
      "command_id": "018f0000-0000-7001-8000-000000000e21",
      "correlation_id": "018f0000-0000-7001-8000-000000000e20",
      "effect": {
        "author_action_sequence": "4",
        "kind": "resolved",
        "preserved_generation": "ready",
        "preserved_operation_resolution": "pending",
        "prior_closure": "withdrawn",
        "resulting_closure": "open",
        "resulting_proposal_revision_id": "018f0000-0000-7001-8000-000000000b09",
        "resulting_validation": "pending",
        "state_event_refs": ["018f0000-0000-7001-8000-000000000b09"],
        "undo_disposition": "forward",
        "withdrawal_event_ref": "018f0000-0000-7001-8000-000000000e15"
      },
      "project": {
        "open": {
          "current_chapter_id": "018f0000-0000-7001-8000-000000000301",
          "kind": "current_chapter"
        },
        "project_id": "018f0000-0000-7001-8000-000000000201",
        "title": "Open Proposal Novel"
      },
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
      },
      "receipt": {
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000e22",
        "authoritative_commit_ids": [],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.reopenWithdrawnProposal.jcs.v1",
          "value_hex_lowercase": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
        },
        "created_at": "2026-09-28T12:00:00.000Z",
        "expected_target_revisions": ["018f0000-0000-7001-8000-000000000b06"],
        "idempotency_key": "018f0000-0000-7001-8000-000000000e24",
        "prior_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000b06"],
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "proposal_id": "018f0000-0000-7001-8000-000000000b02",
        "receipt_id": "018f0000-0000-7001-8000-000000000e23",
        "result": "resolved",
        "resulting_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000b06"],
        "resulting_proposal_revision_id": "018f0000-0000-7001-8000-000000000b09",
        "source_proposal_revision_id": "018f0000-0000-7001-8000-000000000b03",
        "withdrawal_event_ref": "018f0000-0000-7001-8000-000000000e15"
      },
      "schema_id": "storyos.command.reopen-withdrawn-proposal.response.v1"
    }"#,
    ),
};

pub(super) const REOPEN_REJECTED_OPERATIONS_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &REOPEN_REJECTED_OPERATIONS,
        &[
            "server_derived_project_scope",
            "project_active",
            "editor_session_writer_generation",
            "current_open_ready_proposal_revision",
            "selected_rejected_operations",
            "matching_rejection_event_refs",
            "expected_target_revisions",
            "explicit_editor_control",
        ],
        OperationCatalog::new(
            "proposal:reopen-rejected-operations",
            Owner::Core,
            Binding::EDITOR_COMMAND,
        ),
    ),
    summary: "Reopen rejected Proposal Operations",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "reopen-rejected-operations-request",
            raw_schema::<ReopenRejectedOperationsRequest>,
            "StoryOS Reopen Rejected Operations Request",
            REOPEN_REJECTED_OPERATIONS_REQUEST,
        ),
        SchemaSource::typed(
            "reopen-rejected-operations-response",
            raw_schema::<ReopenRejectedOperationsResponse>,
            "StoryOS Reopen Rejected Operations Response",
            REOPEN_REJECTED_OPERATIONS_RESPONSE,
        ),
    ),
    typescript: &[
        ReopenRejectedOperationsInput::decl,
        ReopenRejectedOperationsRequest::decl,
        ReopenReceiptResult::decl,
        ReopenReceipt::decl,
        ReopenRejectedOperationsRefusalReason::decl,
        ReopenRejectedOperationsConflictReason::decl,
        ReopenRejectedOperationsEffect::decl,
        ReopenRejectedOperationsResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000d22",
      "command_id": "018f0000-0000-7001-8000-000000000d21",
      "correlation_id": "018f0000-0000-7001-8000-000000000d20",
      "effect": {
        "author_action_sequence": "4",
        "kind": "resolved",
        "operation_ids": ["018f0000-0000-7001-8000-000000000b04"],
        "preserved_closure": "open",
        "preserved_generation": "ready",
        "prior_resolution": "rejected",
        "rejection_event_refs": ["018f0000-0000-7001-8000-000000000d15"],
        "resulting_proposal_revision_id": "018f0000-0000-7001-8000-000000000b09",
        "resulting_resolution": "pending",
        "resulting_validation": "pending",
        "state_event_refs": ["018f0000-0000-7001-8000-000000000d25"],
        "undo_disposition": "forward"
      },
      "project": {
        "open": {
          "current_chapter_id": "018f0000-0000-7001-8000-000000000301",
          "kind": "current_chapter"
        },
        "project_id": "018f0000-0000-7001-8000-000000000201",
        "title": "Open Proposal Novel"
      },
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
      },
      "receipt": {
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000d22",
        "authoritative_commit_ids": [],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.reopenRejectedOperations.jcs.v1",
          "value_hex_lowercase": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
        },
        "created_at": "2026-09-19T12:00:00.000Z",
        "expected_target_revisions": ["018f0000-0000-7001-8000-000000000b06"],
        "idempotency_key": "018f0000-0000-7001-8000-000000000d24",
        "prior_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000b06"],
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "proposal_id": "018f0000-0000-7001-8000-000000000b02",
        "receipt_id": "018f0000-0000-7001-8000-000000000d23",
        "rejection_event_refs": ["018f0000-0000-7001-8000-000000000d15"],
        "result": "resolved",
        "resulting_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000b06"],
        "resulting_proposal_revision_id": "018f0000-0000-7001-8000-000000000b09",
        "selected_rejected_operation_ids": ["018f0000-0000-7001-8000-000000000b04"],
        "source_proposal_revision_id": "018f0000-0000-7001-8000-000000000b03"
      },
      "schema_id": "storyos.command.reopen-rejected-operations.response.v1"
    }"#,
    ),
};

const REPLAN_PROPOSAL_CAUSES: CauseProfiles = CauseProfiles {
    author: AuthorCause {
        semantic_payload: "explicit author ReplanProposal cause",
        request_schema: "storyos.command.replan-proposal.author-cause.request.v1",
        response_schema: "storyos.command.replan-proposal.author-cause.response.v1",
    },
    current_producer: CurrentProducerCause {
        semantic_cause: "Exact current Proposal Revision producer: AgentRunDecision",
        semantic_payload: "exact current producer ReplanProposal cause",
        cause_binding: "exact_current_proposal_revision_producer",
        request_schema: "storyos.command.replan-proposal.current-producer-cause.request.v1",
        response_schema: "storyos.command.replan-proposal.current-producer-cause.response.v1",
        recovery_rule: "The request binds the exact current AgentRunDecision producer and never fabricates an Editor Session, writer generation, or AuthorCommandAdmission.",
    },
};

const REPLAN_PROPOSAL_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/ReplanProposalInput/properties/conflicted_proposal_revision_id",
            "/$defs/ReplanProposalInput/properties/correlation_id",
            "/$defs/ReplanProposalInput/properties/editor_session_id",
            "/$defs/ReplanProposalInput/properties/expected_current_proposal_head",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const REPLAN_PROPOSAL_RESPONSE: &[Patch] = &[
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

const REOPEN_WITHDRAWN_PROPOSAL_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/ReopenWithdrawnProposalInput/properties/correlation_id",
            "/$defs/ReopenWithdrawnProposalInput/properties/editor_session_id",
            "/$defs/ReopenWithdrawnProposalInput/properties/proposal_revision_id",
            "/$defs/ReopenWithdrawnProposalInput/properties/withdrawal_event_ref",
        ],
    ),
    Patch(
        Edit::Const("withdrawn"),
        &["/$defs/ReopenWithdrawnProposalInput/properties/expected_closure"],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const REOPEN_WITHDRAWN_PROPOSAL_RESPONSE: &[Patch] = &[
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

const REOPEN_REJECTED_OPERATIONS_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/ReopenRejectedOperationsInput/properties/correlation_id",
            "/$defs/ReopenRejectedOperationsInput/properties/editor_session_id",
            "/$defs/ReopenRejectedOperationsInput/properties/proposal_revision_id",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const REOPEN_REJECTED_OPERATIONS_RESPONSE: &[Patch] = &[
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
