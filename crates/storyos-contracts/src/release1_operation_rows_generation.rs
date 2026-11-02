//! The rows of the Proposal generation commands and Proposal queries in the operation table.

use ts_rs::TS;

use super::{
    Edit, Fixture, Headers, OperationRow, Patch, SCHEMA_ID, SchemaSource, SchemaSources, UUID,
    raw_schema,
};
use crate::release1_open_proposal_list::{
    LIST_CHAPTER_OPEN_PROPOSALS, ListChapterOpenProposalsResponse, OpenProposalEntry,
    OpenProposalListCompleteness, OpenProposalSource,
};
use crate::release1_operation_registry::RegisteredOperation;
use crate::release1_proposal::{
    AcceptanceRefusalBoundary, AcceptanceRefusalInspect, AcceptanceRefusalReason,
    BlockProposalInspect, GET_PROPOSAL, GetProposalResponse, OptionalAcceptanceRefusalInspect,
    OptionalRevisionComparisonInspect, OptionalValidationReceiptInspect, ProposalOperationInspect,
    ProposalSourceConditionInspect, ProposalSourceInspect, ReplacementSpanInspect,
};
use crate::release1_proposal_generation_decision::{
    COMPLETE_READY_PARTIAL_PROPOSAL, CONTINUE_PROPOSAL_GENERATION,
    CompleteReadyPartialProposalEffect, CompleteReadyPartialProposalInput,
    CompleteReadyPartialProposalRefusalReason, CompleteReadyPartialProposalRequest,
    CompleteReadyPartialProposalResponse, ContinueProposalGenerationEffect,
    ContinueProposalGenerationInput, ContinueProposalGenerationRefusalReason,
    ContinueProposalGenerationRequest, ContinueProposalGenerationResponse,
    ProposalGenerationConflictReason, ProposalGenerationReceipt, ProposalGenerationReceiptResult,
    ProposalGenerationUndoDisposition,
};
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};

pub(super) const COMPLETE_READY_PARTIAL_PROPOSAL_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &COMPLETE_READY_PARTIAL_PROPOSAL,
        &[
            "server_derived_project_scope",
            "project_active",
            "editor_session_writer_generation",
            "current_ready_partial_generation",
            "expected_candidate_digest",
            "last_applied_stream_seq",
            "expected_target_revisions",
            "explicit_editor_control",
        ],
        OperationCatalog::new(
            "proposal:generation-complete",
            Owner::Core,
            Binding::EDITOR_COMMAND,
        ),
    ),
    summary: "Complete one ready partial Proposal",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "complete-ready-partial-proposal-request",
            raw_schema::<CompleteReadyPartialProposalRequest>,
            "StoryOS Complete Ready Partial Proposal Request",
            COMPLETE_READY_PARTIAL_PROPOSAL_REQUEST,
        ),
        SchemaSource::typed(
            "complete-ready-partial-proposal-response",
            raw_schema::<CompleteReadyPartialProposalResponse>,
            "StoryOS Complete Ready Partial Proposal Response",
            COMPLETE_READY_PARTIAL_PROPOSAL_RESPONSE,
        ),
    ),
    typescript: &[
        CompleteReadyPartialProposalInput::decl,
        CompleteReadyPartialProposalRequest::decl,
        ContinueProposalGenerationInput::decl,
        ContinueProposalGenerationRequest::decl,
        ProposalGenerationReceiptResult::decl,
        ProposalGenerationReceipt::decl,
        CompleteReadyPartialProposalRefusalReason::decl,
        ContinueProposalGenerationRefusalReason::decl,
        ProposalGenerationConflictReason::decl,
        ProposalGenerationUndoDisposition::decl,
        CompleteReadyPartialProposalEffect::decl,
        ContinueProposalGenerationEffect::decl,
        CompleteReadyPartialProposalResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000e12",
      "command_id": "018f0000-0000-7001-8000-000000000e11",
      "correlation_id": "018f0000-0000-7001-8000-000000000e10",
      "effect": {
        "author_action_sequence": "4",
        "generation_event_ref": "018f0000-0000-7001-8000-000000000e17",
        "generation_id": "018f0000-0000-7001-8000-000000000e16",
        "kind": "completed",
        "preserved_closure": "open",
        "preserved_operation_resolution": "pending",
        "preserved_validation": "pending",
        "prior_generation_state": "ready_partial",
        "resulting_generation_state": "ready",
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
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000e12",
        "authoritative_commit_ids": [],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.completeReadyPartialProposal.jcs.v1",
          "value_hex_lowercase": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
        },
        "created_at": "2026-09-25T12:00:00.000Z",
        "expected_target_revisions": ["018f0000-0000-7001-8000-000000000b06"],
        "idempotency_key": "018f0000-0000-7001-8000-000000000e14",
        "prior_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000b06"],
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "proposal_id": "018f0000-0000-7001-8000-000000000b02",
        "proposal_revision_id": "018f0000-0000-7001-8000-000000000b03",
        "receipt_id": "018f0000-0000-7001-8000-000000000e13",
        "result": "proposal_generation_completed",
        "resulting_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000b06"]
      },
      "schema_id": "storyos.command.complete-ready-partial-proposal.response.v1"
    }"#,
    ),
};

