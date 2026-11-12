//! The rows of the AgentRun start and inspection operations in the operation table.

use ts_rs::TS;

use super::{
    Edit, Fixture, Headers, OperationRow, Patch, QueryValue, SCHEMA_ID, SchemaSource,
    SchemaSources, U64, UUID, raw_schema,
};
use crate::release1_agent_run::{
    ActiveCompactionInstallState, ActiveCompactionKnownInput, ActiveCompactionMappingKind,
    AgentRunContextInspect, AgentRunRef, AgentRunStatus, AgentRunSteeringInspect,
    AgentRunStreamItemInspect, AgentRunUsageInspect, AssistanceCause, AssistanceWorkingTarget,
    AttemptCacheInspect, AttemptEvidence, AuthorMessage, CREATE_AGENT_RUN,
    CapturedMemorySettingsInspect, ContextBlockReason, ContextProjectionInspect, ContextPurpose,
    ContextRejectionInspect, ContextRejectionReason, ContextSourceClass, ContextSourceInspect,
    ContextSufficiency, ContinuationAdmissionInspect, ContinuationInputMappingInspect,
    ConversationSelection, CreateAgentRunEffect, CreateAgentRunInput, CreateAgentRunRequest,
    CreateAgentRunResponse, CurrentAvailabilityInspect, DestinationIo, EvidenceAvailability,
    GET_AGENT_RUN, GetAgentRunRequest, GetAgentRunResponse, HostControlInspect, InstructionBinding,
    OptionalActiveCompactionInspect, OptionalCompactionInstallInspect, OptionalContinuationInspect,
    OptionalDecisionInspect, OptionalManifestRef, OptionalModelAttemptInspect,
    OptionalOpenedProposalInspect, OptionalOriginalResultRetrievalInspect,
    OptionalReferenceRecoveryInspect, OptionalUnknownCreateSuccessorInspect,
    OriginalResultRetrievalDisposition, PassageTarget, ProjectionMode, ProposalCandidateTarget,
    ProseChangeLocationCurrent, ProseChangeLocationInspect, ProseChangeLocationOutcome,
    ReferenceRecoveryDisposition, SourceAvailability, TokenCountingProfileInspect, ToolCallInspect,
    ToolCallOutcomeInspect, UnknownCreateSuccessorDisposition,
};
use crate::release1_operation_registry::{ClientOverride, RegisteredOperation};
use crate::release1_project_activity::ProjectActivityKind;
use crate::release1_route_catalog::{
    ActionClass, Admission, Binding, EditorRule, ErrorProfile, Idempotency, OperationCatalog,
    Owner, Settlement,
};
use crate::release1_selection_reference::{
    SelectionCoordinateProfile, SelectionPoint, SelectionReference, SelectionReferenceInspect,
};

