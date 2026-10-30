use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::ProjectActivityKind;
use crate::release1_agent_run::{
    ActiveCompactionInstallState, ActiveCompactionKnownInput, ActiveCompactionMappingKind,
    AgentRunContextInspect, AgentRunRef, AgentRunStatus, AgentRunStreamItemInspect,
    AgentRunUsageInspect, AssistanceCause, AssistanceWorkingTarget, AttemptCacheInspect,
    AttemptEvidence, AuthorMessage, CREATE_AGENT_RUN, CREATE_AGENT_RUN_REQUEST_SCHEMA_ID,
    CREATE_AGENT_RUN_RESPONSE_SCHEMA_ID, CapturedMemorySettingsInspect, ContextBlockReason,
    ContextProjectionInspect, ContextPurpose, ContextRejectionInspect, ContextRejectionReason,
    ContextSourceClass, ContextSourceInspect, ContextSufficiency, ContinuationAdmissionInspect,
    ContinuationInputMappingInspect, ConversationSelection, CreateAgentRunEffect,
    CreateAgentRunInput, CreateAgentRunRequest, CreateAgentRunResponse, CurrentAvailabilityInspect,
    DestinationIo, EvidenceAvailability, GET_AGENT_RUN, GET_AGENT_RUN_REQUEST_SCHEMA_ID,
    GET_AGENT_RUN_RESPONSE_SCHEMA_ID, GetAgentRunRequest, GetAgentRunResponse, HostControlInspect,
    InstructionBinding, OptionalActiveCompactionInspect, OptionalCompactionInstallInspect,
    OptionalContinuationInspect, OptionalDecisionInspect, OptionalManifestRef,
    OptionalModelAttemptInspect, OptionalOpenedProposalInspect,
    OptionalOriginalResultRetrievalInspect, OptionalReferenceRecoveryInspect,
    OptionalUnknownCreateSuccessorInspect, OriginalResultRetrievalDisposition, PassageTarget,
    ProjectionMode, ProposalCandidateTarget, ProseChangeLocationCurrent,
    ProseChangeLocationInspect, ProseChangeLocationOutcome, ReferenceRecoveryDisposition,
    SelectionCoordinateProfile, SelectionPoint, SelectionReference, SelectionReferenceInspect,
    SourceAvailability, TokenCountingProfileInspect, UnknownCreateSuccessorDisposition,
};
use crate::release1_operation_registry::{
    ClientOverride, FixtureMembership, OpenApiMethod, OperationArtifacts, RegisteredOperation,
    operation_schemas,
};
use crate::release1_route_catalog::{
    ActionClass, Admission, Binding, EditorRule, ErrorProfile, Idempotency, OperationCatalog,
    Owner, Settlement,
};
use crate::release1_wire::{U64_WIRE, generated_ref, json_bytes, schema_value};

const CREATE_AGENT_RUN_NOTES: &str = "Stage 3 planned v2: closed conversation selection new or existing with exact conversation_id; atomic new identity/settings and Run admission; accepted response and getAgentRun return conversation_id and captured memory_settings_revision; exact retries reuse all identities. See protocol section 13.9. Foreground admission serializes with settings changes.";
const GET_AGENT_RUN_NOTES: &str = "Stage 3 planned v2 inspection: exact conversation/settings binding, bounded Attempt/continuation/hosted-operation evidence, typed known/reported/opaque content and availability. Real Provider capability proof remains Stage 4/5.";

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[
        RegisteredOperation::command(
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
        RegisteredOperation::query(
            &GET_AGENT_RUN,
            &["run_scope_join", "run_projection_watermark_or_snapshot"],
            OperationCatalog::new("agent-run:read", Owner::Context, Binding::PROJECTION_QUERY)
                .notes(GET_AGENT_RUN_NOTES),
        )
        .client(ClientOverride::OptionalQueryParameter("model_attempt_id")),
    ],
    schemas: || {
        [
            operation_schemas(
                &CREATE_AGENT_RUN,
                (CREATE_REQUEST_SCHEMA_PATH, create_request_schema_bytes()),
                (CREATE_RESPONSE_SCHEMA_PATH, create_response_schema_bytes()),
            ),
            operation_schemas(
                &GET_AGENT_RUN,
                (GET_REQUEST_SCHEMA_PATH, get_request_schema_bytes()),
                (GET_RESPONSE_SCHEMA_PATH, get_response_schema_bytes()),
            ),
        ]
        .into_iter()
        .flatten()
        .collect()
    },
    openapi,
    typescript_types: typescript_type_declarations,
    fixtures: &[
        FixtureMembership {
            path: CREATE_FIXTURE_PATH,
            bytes: |_| create_fixture_bytes(),
        },
        FixtureMembership {
            path: GET_FIXTURE_PATH,
            bytes: |_| get_fixture_bytes(),
        },
    ],
};