pub(super) const CONTINUE_PROPOSAL_GENERATION_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &CONTINUE_PROPOSAL_GENERATION,
        &[
            "server_derived_project_scope",
            "project_active",
            "editor_session_writer_generation",
            "current_generation_ready_partial_or_ready",
            "prior_generation_id",
            "expected_candidate_digest",
            "selected_pending_operations",
            "expected_target_revisions",
            "explicit_editor_control",
        ],
        OperationCatalog::new(
            "proposal:generation-continue",
            Owner::Core,
            Binding::EDITOR_COMMAND,
        ),
    ),
    summary: "Continue one Proposal Generation",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "continue-proposal-generation-request",
            raw_schema::<ContinueProposalGenerationRequest>,
            "StoryOS Continue Proposal Generation Request",
            CONTINUE_PROPOSAL_GENERATION_REQUEST,
        ),
        SchemaSource::typed(
            "continue-proposal-generation-response",
            raw_schema::<ContinueProposalGenerationResponse>,
            "StoryOS Continue Proposal Generation Response",
            CONTINUE_PROPOSAL_GENERATION_RESPONSE,
        ),
    ),
    typescript: &[ContinueProposalGenerationResponse::decl],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000e12",
      "command_id": "018f0000-0000-7001-8000-000000000e11",
      "correlation_id": "018f0000-0000-7001-8000-000000000e20",
      "effect": {
        "author_action_sequence": "5",
        "generation_event_ref": "018f0000-0000-7001-8000-000000000e2a",
        "kind": "started",
        "new_generation_id": "018f0000-0000-7001-8000-000000000e27",
        "preserved_closure": "open",
        "preserved_operation_resolution": "pending",
        "preserved_validation": "pending",
        "prior_generation_id": "018f0000-0000-7001-8000-000000000e26",
        "prior_generation_state": "ready_partial",
        "prior_run_id": "018f0000-0000-7001-8000-000000000e28",
        "resulting_generation_state": "generating",
        "resulting_run_id": "018f0000-0000-7001-8000-000000000e29",
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
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000e12",
        "authoritative_commit_ids": [],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.continueProposalGeneration.jcs.v1",
          "value_hex_lowercase": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
        },
        "created_at": "2026-09-25T12:00:00.000Z",
        "expected_target_revisions": ["018f0000-0000-7001-8000-000000000b06"],
        "idempotency_key": "018f0000-0000-7001-8000-000000000e14",
        "prior_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000b06"],
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "proposal_id": "018f0000-0000-7001-8000-000000000b02",
        "proposal_revision_id": "018f0000-0000-7001-8000-000000000b03",
        "receipt_id": "018f0000-0000-7001-8000-000000000e13",
        "result": "proposal_generation_started",
        "resulting_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000b06"]
      },
      "schema_id": "storyos.command.continue-proposal-generation.response.v1"
    }"#,
    ),
};