pub(super) const CREATE_AGENT_RUN_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &CREATE_AGENT_RUN,
        &[
            "server_derived_project_scope",
            "operation_requirement",
            "working_target_or_explicit_not_applicable",
            "capability_and_destination_grant",
        ],
        OperationCatalog::new(
            "agent-run:create",
            Owner::Context,
            Binding {
                action_class: ActionClass::AgentRunStart,
                admission: Admission::ProducerCause,
                editor: EditorRule::Optional,
                idempotency: Idempotency::HeaderAndOperationRecord,
                settlement: Settlement::Accepted {
                    query: &GET_AGENT_RUN,
                    operation_ref: "agent_run",
                },
                error_profile: ErrorProfile::AsyncCommand,
            },
        )
        .activity(&[ProjectActivityKind::AgentRunCreated], &[])
        .notes(CREATE_AGENT_RUN_NOTES),
    ),
    summary: "Admit one bounded assistance request",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "create-agent-run-request",
            raw_schema::<CreateAgentRunRequest>,
            "StoryOS Create Agent Run Request",
            CREATE_AGENT_RUN_REQUEST,
        ),
        SchemaSource::typed(
            "create-agent-run-response",
            raw_schema::<CreateAgentRunResponse>,
            "StoryOS Create Agent Run Response",
            CREATE_AGENT_RUN_RESPONSE,
        ),
    ),
    typescript: &[
        ConversationSelection::decl,
        AuthorMessage::decl,
        AssistanceWorkingTarget::decl,
        PassageTarget::decl,
        SelectionReference::decl,
        SelectionCoordinateProfile::decl,
        SelectionPoint::decl,
        ProposalCandidateTarget::decl,
        InstructionBinding::decl,
        AssistanceCause::decl,
        CreateAgentRunInput::decl,
        CreateAgentRunRequest::decl,
        AgentRunRef::decl,
        CreateAgentRunEffect::decl,
        CreateAgentRunResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "acknowledgement": "accepted",
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000a33",
      "command_id": "018f0000-0000-7001-8000-000000000a32",
      "conversation_id": "018f0000-0000-7001-8000-000000000a36",
      "correlation_id": "018f0000-0000-7001-8000-000000000a31",
      "effect": {
        "conversation_id": "018f0000-0000-7001-8000-000000000a36",
        "kind": "admitted",
        "memory_settings_revision": "018f0000-0000-7001-8000-000000000a38",
        "project_activity_position": "1",
        "project_agent_id": "018f0000-0000-7001-8000-000000000a35",
        "run_id": "018f0000-0000-7001-8000-000000000a34"
      },
      "memory_settings_revision": "018f0000-0000-7001-8000-000000000a38",
      "operation_ref": {"kind": "agent_run", "run_id": "018f0000-0000-7001-8000-000000000a34"},
      "project": {
        "open": {
          "current_chapter_id": "018f0000-0000-7001-8000-000000000301",
          "kind": "current_chapter"
        },
        "project_id": "018f0000-0000-7001-8000-000000000201",
        "title": "Empty Novel"
      },
      "project_agent_id": "018f0000-0000-7001-8000-000000000a35",
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
      },
      "schema_id": "storyos.command.create-agent-run.response.v2"
    }"#,
    ),
};