pub(super) const CREATE_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/create-agent-run-request.schema.json";
pub(super) const CREATE_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/create-agent-run-response.schema.json";
pub(super) const GET_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/agent-run-request.schema.json";
pub(super) const GET_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/agent-run-response.schema.json";
pub(super) const CREATE_FIXTURE_PATH: &str =
    "generated/golden-wire/storyos-public-release-1/create-agent-run.json";
pub(super) const GET_FIXTURE_PATH: &str =
    "generated/golden-wire/storyos-public-release-1/get-agent-run.json";

pub(super) const SELECTION_REFERENCE_UUID_FIELDS: &[&str] = &[
    "chapter_id",
    "base_authoritative_revision_id",
    "proposal_id",
    "operation_id",
    "revision_id",
];

pub(super) fn create_request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<CreateAgentRunRequest>(
        CREATE_AGENT_RUN_REQUEST_SCHEMA_ID,
        "StoryOS Create Agent Run Request",
    );
    schema["properties"]["command_schema"]["const"] = json!(CREATE_AGENT_RUN_REQUEST_SCHEMA_ID);
    let input = &mut schema["$defs"]["CreateAgentRunInput"]["properties"];
    input["correlation_id"]["format"] = json!("uuid");
    if let Some(message) = schema["$defs"].get_mut("AuthorMessage") {
        message["properties"]["text"]["maxLength"] = json!(8000);
        message["properties"]["text"]["minLength"] = json!(1);
    }
    if let Some(existing) = schema["$defs"].get_mut("ConversationSelection") {
        constrain_uuid_fields(existing, &["conversation_id"]);
    }
    if let Some(target) = schema["$defs"].get_mut("AssistanceWorkingTarget") {
        constrain_uuid_fields(target, &["chapter_id", "source_chapter_id"]);
    }
    if let Some(target) = schema["$defs"].get_mut("PassageTarget") {
        constrain_uuid_fields(target, &["chapter_id", "base_authoritative_revision_id"]);
        target["properties"]["manuscript_block_ids"]["items"]["format"] = json!("uuid");
    }
    if let Some(reference) = schema["$defs"].get_mut("SelectionReference") {
        constrain_uuid_fields(reference, SELECTION_REFERENCE_UUID_FIELDS);
    }
    if let Some(point) = schema["$defs"].get_mut("SelectionPoint") {
        constrain_uuid_fields(point, &["manuscript_block_id"]);
    }
    json_bytes(&schema)
}

pub(super) fn create_response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<CreateAgentRunResponse>(
        CREATE_AGENT_RUN_RESPONSE_SCHEMA_ID,
        "StoryOS Create Agent Run Response",
    );
    schema["properties"]["schema_id"]["const"] = json!(CREATE_AGENT_RUN_RESPONSE_SCHEMA_ID);
    for field in [
        "correlation_id",
        "command_id",
        "author_command_admission_id",
        "project_agent_id",
        "conversation_id",
    ] {
        schema["properties"][field]["format"] = json!("uuid");
    }
    schema["properties"]["memory_settings_revision"]["format"] = json!("uuid");
    if let Some(effect) = schema["$defs"].get_mut("CreateAgentRunEffect") {
        constrain_uuid_fields(
            effect,
            &[
                "project_agent_id",
                "conversation_id",
                "run_id",
                "memory_settings_revision",
            ],
        );
        constrain_revision_fields(effect);
    }
    if let Some(run_ref) = schema["$defs"].get_mut("AgentRunRef") {
        constrain_uuid_fields(run_ref, &["run_id"]);
    }
    if let Some(scope) = schema["$defs"].get_mut("ProjectScope") {
        scope["properties"]["owner_user_id"]["format"] = json!("uuid");
        scope["properties"]["project_id"]["format"] = json!("uuid");
    }
    json_bytes(&schema)
}

