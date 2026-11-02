//! The rows of the Proposal decision commands in the operation table.

use ts_rs::TS;

use super::{
    Edit, Fixture, Headers, OperationRow, Patch, SCHEMA_ID, SchemaSource, SchemaSources, UUID,
    raw_schema,
};
use crate::release1_accept_proposal::{
    ACCEPT_PROPOSAL, AcceptProposalConflictReason, AcceptProposalEffect, AcceptProposalInput,
    AcceptProposalInvalidReason, AcceptProposalRefusalReason, AcceptProposalRequest,
    AcceptProposalResponse, AcceptanceReceipt, AcceptanceReceiptResult, AcceptanceTargetResolution,
};
use crate::release1_operation_registry::{ClientOverride, RegisteredOperation};
use crate::release1_reject_proposal_operations::{
    AuthorUndoDisposition, BoundedAuthorNote, ProposalRejectionReason, REJECT_PROPOSAL_OPERATIONS,
    RejectProposalOperationsConflictReason, RejectProposalOperationsEffect,
    RejectProposalOperationsInput, RejectProposalOperationsRefusalReason,
    RejectProposalOperationsRequest, RejectProposalOperationsResponse, RejectionReceipt,
    RejectionReceiptResult,
};
use crate::release1_route_catalog::{
    AuthorCause, Binding, CauseProfiles, CurrentProducerCause, OperationCatalog, Owner,
};
use crate::release1_withdraw_proposal::{
    AgentRunDecisionKind, AgentRunDecisionProducer, AuthorWithdrawalReason,
    CurrentProducerWithdrawalReason, ProposalWithdrawalReason, WITHDRAW_PROPOSAL,
    WithdrawProposalConflictReason, WithdrawProposalEffect, WithdrawProposalInput,
    WithdrawProposalNoEffectReason, WithdrawProposalRefusalReason, WithdrawProposalRequest,
    WithdrawProposalResponse, WithdrawalReceipt, WithdrawalReceiptResult,
};

pub(super) const ACCEPT_PROPOSAL_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &ACCEPT_PROPOSAL,
        &[
            "server_derived_project_scope",
            "project_active",
            "editor_session_writer_generation",
            "current_open_ready_proposal_revision",
            "valid_validation_receipt",
            "selected_pending_operations",
            "expected_target_revisions",
            "acceptance_admission",
        ],
        OperationCatalog::new("proposal:accept", Owner::Core, Binding::EDITOR_COMMAND)
            .notes(ACCEPT_PROPOSAL_NOTES),
    ),
    summary: "Accept a permitted pending Proposal Operation set",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "accept-proposal-request",
            raw_schema::<AcceptProposalRequest>,
            "StoryOS Accept Proposal Request",
            ACCEPT_PROPOSAL_REQUEST,
        ),
        SchemaSource::typed(
            "accept-proposal-response",
            raw_schema::<AcceptProposalResponse>,
            "StoryOS Accept Proposal Response",
            ACCEPT_PROPOSAL_RESPONSE,
        ),
    ),
    typescript: &[
        AcceptProposalInput::decl,
        AcceptProposalRequest::decl,
        AcceptanceReceiptResult::decl,
        AcceptanceReceipt::decl,
        AcceptProposalRefusalReason::decl,
        AcceptProposalInvalidReason::decl,
        AcceptProposalConflictReason::decl,
        AcceptanceTargetResolution::decl,
        AcceptProposalEffect::decl,
        AcceptProposalResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000c12",
      "command_id": "018f0000-0000-7001-8000-000000000c11",
      "correlation_id": "018f0000-0000-7001-8000-000000000c10",
      "effect": {
        "author_action_sequence": "2",
        "authoritative_commit_id": "018f0000-0000-7001-8000-000000000c16",
        "authoritative_revision": {
          "blocks": [
            {
              "block_kind": "paragraph",
              "manuscript_block_id": "018f0000-0000-7001-8000-000000000b05",
              "text": "Keep the narrator voice in this passage."
            }
          ],
          "body": "Keep the narrator voice in this passage.",
          "revision_id": "018f0000-0000-7001-8000-000000000c15"
        },
        "kind": "applied",
        "project_activity_position": "3",
        "target_resolution": "exact_base"
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
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000c12",
        "authoritative_commit_ids": ["018f0000-0000-7001-8000-000000000c16"],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.acceptProposal.jcs.v1",
          "value_hex_lowercase": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
        },
        "condition_refs": [],
        "created_at": "2026-09-18T12:00:00.000Z",
        "idempotency_key": "018f0000-0000-7001-8000-000000000c14",
        "prior_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000b06"],
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "proposal_id": "018f0000-0000-7001-8000-000000000b02",
        "proposal_revision_id": "018f0000-0000-7001-8000-000000000b03",
        "receipt_id": "018f0000-0000-7001-8000-000000000c13",
        "result": "applied",
        "resulting_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000c15"],
        "selected_operation_ids": ["018f0000-0000-7001-8000-000000000b04"],
        "validation_receipt_id": "018f0000-0000-7001-8000-000000000b08"
      },
      "schema_id": "storyos.command.accept-proposal.response.v1"
    }"#,
    ),
};