pub(super) const GET_AGENT_RUN_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
        &GET_AGENT_RUN,
        &["run_scope_join", "run_projection_watermark_or_snapshot"],
        OperationCatalog::new("agent-run:read", Owner::Context, Binding::PROJECTION_QUERY)
            .notes(GET_AGENT_RUN_NOTES),
    )
    .client(ClientOverride::OptionalQueryParameter("model_attempt_id")),
    summary: "Inspect one durable AgentRun status",
    headers: Headers::NONE,
    schemas: SchemaSources::get(SchemaSource::typed(
        "agent-run-response",
        raw_schema::<GetAgentRunResponse>,
        "StoryOS Agent Run Response",
        GET_AGENT_RUN_RESPONSE,
    ))
    .query_value(QueryValue::Uuid),
    typescript: &[
        AgentRunStatus::decl,
        ContextPurpose::decl,
        ContextSourceClass::decl,
        ProjectionMode::decl,
        ContextSufficiency::decl,
        ContextBlockReason::decl,
        ContextRejectionReason::decl,
        DestinationIo::decl,
        OptionalManifestRef::decl,
        SourceAvailability::decl,
        TokenCountingProfileInspect::decl,
        ContextSourceInspect::decl,
        ContextProjectionInspect::decl,
        ContextRejectionInspect::decl,
        HostControlInspect::decl,
        CurrentAvailabilityInspect::decl,
        AgentRunContextInspect::decl,
        SelectionReferenceInspect::decl,
        EvidenceAvailability::decl,
        AttemptEvidence::decl,
        OptionalContinuationInspect::decl,
        OptionalOpenedProposalInspect::decl,
        ProseChangeLocationInspect::decl,
        ProseChangeLocationOutcome::decl,
        ProseChangeLocationCurrent::decl,
        ToolCallOutcomeInspect::decl,
        ToolCallInspect::decl,
        OptionalDecisionInspect::decl,
        ContinuationInputMappingInspect::decl,
        ContinuationAdmissionInspect::decl,
        OptionalModelAttemptInspect::decl,
        ActiveCompactionInstallState::decl,
        ActiveCompactionMappingKind::decl,
        ActiveCompactionKnownInput::decl,
        OptionalCompactionInstallInspect::decl,
        OptionalActiveCompactionInspect::decl,
        ReferenceRecoveryDisposition::decl,
        OptionalReferenceRecoveryInspect::decl,
        AgentRunStreamItemInspect::decl,
        AgentRunUsageInspect::decl,
        AttemptCacheInspect::decl,
        GetAgentRunRequest::decl,
        CapturedMemorySettingsInspect::decl,
        AgentRunSteeringInspect::decl,
        GetAgentRunResponse::decl,
        OriginalResultRetrievalDisposition::decl,
        OptionalOriginalResultRetrievalInspect::decl,
        UnknownCreateSuccessorDisposition::decl,
        OptionalUnknownCreateSuccessorInspect::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "active_compaction": {"kind": "absent"},
      "cache": {"kind": "not_recorded"},
      "captured_memory_settings": {
        "contribution_enabled": true,
        "kind": "available",
        "memory_settings_revision": "018f0000-0000-7001-8000-000000000a38",
        "use_enabled": true
      },
      "context": {
        "assembly_manifest_id": "018f0000-0000-7001-8000-000000000a3b",
        "considered": [
          {
            "eligible": true,
            "source_class": "host_control",
            "source_version": "018f0000-0000-7001-8000-000000000a39",
            "token_count": "0"
          },
          {
            "eligible": true,
            "source_class": "author_instruction",
            "source_version": "018f0000-0000-7001-8000-000000000a3a",
            "token_count": "23"
          },
          {
            "eligible": true,
            "source_class": "working_target",
            "source_version": "018f0000-0000-7001-8000-000000000301",
            "token_count": "0"
          },
          {
            "eligible": true,
            "source_class": "instruction_binding",
            "source_version": "absent",
            "token_count": "0"
          }
        ],
        "current_availability": {"working_target": {"kind": "current"}},
        "destination_context_manifest": {"kind": "absent"},
        "destination_io": {"kind": "none"},
        "host_control": {"destination_visible": false, "distinct_from_destination": true},
        "input_snapshot_id": "018f0000-0000-7001-8000-000000000a3a",
        "operation_requirement_id": "018f0000-0000-7001-8000-000000000a39",
        "outbound_disclosure_manifest": {"kind": "absent"},
        "purpose": "current_passage_assistance",
        "rejected": [],
        "selected": [
          {
            "content": "Help with this passage.",
            "projection_mode": "exact_required",
            "source_class": "author_instruction",
            "source_version": "018f0000-0000-7001-8000-000000000a3a",
            "token_count": "23"
          },
          {
            "content": "",
            "projection_mode": "exact_required",
            "source_class": "working_target",
            "source_version": "018f0000-0000-7001-8000-000000000301",
            "token_count": "0"
          }
        ],
        "sufficiency": {"kind": "complete"},
        "token_counting_profile": {
          "algorithm_revision": "storyos.statistics.unicode-16.0.0.v1",
          "item_token_limit": "32000",
          "profile_revision": "storyos.token-counting.unicode-scalar.v1"
        }
      },
      "conversation_id": "018f0000-0000-7001-8000-000000000a36",
      "correlation_id": "018f0000-0000-7001-8000-000000000a37",
      "decision": {"kind": "absent"},
      "evidence": [],
      "items": [],
      "memory_settings_revision": "018f0000-0000-7001-8000-000000000a38",
      "model_attempt": {"kind": "absent"},
      "original_result_retrieval": {"kind": "absent"},
      "project_agent_id": "018f0000-0000-7001-8000-000000000a35",
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
      },
      "redaction_profile": "storyos.author.v1",
      "reference_recovery": {"kind": "absent"},
      "run_id": "018f0000-0000-7001-8000-000000000a34",
      "schema_id": "storyos.query.agent-run.response.v2",
      "status": "queued",
      "steering_inputs": [],
      "unknown_create_successor": {"kind": "absent"},
      "usage": {"kind": "unknown"}
    }"#,
    ),
};