pub(super) fn get_request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<GetAgentRunRequest>(
        GET_AGENT_RUN_REQUEST_SCHEMA_ID,
        "StoryOS Agent Run Request",
    );
    if let Some(properties) = schema.get_mut("properties")
        && properties.get("model_attempt_id").is_some()
    {
        properties["model_attempt_id"]["format"] = json!("uuid");
    }
    json_bytes(&schema)
}

pub(super) fn get_response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<GetAgentRunResponse>(
        GET_AGENT_RUN_RESPONSE_SCHEMA_ID,
        "StoryOS Agent Run Response",
    );
    schema["properties"]["schema_id"]["const"] = json!(GET_AGENT_RUN_RESPONSE_SCHEMA_ID);
    for field in [
        "correlation_id",
        "project_agent_id",
        "conversation_id",
        "run_id",
    ] {
        schema["properties"][field]["format"] = json!("uuid");
    }
    schema["properties"]["memory_settings_revision"]["format"] = json!("uuid");
    if let Some(settings) = schema["$defs"].get_mut("CapturedMemorySettingsInspect") {
        for variant in settings["oneOf"].as_array_mut().into_iter().flatten() {
            if let Some(revision) = variant["properties"].get_mut("memory_settings_revision") {
                revision["format"] = json!("uuid");
            }
        }
    }
    if let Some(scope) = schema["$defs"].get_mut("ProjectScope") {
        scope["properties"]["owner_user_id"]["format"] = json!("uuid");
        scope["properties"]["project_id"]["format"] = json!("uuid");
    }
    if let Some(inspect) = schema["$defs"].get_mut("AgentRunContextInspect") {
        constrain_uuid_fields(
            inspect,
            &[
                "operation_requirement_id",
                "input_snapshot_id",
                "assembly_manifest_id",
            ],
        );
        constrain_count_fields(inspect);
        inspect["properties"]["joinable_block_ids"]["items"]["format"] = json!("uuid");
    }
    for name in [
        "AgentRunSteeringInspect",
        "TokenCountingProfileInspect",
        "ContextSourceInspect",
        "ContextProjectionInspect",
        "ContextRejectionInspect",
    ] {
        if let Some(definition) = schema["$defs"].get_mut(name) {
            constrain_count_fields(definition);
        }
    }
    if let Some(manifest) = schema["$defs"].get_mut("OptionalManifestRef") {
        constrain_uuid_fields(manifest, &["manifest_id"]);
    }
    if let Some(availability) = schema["$defs"].get_mut("SourceAvailability") {
        constrain_uuid_fields(availability, &["current_revision_id"]);
    }
    for name in [
        "AgentRunSteeringInspect",
        "AttemptEvidence",
        "OptionalDecisionInspect",
        "OptionalModelAttemptInspect",
        "OptionalActiveCompactionInspect",
        "OptionalCompactionInstallInspect",
        "OptionalReferenceRecoveryInspect",
        "OptionalOriginalResultRetrievalInspect",
        "OptionalUnknownCreateSuccessorInspect",
        "ActiveCompactionKnownInput",
        "OptionalContinuationInspect",
        "OptionalOpenedProposalInspect",
        "ProseChangeLocationInspect",
        "PassageTarget",
        "ProposalCandidateTarget",
        "SelectionReference",
        "SelectionPoint",
        "SelectionReferenceInspect",
        "ProseChangeLocationOutcome",
        "ProseChangeLocationCurrent",
        "ContinuationAdmissionInspect",
    ] {
        if let Some(definition) = schema["$defs"].get_mut(name) {
            constrain_uuid_fields(
                definition,
                &[
                    "steering_input_id",
                    "input_snapshot_id",
                    "model_attempt_id",
                    "attempt_id",
                    "decision_id",
                    "model_attempt_id",
                    "destination_attempt_id",
                    "outbound_disclosure_event_id",
                    "model_invocation_id",
                    "continuation_binding_id",
                    "reference_id",
                    "compaction_id",
                    "recovery_id",
                    "reconciliation_id",
                    "original_model_attempt_id",
                    "response_reference_id",
                    "retrieval_attempt_id",
                    "predecessor_model_attempt_id",
                    "successor_model_attempt_id",
                    "predecessor_run_id",
                    "predecessor_continuation_binding_id",
                    "assembly_manifest_id",
                    "prior_model_attempt_id",
                    "prior_manifest_id",
                    "prior_run_step_id",
                    "producer_model_attempt_id",
                    "producer_manifest_id",
                    "producer_invocation_id",
                    "run_step_id",
                    "id",
                    "proposal_id",
                    "chapter_id",
                    "manuscript_block_id",
                    "base_authoritative_revision_id",
                    "operation_id",
                    "revision_id",
                    "prior_revision_id",
                    "validation_receipt_id",
                    "processing_destination_identity",
                    "model_registration_revision",
                    "project_model_use_binding_revision",
                    "external_compatibility_decision",
                ],
            );
        }
    }
    schema["$defs"]["PassageTarget"]["properties"]["manuscript_block_ids"]["items"]["format"] =
        json!("uuid");
    if let Some(reference) = schema["$defs"].get_mut("SelectionReferenceInspect") {
        reference["properties"]["manuscript_block_ids"]["items"]["format"] = json!("uuid");
    }
    json_bytes(&schema)
}

