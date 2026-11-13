//! The rows of the Proposal queries in the operation table.

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
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};

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
    schemas: SchemaSources::get(SchemaSource::typed(
        "proposal-response",
        raw_schema::<GetProposalResponse>,
        "StoryOS Proposal Response",
        GET_PROPOSAL_RESPONSE,
    )),
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
    schemas: SchemaSources::get(SchemaSource::typed(
        "chapter-open-proposals-response",
        raw_schema::<ListChapterOpenProposalsResponse>,
        "StoryOS Chapter Open Proposals Response",
        LIST_CHAPTER_OPEN_PROPOSALS_RESPONSE,
    )),
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