pub(super) const GET_PROPOSAL_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
        &GET_PROPOSAL,
        &[
            "proposal_scope_join",
            "exact_revision_or_current_projection",
            "redaction_profile",
        ],
        OperationCatalog::new("proposal:read", Owner::Core, Binding::CANONICAL_QUERY),
    ),
    summary: "Inspect one current Proposal",
    headers: Headers::NONE,
    schemas: SchemaSources::new(
        SchemaSource::titled_path_parameters("proposal-request", "StoryOS Proposal Request"),
        SchemaSource::typed(
            "proposal-response",
            raw_schema::<GetProposalResponse>,
            "StoryOS Proposal Response",
            GET_PROPOSAL_RESPONSE,
        ),
    ),
    typescript: &[
        ProposalSourceInspect::decl,
        ProposalSourceConditionInspect::decl,
        OptionalValidationReceiptInspect::decl,
        AcceptanceRefusalReason::decl,
        AcceptanceRefusalBoundary::decl,
        AcceptanceRefusalInspect::decl,
        OptionalAcceptanceRefusalInspect::decl,
        ProposalOperationInspect::decl,
        ReplacementSpanInspect::decl,
        OptionalRevisionComparisonInspect::decl,
        BlockProposalInspect::decl,
        GetProposalResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "correlation_id": "018f0000-0000-7001-8000-000000000b01",
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
      },
      "proposal": {
        "base_authoritative_revision_id": "018f0000-0000-7001-8000-000000000b06",
        "bundle_policy": "none",
        "candidate_text": "Guard the narrator voice in this passage.",
        "chapter_id": "018f0000-0000-7001-8000-000000000301",
        "closure": "open",
        "condition_refs": [],
        "generation": "ready",
        "kind": "block_edit",
        "latest_acceptance_refusal": {"kind": "absent"},
        "manuscript_block_id": "018f0000-0000-7001-8000-000000000b05",
        "operation_id": "018f0000-0000-7001-8000-000000000b04",
        "operation_resolution": "pending",
        "operations": [
          {
            "candidate_text": "Guard the narrator voice in this passage.",
            "manuscript_block_id": "018f0000-0000-7001-8000-000000000b05",
            "operation_id": "018f0000-0000-7001-8000-000000000b04",
            "reservation_state": "unresolved",
            "resolution": "pending"
          }
        ],
        "proposal_id": "018f0000-0000-7001-8000-000000000b02",
        "reservation_state": "unresolved",
        "revision_comparison": {"kind": "absent"},
        "revision_id": "018f0000-0000-7001-8000-000000000b03",
        "source": {
          "decision_id": "018f0000-0000-7001-8000-000000000b07",
          "kind": "agent_run_decision",
          "run_id": "018f0000-0000-7001-8000-000000000a34"
        },
        "source_condition": {"kind": "absent"},
        "validation": "valid",
        "validation_receipt": {
          "kind": "present",
          "result": "valid",
          "validation_receipt_id": "018f0000-0000-7001-8000-000000000b08"
        }
      },
      "schema_id": "storyos.query.proposal.response.v1"
    }"#,
    ),
};

pub(super) const LIST_CHAPTER_OPEN_PROPOSALS_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
        &LIST_CHAPTER_OPEN_PROPOSALS,
        &[
            "server_derived_project_scope",
            "chapter_scope_join",
            "bounded_open_proposal_list",
        ],
        OperationCatalog::new(
            "proposal:chapter-open-list",
            Owner::Core,
            Binding::CANONICAL_QUERY,
        ),
    ),
    summary: "List the open Proposals of one Chapter",
    headers: Headers::NONE,
    schemas: SchemaSources::new(
        SchemaSource::titled_path_parameters(
            "chapter-open-proposals-request",
            "StoryOS Chapter Open Proposals Request",
        ),
        SchemaSource::typed(
            "chapter-open-proposals-response",
            raw_schema::<ListChapterOpenProposalsResponse>,
            "StoryOS Chapter Open Proposals Response",
            LIST_CHAPTER_OPEN_PROPOSALS_RESPONSE,
        ),
    ),
    typescript: &[
        OpenProposalSource::decl,
        OpenProposalEntry::decl,
        OpenProposalListCompleteness::decl,
        ListChapterOpenProposalsResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "chapter_id": "018f0000-0000-7001-8000-000000000301",
      "completeness": "complete",
      "correlation_id": "018f0000-0000-7001-8000-000000000c01",
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
      },
      "proposals": [
        {
          "kind": "block_edit",
          "proposal_id": "018f0000-0000-7001-8000-000000000b02",
          "revision_id": "018f0000-0000-7001-8000-000000000b03",
          "source": {
            "decision_id": "018f0000-0000-7001-8000-000000000b07",
            "kind": "agent_run_decision",
            "run_id": "018f0000-0000-7001-8000-000000000a34"
          }
        },
        {
          "kind": "block_edit",
          "proposal_id": "018f0000-0000-7001-8000-000000000c02",
          "revision_id": "018f0000-0000-7001-8000-000000000c03",
          "source": {
            "decision_id": "018f0000-0000-7001-8000-000000000c05",
            "kind": "agent_run_decision",
            "run_id": "018f0000-0000-7001-8000-000000000c04"
          }
        }
      ],
      "schema_id": "storyos.query.chapter-open-proposals.response.v1"
    }"#,
    ),
};