pub(super) fn openapi() -> Vec<OpenApiMethod> {
    let create_request = generated_ref(CREATE_REQUEST_SCHEMA_PATH);
    let create_response = generated_ref(CREATE_RESPONSE_SCHEMA_PATH);
    let get_response = generated_ref(GET_RESPONSE_SCHEMA_PATH);
    let create_responses = status_block(
        CREATE_AGENT_RUN.responses,
        create_response,
        /*content_status*/ 202,
        /*command*/ true,
    );
    let get_responses = status_block(
        GET_AGENT_RUN.responses,
        get_response,
        /*content_status*/ 200,
        /*command*/ false,
    );
    vec![
        OpenApiMethod {
            path: CREATE_AGENT_RUN.path,
            yaml: format!(
                concat!(
                    "    post:\n      operationId: {}\n      summary: Admit one bounded assistance request\n",
                    "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
                    "        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
                    "        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
                    "        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n",
                    "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
                    "      responses:\n{}",
                ),
                CREATE_AGENT_RUN.operation_id, create_request, create_responses,
            ),
        },
        OpenApiMethod {
            path: GET_AGENT_RUN.path,
            yaml: format!(
                concat!(
                    "    get:\n      operationId: {}\n      summary: Inspect one durable AgentRun status\n",
                    "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
                    "        - name: run_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
                    "        - name: model_attempt_id\n          in: query\n          required: false\n          schema:\n            type: string\n            format: uuid\n",
                    "      responses:\n{}",
                ),
                GET_AGENT_RUN.operation_id, get_responses,
            ),
        },
    ]
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        ConversationSelection::decl(&config),
        AuthorMessage::decl(&config),
        AssistanceWorkingTarget::decl(&config),
        PassageTarget::decl(&config),
        SelectionReference::decl(&config),
        SelectionCoordinateProfile::decl(&config),
        SelectionPoint::decl(&config),
        ProposalCandidateTarget::decl(&config),
        InstructionBinding::decl(&config),
        AssistanceCause::decl(&config),
        CreateAgentRunInput::decl(&config),
        CreateAgentRunRequest::decl(&config),
        AgentRunRef::decl(&config),
        CreateAgentRunEffect::decl(&config),
        CreateAgentRunResponse::decl(&config),
        AgentRunStatus::decl(&config),
        ContextPurpose::decl(&config),
        ContextSourceClass::decl(&config),
        ProjectionMode::decl(&config),
        ContextSufficiency::decl(&config),
        ContextBlockReason::decl(&config),
        ContextRejectionReason::decl(&config),
        DestinationIo::decl(&config),
        OptionalManifestRef::decl(&config),
        SourceAvailability::decl(&config),
        TokenCountingProfileInspect::decl(&config),
        ContextSourceInspect::decl(&config),
        ContextProjectionInspect::decl(&config),
        ContextRejectionInspect::decl(&config),
        HostControlInspect::decl(&config),
        CurrentAvailabilityInspect::decl(&config),
        AgentRunContextInspect::decl(&config),
        SelectionReferenceInspect::decl(&config),
        EvidenceAvailability::decl(&config),
        AttemptEvidence::decl(&config),
        OptionalContinuationInspect::decl(&config),
        OptionalOpenedProposalInspect::decl(&config),
        ProseChangeLocationInspect::decl(&config),
        ProseChangeLocationOutcome::decl(&config),
        ProseChangeLocationCurrent::decl(&config),
        OptionalDecisionInspect::decl(&config),
        ContinuationInputMappingInspect::decl(&config),
        ContinuationAdmissionInspect::decl(&config),
        OptionalModelAttemptInspect::decl(&config),
        ActiveCompactionInstallState::decl(&config),
        ActiveCompactionMappingKind::decl(&config),
        ActiveCompactionKnownInput::decl(&config),
        OptionalCompactionInstallInspect::decl(&config),
        OptionalActiveCompactionInspect::decl(&config),
        ReferenceRecoveryDisposition::decl(&config),
        OptionalReferenceRecoveryInspect::decl(&config),
        AgentRunStreamItemInspect::decl(&config),
        AgentRunUsageInspect::decl(&config),
        AttemptCacheInspect::decl(&config),
        GetAgentRunRequest::decl(&config),
        CapturedMemorySettingsInspect::decl(&config),
        crate::release1_agent_run::AgentRunSteeringInspect::decl(&config),
        GetAgentRunResponse::decl(&config),
        OriginalResultRetrievalDisposition::decl(&config),
        OptionalOriginalResultRetrievalInspect::decl(&config),
        UnknownCreateSuccessorDisposition::decl(&config),
        OptionalUnknownCreateSuccessorInspect::decl(&config),
    )
}