const CREATE_AGENT_RUN_NOTES: &str = "Stage 3 planned v2: closed conversation selection new or existing with exact conversation_id; atomic new identity/settings and Run admission; accepted response and getAgentRun return conversation_id and captured memory_settings_revision; exact retries reuse all identities. See protocol section 13.9. Foreground admission serializes with settings changes.";

const GET_AGENT_RUN_NOTES: &str = "Stage 3 planned v2 inspection: exact conversation/settings binding, bounded Attempt/continuation evidence, typed known/reported/opaque content and availability. Real Provider capability proof remains Stage 4/5.";

const CREATE_AGENT_RUN_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/AssistanceWorkingTarget/oneOf/0/properties/chapter_id",
            "/$defs/AssistanceWorkingTarget/oneOf/1/properties/source_chapter_id",
            "/$defs/AssistanceWorkingTarget/oneOf/2/properties/source_chapter_id",
            "/$defs/ConversationSelection/oneOf/1/properties/conversation_id",
            "/$defs/CreateAgentRunInput/properties/correlation_id",
            "/$defs/PassageTarget/properties/base_authoritative_revision_id",
            "/$defs/PassageTarget/properties/chapter_id",
            "/$defs/PassageTarget/properties/manuscript_block_ids/items",
            "/$defs/SelectionPoint/properties/manuscript_block_id",
            "/$defs/SelectionReference/oneOf/0/properties/base_authoritative_revision_id",
            "/$defs/SelectionReference/oneOf/0/properties/chapter_id",
            "/$defs/SelectionReference/oneOf/1/properties/chapter_id",
            "/$defs/SelectionReference/oneOf/1/properties/operation_id",
            "/$defs/SelectionReference/oneOf/1/properties/proposal_id",
            "/$defs/SelectionReference/oneOf/1/properties/revision_id",
        ],
    ),
    Patch(
        Edit::MaxLength(8000),
        &["/$defs/AuthorMessage/properties/text"],
    ),
    Patch(
        Edit::MinLength(1),
        &["/$defs/AuthorMessage/properties/text"],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const CREATE_AGENT_RUN_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/AgentRunRef/oneOf/0/properties/run_id",
            "/$defs/CreateAgentRunEffect/oneOf/0/properties/conversation_id",
            "/$defs/CreateAgentRunEffect/oneOf/0/properties/memory_settings_revision",
            "/$defs/CreateAgentRunEffect/oneOf/0/properties/project_agent_id",
            "/$defs/CreateAgentRunEffect/oneOf/0/properties/run_id",
            "/$defs/ProjectScope/properties/owner_user_id",
            "/$defs/ProjectScope/properties/project_id",
            "/properties/author_command_admission_id",
            "/properties/command_id",
            "/properties/conversation_id",
            "/properties/correlation_id",
            "/properties/memory_settings_revision",
            "/properties/project_agent_id",
        ],
    ),
    Patch(
        U64,
        &["/$defs/CreateAgentRunEffect/oneOf/0/properties/project_activity_position"],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];