const COMPLETE_READY_PARTIAL_PROPOSAL_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/CompleteReadyPartialProposalInput/properties/correlation_id",
            "/$defs/CompleteReadyPartialProposalInput/properties/editor_session_id",
            "/$defs/CompleteReadyPartialProposalInput/properties/generation_id",
            "/$defs/CompleteReadyPartialProposalInput/properties/proposal_revision_id",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const COMPLETE_READY_PARTIAL_PROPOSAL_RESPONSE: &[Patch] = &[
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

const CONTINUE_PROPOSAL_GENERATION_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/ContinueProposalGenerationInput/properties/correlation_id",
            "/$defs/ContinueProposalGenerationInput/properties/editor_session_id",
            "/$defs/ContinueProposalGenerationInput/properties/prior_generation_id",
            "/$defs/ContinueProposalGenerationInput/properties/proposal_revision_id",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const CONTINUE_PROPOSAL_GENERATION_RESPONSE: &[Patch] = &[
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

const GET_PROPOSAL_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/BlockProposalInspect/properties/base_authoritative_revision_id",
            "/$defs/BlockProposalInspect/properties/chapter_id",
            "/$defs/BlockProposalInspect/properties/manuscript_block_id",
            "/$defs/BlockProposalInspect/properties/operation_id",
            "/$defs/BlockProposalInspect/properties/proposal_id",
            "/$defs/BlockProposalInspect/properties/revision_id",
            "/$defs/OptionalRevisionComparisonInspect/oneOf/1/properties/base_authoritative_revision_id",
            "/$defs/OptionalRevisionComparisonInspect/oneOf/1/properties/candidate_revision_id",
            "/$defs/OptionalRevisionComparisonInspect/oneOf/1/properties/operation_id",
            "/$defs/OptionalValidationReceiptInspect/properties/validation_receipt_id",
            "/$defs/ProjectScope/properties/owner_user_id",
            "/$defs/ProjectScope/properties/project_id",
            "/$defs/ProposalOperationInspect/properties/manuscript_block_id",
            "/$defs/ProposalOperationInspect/properties/operation_id",
            "/$defs/ProposalSourceInspect/properties/decision_id",
            "/$defs/ProposalSourceInspect/properties/run_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(
        Edit::Enum(&["none", "atomic"]),
        &["/$defs/BlockProposalInspect/properties/bundle_policy"],
    ),
    Patch(
        Edit::Enum(&["block_edit", "reversal"]),
        &["/$defs/BlockProposalInspect/properties/kind"],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];

const LIST_CHAPTER_OPEN_PROPOSALS_RESPONSE: &[Patch] = &[
    Patch(
        Edit::Enum(&["block_edit", "reversal"]),
        &["/$defs/OpenProposalEntry/properties/kind"],
    ),
    Patch(
        UUID,
        &[
            "/$defs/OpenProposalEntry/properties/proposal_id",
            "/$defs/OpenProposalEntry/properties/revision_id",
            "/$defs/OpenProposalSource/oneOf/0/properties/decision_id",
            "/$defs/OpenProposalSource/oneOf/0/properties/run_id",
            "/$defs/ProjectScope/properties/owner_user_id",
            "/$defs/ProjectScope/properties/project_id",
            "/properties/chapter_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(Edit::MaxItems(64), &["/properties/proposals"]),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];