pub(super) fn create_fixture_bytes() -> Vec<u8> {
    json_bytes(&create_fixture())
}

pub(super) fn get_fixture_bytes() -> Vec<u8> {
    json_bytes(&get_fixture())
}

fn create_fixture() -> Value {
    json!({
        "schema_id": CREATE_AGENT_RUN_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000a31",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "command_id": "018f0000-0000-7001-8000-000000000a32",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000a33",
        "acknowledgement": "accepted",
        "operation_ref": {
            "kind": "agent_run",
            "run_id": "018f0000-0000-7001-8000-000000000a34"
        },
        "project": {
            "project_id": "018f0000-0000-7001-8000-000000000201",
            "title": "Empty Novel",
            "open": {
                "kind": "current_chapter",
                "current_chapter_id": "018f0000-0000-7001-8000-000000000301"
            }
        },
        "project_agent_id": "018f0000-0000-7001-8000-000000000a35",
        "conversation_id": "018f0000-0000-7001-8000-000000000a36",
        "memory_settings_revision": "018f0000-0000-7001-8000-000000000a38",
        "effect": {
            "kind": "admitted",
            "project_agent_id": "018f0000-0000-7001-8000-000000000a35",
            "conversation_id": "018f0000-0000-7001-8000-000000000a36",
            "memory_settings_revision": "018f0000-0000-7001-8000-000000000a38",
            "run_id": "018f0000-0000-7001-8000-000000000a34",
            "project_activity_position": "1"
        }
    })
}