const GET_AGENT_RUN_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/ActiveCompactionKnownInput/oneOf/0/properties/id",
            "/$defs/ActiveCompactionKnownInput/oneOf/1/properties/id",
            "/$defs/AgentRunContextInspect/properties/assembly_manifest_id",
            "/$defs/AgentRunContextInspect/properties/input_snapshot_id",
            "/$defs/AgentRunContextInspect/properties/joinable_block_ids/items",
            "/$defs/AgentRunContextInspect/properties/operation_requirement_id",
            "/$defs/AgentRunSteeringInspect/properties/input_snapshot_id",
            "/$defs/AgentRunSteeringInspect/properties/model_attempt_id",
            "/$defs/AgentRunSteeringInspect/properties/steering_input_id",
            "/$defs/AttemptEvidence/oneOf/0/properties/attempt_id",
            "/$defs/AttemptEvidence/oneOf/1/properties/attempt_id",
            "/$defs/AttemptEvidence/oneOf/1/properties/reference_id",
            "/$defs/AttemptEvidence/oneOf/2/properties/attempt_id",
            "/$defs/AttemptEvidence/oneOf/3/properties/attempt_id",
            "/$defs/CapturedMemorySettingsInspect/oneOf/0/properties/memory_settings_revision",
            "/$defs/ContinuationAdmissionInspect/properties/external_compatibility_decision",
            "/$defs/ContinuationAdmissionInspect/properties/model_registration_revision",
            "/$defs/ContinuationAdmissionInspect/properties/processing_destination_identity",
            "/$defs/ContinuationAdmissionInspect/properties/project_model_use_binding_revision",
            "/$defs/OptionalActiveCompactionInspect/oneOf/1/properties/compaction_id",
            "/$defs/OptionalActiveCompactionInspect/oneOf/1/properties/prior_manifest_id",
            "/$defs/OptionalActiveCompactionInspect/oneOf/1/properties/prior_model_attempt_id",
            "/$defs/OptionalActiveCompactionInspect/oneOf/1/properties/prior_run_step_id",
            "/$defs/OptionalActiveCompactionInspect/oneOf/1/properties/producer_invocation_id",
            "/$defs/OptionalActiveCompactionInspect/oneOf/1/properties/producer_manifest_id",
            "/$defs/OptionalActiveCompactionInspect/oneOf/1/properties/producer_model_attempt_id",
            "/$defs/OptionalCompactionInstallInspect/oneOf/1/properties/model_attempt_id",
            "/$defs/OptionalCompactionInstallInspect/oneOf/1/properties/model_invocation_id",
            "/$defs/OptionalCompactionInstallInspect/oneOf/1/properties/run_step_id",
            "/$defs/OptionalContinuationInspect/oneOf/1/properties/continuation_binding_id",
            "/$defs/OptionalDecisionInspect/oneOf/2/properties/decision_id",
            "/$defs/OptionalDecisionInspect/oneOf/3/properties/decision_id",
            "/$defs/OptionalDecisionInspect/oneOf/4/properties/decision_id",
            "/$defs/OptionalDecisionInspect/oneOf/5/properties/decision_id",
            "/$defs/OptionalDecisionInspect/oneOf/6/properties/decision_id",
            "/$defs/OptionalDecisionInspect/oneOf/6/properties/research_artifact_id",
            "/$defs/OptionalManifestRef/oneOf/1/properties/manifest_id",
            "/$defs/OptionalModelAttemptInspect/oneOf/1/properties/destination_attempt_id",
            "/$defs/OptionalModelAttemptInspect/oneOf/1/properties/model_attempt_id",
            "/$defs/OptionalModelAttemptInspect/oneOf/1/properties/model_invocation_id",
            "/$defs/OptionalModelAttemptInspect/oneOf/1/properties/outbound_disclosure_event_id",
            "/$defs/OptionalOpenedProposalInspect/oneOf/1/properties/proposal_id",
            "/$defs/OptionalOriginalResultRetrievalInspect/oneOf/1/properties/assembly_manifest_id",
            "/$defs/OptionalOriginalResultRetrievalInspect/oneOf/1/properties/original_model_attempt_id",
            "/$defs/OptionalOriginalResultRetrievalInspect/oneOf/1/properties/reconciliation_id",
            "/$defs/OptionalOriginalResultRetrievalInspect/oneOf/1/properties/response_reference_id",
            "/$defs/OptionalOriginalResultRetrievalInspect/oneOf/1/properties/retrieval_attempt_id",
            "/$defs/OptionalReferenceRecoveryInspect/oneOf/1/properties/assembly_manifest_id",
            "/$defs/OptionalReferenceRecoveryInspect/oneOf/1/properties/model_attempt_id",
            "/$defs/OptionalReferenceRecoveryInspect/oneOf/1/properties/model_invocation_id",
            "/$defs/OptionalReferenceRecoveryInspect/oneOf/1/properties/predecessor_continuation_binding_id",
            "/$defs/OptionalReferenceRecoveryInspect/oneOf/1/properties/predecessor_run_id",
            "/$defs/OptionalReferenceRecoveryInspect/oneOf/1/properties/recovery_id",
            "/$defs/OptionalReferenceRecoveryInspect/oneOf/1/properties/run_step_id",
            "/$defs/OptionalUnknownCreateSuccessorInspect/oneOf/1/properties/model_invocation_id",
            "/$defs/OptionalUnknownCreateSuccessorInspect/oneOf/1/properties/predecessor_model_attempt_id",
            "/$defs/OptionalUnknownCreateSuccessorInspect/oneOf/1/properties/recovery_id",
            "/$defs/OptionalUnknownCreateSuccessorInspect/oneOf/1/properties/successor_model_attempt_id",
            "/$defs/PassageTarget/properties/base_authoritative_revision_id",
            "/$defs/PassageTarget/properties/chapter_id",
            "/$defs/PassageTarget/properties/manuscript_block_ids/items",
            "/$defs/ProjectScope/properties/owner_user_id",
            "/$defs/ProjectScope/properties/project_id",
            "/$defs/ProposalCandidateTarget/properties/operation_id",
            "/$defs/ProposalCandidateTarget/properties/proposal_id",
            "/$defs/ProposalCandidateTarget/properties/revision_id",
            "/$defs/ProseChangeLocationCurrent/properties/revision_id",
            "/$defs/ProseChangeLocationInspect/properties/base_authoritative_revision_id",
            "/$defs/ProseChangeLocationInspect/properties/chapter_id",
            "/$defs/ProseChangeLocationInspect/properties/manuscript_block_id",
            "/$defs/ProseChangeLocationOutcome/oneOf/0/properties/operation_id",
            "/$defs/ProseChangeLocationOutcome/oneOf/0/properties/prior_revision_id",
            "/$defs/ProseChangeLocationOutcome/oneOf/0/properties/proposal_id",
            "/$defs/ProseChangeLocationOutcome/oneOf/0/properties/revision_id",
            "/$defs/ProseChangeLocationOutcome/oneOf/0/properties/validation_receipt_id",
            "/$defs/ProseChangeLocationOutcome/oneOf/1/properties/operation_id",
            "/$defs/ProseChangeLocationOutcome/oneOf/1/properties/proposal_id",
            "/$defs/ProseChangeLocationOutcome/oneOf/1/properties/revision_id",
            "/$defs/ProseChangeLocationOutcome/oneOf/1/properties/validation_receipt_id",
            "/$defs/SelectionPoint/properties/manuscript_block_id",
            "/$defs/SelectionReference/oneOf/0/properties/base_authoritative_revision_id",
            "/$defs/SelectionReference/oneOf/0/properties/chapter_id",
            "/$defs/SelectionReference/oneOf/1/properties/chapter_id",
            "/$defs/SelectionReference/oneOf/1/properties/operation_id",
            "/$defs/SelectionReference/oneOf/1/properties/proposal_id",
            "/$defs/SelectionReference/oneOf/1/properties/revision_id",
            "/$defs/SelectionReferenceInspect/properties/manuscript_block_ids/items",
            "/$defs/SourceAvailability/oneOf/2/properties/current_revision_id",
            "/properties/conversation_id",
            "/properties/correlation_id",
            "/properties/memory_settings_revision",
            "/properties/project_agent_id",
            "/properties/run_id",
        ],
    ),
    Patch(
        U64,
        &[
            "/$defs/ContextProjectionInspect/properties/token_count",
            "/$defs/ContextRejectionInspect/properties/token_count",
            "/$defs/ContextSourceInspect/properties/token_count",
            "/$defs/TokenCountingProfileInspect/properties/item_token_limit",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];