pub(super) const REJECT_PROPOSAL_OPERATIONS_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &REJECT_PROPOSAL_OPERATIONS,
        &[
            "server_derived_project_scope",
            "project_active",
            "editor_session_writer_generation",
            "current_open_ready_proposal_revision",
            "selected_pending_operations",
            "expected_target_revisions",
            "explicit_editor_control",
        ],
        OperationCatalog::new(
            "proposal:reject-operations",
            Owner::Core,
            Binding::EDITOR_COMMAND,
        ),
    ),
    summary: "Reject one pending Proposal Operation",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "reject-proposal-operations-request",
            raw_schema::<RejectProposalOperationsRequest>,
            "StoryOS Reject Proposal Operations Request",
            REJECT_PROPOSAL_OPERATIONS_REQUEST,
        ),
        SchemaSource::typed(
            "reject-proposal-operations-response",
            raw_schema::<RejectProposalOperationsResponse>,
            "StoryOS Reject Proposal Operations Response",
            REJECT_PROPOSAL_OPERATIONS_RESPONSE,
        ),
    ),
    typescript: &[
        BoundedAuthorNote::decl,
        ProposalRejectionReason::decl,
        RejectProposalOperationsInput::decl,
        RejectProposalOperationsRequest::decl,
        RejectionReceiptResult::decl,
        RejectionReceipt::decl,
        RejectProposalOperationsRefusalReason::decl,
        RejectProposalOperationsConflictReason::decl,
        AuthorUndoDisposition::decl,
        RejectProposalOperationsEffect::decl,
        RejectProposalOperationsResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000d12",
      "command_id": "018f0000-0000-7001-8000-000000000d11",
      "correlation_id": "018f0000-0000-7001-8000-000000000d10",
      "effect": {
        "author_action_sequence": "3",
        "kind": "resolved",
        "operation_ids": ["018f0000-0000-7001-8000-000000000b04"],
        "preserved_closure": "open",
        "preserved_generation": "ready",
        "preserved_validation": "valid",
        "prior_resolution": "pending",
        "rejection_reason": {"kind": "author_declined", "note": {"kind": "omitted"}},
        "resolution_event_refs": ["018f0000-0000-7001-8000-000000000d15"],
        "resulting_resolution": "rejected",
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
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000d12",
        "authoritative_commit_ids": [],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.rejectProposalOperations.jcs.v1",
          "value_hex_lowercase": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
        },
        "created_at": "2026-09-19T12:00:00.000Z",
        "expected_target_revisions": ["018f0000-0000-7001-8000-000000000b06"],
        "idempotency_key": "018f0000-0000-7001-8000-000000000d14",
        "prior_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000b06"],
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "proposal_id": "018f0000-0000-7001-8000-000000000b02",
        "proposal_revision_id": "018f0000-0000-7001-8000-000000000b03",
        "receipt_id": "018f0000-0000-7001-8000-000000000d13",
        "result": "resolved",
        "resulting_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000b06"],
        "selected_pending_operation_ids": ["018f0000-0000-7001-8000-000000000b04"]
      },
      "schema_id": "storyos.command.reject-proposal-operations.response.v1"
    }"#,
    ),
};