fn get_fixture() -> Value {
    json!({
        "schema_id": GET_AGENT_RUN_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000a37",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "project_agent_id": "018f0000-0000-7001-8000-000000000a35",
        "conversation_id": "018f0000-0000-7001-8000-000000000a36",
        "memory_settings_revision": "018f0000-0000-7001-8000-000000000a38",
        "run_id": "018f0000-0000-7001-8000-000000000a34",
        "status": "queued",
        "steering_inputs": [],
        "captured_memory_settings": {
            "kind": "available",
            "memory_settings_revision": "018f0000-0000-7001-8000-000000000a38",
            "use_enabled": true,
            "contribution_enabled": true
        },
        "context": {
            "operation_requirement_id": "018f0000-0000-7001-8000-000000000a39",
            "input_snapshot_id": "018f0000-0000-7001-8000-000000000a3a",
            "purpose": "current_passage_assistance",
            "token_counting_profile": {
                "profile_revision": "storyos.token-counting.unicode-scalar.v1",
                "algorithm_revision": "storyos.statistics.unicode-16.0.0.v1",
                "item_token_limit": "32000"
            },
            "sufficiency": { "kind": "complete" },
            "considered": [
                {
                    "source_class": "host_control",
                    "source_version": "018f0000-0000-7001-8000-000000000a39",
                    "token_count": "0",
                    "eligible": true
                },
                {
                    "source_class": "author_instruction",
                    "source_version": "018f0000-0000-7001-8000-000000000a3a",
                    "token_count": "23",
                    "eligible": true
                },
                {
                    "source_class": "working_target",
                    "source_version": "018f0000-0000-7001-8000-000000000301",
                    "token_count": "0",
                    "eligible": true
                },
                {
                    "source_class": "instruction_binding",
                    "source_version": "absent",
                    "token_count": "0",
                    "eligible": true
                }
            ],
            "selected": [
                {
                    "source_class": "author_instruction",
                    "source_version": "018f0000-0000-7001-8000-000000000a3a",
                    "projection_mode": "exact_required",
                    "token_count": "23",
                    "content": "Help with this passage."
                },
                {
                    "source_class": "working_target",
                    "source_version": "018f0000-0000-7001-8000-000000000301",
                    "projection_mode": "exact_required",
                    "token_count": "0",
                    "content": ""
                }
            ],
            "rejected": [],
            "host_control": {
                "distinct_from_destination": true,
                "destination_visible": false
            },
            "assembly_manifest_id": "018f0000-0000-7001-8000-000000000a3b",
            "destination_context_manifest": { "kind": "absent" },
            "outbound_disclosure_manifest": { "kind": "absent" },
            "destination_io": { "kind": "none" },
            "current_availability": {
                "working_target": { "kind": "current" }
            }
        },
        "decision": { "kind": "absent" },
        "model_attempt": { "kind": "absent" },
        "active_compaction": { "kind": "absent" },
        "reference_recovery": { "kind": "absent" },
        "original_result_retrieval": { "kind": "absent" },
        "unknown_create_successor": { "kind": "absent" },
        "evidence": [],
        "items": [],
        "usage": { "kind": "unknown" },
        "cache": { "kind": "not_recorded" },
        "redaction_profile": "storyos.author.v1"
    })
}

fn constrain_count_fields(schema: &mut Value) {
    if let Some(properties) = schema.get_mut("properties") {
        for field in ["item_token_limit", "token_count"] {
            if properties.get(field).is_some() {
                properties[field] = json!({"type": "string", "pattern": U64_WIRE});
            }
        }
    }
    if let Some(one_of) = schema.get_mut("oneOf").and_then(Value::as_array_mut) {
        for variant in one_of {
            constrain_count_fields(variant);
        }
    }
}

fn constrain_revision_fields(schema: &mut Value) {
    if let Some(properties) = schema.get_mut("properties")
        && properties.get("project_activity_position").is_some()
    {
        properties["project_activity_position"] = json!({"type": "string", "pattern": U64_WIRE});
    }
    if let Some(one_of) = schema.get_mut("oneOf").and_then(Value::as_array_mut) {
        for variant in one_of {
            constrain_revision_fields(variant);
        }
    }
}

fn constrain_uuid_fields(schema: &mut Value, fields: &[&str]) {
    if let Some(properties) = schema.get_mut("properties") {
        for field in fields {
            if properties.get(*field).is_some() {
                properties[*field]["format"] = json!("uuid");
            }
        }
    }
    if let Some(one_of) = schema.get_mut("oneOf").and_then(Value::as_array_mut) {
        for variant in one_of {
            constrain_uuid_fields(variant, fields);
        }
    }
}

fn status_block(
    responses: &[(u16, &str)],
    response_schema: &str,
    content_status: u16,
    command: bool,
) -> String {
    responses
        .iter()
        .map(|(status, description)| {
            let retry_after = if command && *status == 429 {
                "          headers:\n            Retry-After:\n              required: true\n              schema:\n                type: integer\n                minimum: 1\n                maximum: 60\n"
            } else {
                ""
            };
            let content = if *status == content_status {
                format!(
                    "          content:\n            application/json:\n              schema:\n                $ref: '../{response_schema}'\n"
                )
            } else {
                String::new()
            };
            format!("        '{status}':\n          description: {description}\n{retry_after}{content}")
        })
        .collect()
}