pub(super) const WITHDRAW_PROPOSAL_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &WITHDRAW_PROPOSAL,
        &[
            "proposal_scope_join",
            "current_open_proposal_revision",
            "expected_target_revisions",
            "author_or_exact_current_producer_cause",
        ],
        OperationCatalog::conditional_by_cause(
            "proposal:withdraw",
            Owner::Core,
            &WITHDRAW_PROPOSAL_CAUSES,
        ),
    )
    .client(ClientOverride::OptionalAntiForgery),
    summary: "Withdraw an open Proposal",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "withdraw-proposal-request",
            raw_schema::<WithdrawProposalRequest>,
            "StoryOS Withdraw Proposal Request",
            WITHDRAW_PROPOSAL_REQUEST,
        ),
        SchemaSource::typed(
            "withdraw-proposal-response",
            raw_schema::<WithdrawProposalResponse>,
            "StoryOS Withdraw Proposal Response",
            WITHDRAW_PROPOSAL_RESPONSE,
        ),
    ),
    typescript: &[
        AgentRunDecisionKind::decl,
        AgentRunDecisionProducer::decl,
        AuthorWithdrawalReason::decl,
        CurrentProducerWithdrawalReason::decl,
        ProposalWithdrawalReason::decl,
        WithdrawProposalInput::decl,
        WithdrawProposalRequest::decl,
        WithdrawalReceiptResult::decl,
        WithdrawalReceipt::decl,
        WithdrawProposalRefusalReason::decl,
        WithdrawProposalConflictReason::decl,
        WithdrawProposalNoEffectReason::decl,
        WithdrawProposalEffect::decl,
        WithdrawProposalResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000e32",
      "command_id": "018f0000-0000-7001-8000-000000000e31",
      "correlation_id": "018f0000-0000-7001-8000-000000000e30",
      "effect": {
        "author_action_sequence": "6",
        "closure_event_refs": ["018f0000-0000-7001-8000-000000000e35"],
        "kind": "resolved",
        "preserved_generation": "ready",
        "preserved_validation": "conflicted",
        "prior_closure": "open",
        "resulting_closure": "withdrawn",
        "undo_disposition": "forward",
        "withdrawal_reason": {"kind": "author_withdrew", "note": {"kind": "omitted"}}
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
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000e32",
        "authoritative_commit_ids": [],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.withdrawProposal.jcs.v1",
          "value_hex_lowercase": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
        },
        "created_at": "2026-09-28T12:00:00.000Z",
        "expected_target_revisions": ["018f0000-0000-7001-8000-000000000b06"],
        "idempotency_key": "018f0000-0000-7001-8000-000000000e34",
        "prior_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000b06"],
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "proposal_id": "018f0000-0000-7001-8000-000000000b02",
        "proposal_revision_id": "018f0000-0000-7001-8000-000000000b03",
        "receipt_id": "018f0000-0000-7001-8000-000000000e33",
        "result": "resolved",
        "resulting_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000b06"]
      },
      "schema_id": "storyos.command.withdraw-proposal.response.v1"
    }"#,
    ),
};

const ACCEPT_PROPOSAL_NOTES: &str = "Acceptance is the explicit author command; it never automatically replans or replaces validation.";

const WITHDRAW_PROPOSAL_CAUSES: CauseProfiles = CauseProfiles {
    author: AuthorCause {
        semantic_payload: "withdrawal_reason: AuthorWithdrew",
        request_schema: "storyos.command.withdraw-proposal.author-cause.request.v1",
        response_schema: "storyos.command.withdraw-proposal.author-cause.response.v1",
    },
    current_producer: CurrentProducerCause {
        semantic_cause: "Exact producer of the Proposal: its source AgentRun and Decision",
        semantic_payload: "withdrawal_reason: CurrentProducerWithdrew",
        cause_binding: "exact_proposal_producer",
        request_schema: "storyos.command.withdraw-proposal.current-producer-cause.request.v1",
        response_schema: "storyos.command.withdraw-proposal.current-producer-cause.response.v1",
        recovery_rule: "The request binds the exact producer of the Proposal, its source AgentRun and Decision, and never fabricates an Editor Session, writer generation, or AuthorCommandAdmission.",
    },
};

const ACCEPT_PROPOSAL_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/AcceptProposalInput/properties/correlation_id",
            "/$defs/AcceptProposalInput/properties/editor_session_id",
            "/$defs/AcceptProposalInput/properties/expected_authoritative_revision_id",
            "/$defs/AcceptProposalInput/properties/proposal_revision_id",
            "/$defs/AcceptProposalInput/properties/selected_operation_ids/items",
            "/$defs/AcceptProposalInput/properties/validation_receipt_id",
        ],
    ),
    Patch(
        Edit::MinItems(1),
        &["/$defs/AcceptProposalInput/properties/selected_operation_ids"],
    ),
    Patch(
        Edit::UniqueItems,
        &["/$defs/AcceptProposalInput/properties/selected_operation_ids"],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const ACCEPT_PROPOSAL_RESPONSE: &[Patch] = &[
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

const REJECT_PROPOSAL_OPERATIONS_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/RejectProposalOperationsInput/properties/correlation_id",
            "/$defs/RejectProposalOperationsInput/properties/editor_session_id",
            "/$defs/RejectProposalOperationsInput/properties/proposal_revision_id",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const REJECT_PROPOSAL_OPERATIONS_RESPONSE: &[Patch] = &[
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

const WITHDRAW_PROPOSAL_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/AgentRunDecisionProducer/properties/decision_id",
            "/$defs/AgentRunDecisionProducer/properties/run_id",
            "/$defs/WithdrawProposalInput/oneOf/0/properties/correlation_id",
            "/$defs/WithdrawProposalInput/oneOf/0/properties/editor_session_id",
            "/$defs/WithdrawProposalInput/oneOf/0/properties/proposal_revision_id",
            "/$defs/WithdrawProposalInput/oneOf/1/properties/correlation_id",
            "/$defs/WithdrawProposalInput/oneOf/1/properties/proposal_revision_id",
        ],
    ),
    Patch(
        Edit::Const("open"),
        &[
            "/$defs/WithdrawProposalInput/oneOf/0/properties/expected_closure",
            "/$defs/WithdrawProposalInput/oneOf/1/properties/expected_closure",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const WITHDRAW_PROPOSAL_RESPONSE: &[Patch] = &[
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
