use axum::body::to_bytes;
use axum::extract::Query;
use sha2::{Digest, Sha256};
use storyos_application::{
    AuthorCommandAdmissionIds, ConversationSelection, CreateAgentRunCommand, CreateAgentRunError,
    EditorClientBinding, ProjectCommandChallengeBinding, open_agent_run, request_create_agent_run,
};

use super::editor_session::{exact_header, session_binding_ref};
use super::project_command_challenge::{
    hex_bytes, plain_digest, valid_uuid_v7, validate_json_content_type,
};
use super::*;

pub(super) async fn create_agent_run(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    request: Request,
) -> Result<(StatusCode, Json<contracts::CreateAgentRunResponse>), ApiError> {
    let (parts, body_stream) = request.into_parts();
    let headers = parts.headers;
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::StateChanging,
    )?;
    validate_json_content_type(&headers)?;
    let bytes = to_bytes(body_stream, contracts::AUTHOR_EDIT_MAX_WIRE_BODY_BYTES)
        .await
        .map_err(|_| payload_too_large())?;
    let body = serde_json::from_slice::<contracts::CreateAgentRunRequest>(&bytes)
        .map_err(|_| invalid_request_shape())?;
    let input = &body.create_agent_run_input;
    let session_handle = session_cookie(&headers).ok_or_else(authentication_required)?;
    let session = state
        .client_session_binding(session_handle)
        .ok_or_else(authentication_required)?;
    if body.command_schema != contracts::CREATE_AGENT_RUN_REQUEST_SCHEMA_ID
        || input.client_contract_revision != session.client_contract_revision
        || input.security_policy_revision != session.security_policy_revision
    {
        return Err(invalid_request());
    }
    valid_uuid(&input.correlation_id)?;
    let idempotency_key = exact_header(&headers, "idempotency-key")?;
    let nonce = exact_header(&headers, "x-storyos-anti-forgery")?;
    if !valid_uuid_v7(idempotency_key)
        || nonce.len() != 64
        || !nonce
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(invalid_request());
    }
    let secret = state
        .config
        .project_command_challenge_secret
        .as_deref()
        .filter(|secret| secret.len() >= 32)
        .ok_or_else(challenge_store_unavailable)?;
    let binding_ref = session_binding_ref(secret, session_handle);
    let canonical_command_bytes = canonical_body_bytes(&body)?;
    let digest_hex = hex_bytes(&Sha256::digest(&canonical_command_bytes));
    let canonical_command_digest = format!(
        "sha256:{}:{digest_hex}",
        contracts::CREATE_AGENT_RUN_DIGEST_PROFILE
    );
    let conversation_id = match &input.conversation {
        contracts::ConversationSelection::New => Uuid::now_v7().to_string(),
        contracts::ConversationSelection::Existing { conversation_id } => {
            valid_uuid(conversation_id)?;
            conversation_id.clone()
        }
    };
    let contracts::AssistanceWorkingTarget::CurrentChapter { chapter_id } = &input.working_target;
    valid_uuid(chapter_id)?;
    let store = project_reader(&state).await?;
    let admission = request_create_agent_run(
        &store,
        &CreateAgentRunCommand {
            project_scope: scope.clone(),
            client_binding: EditorClientBinding {
                binding_ref: binding_ref.clone(),
                session_generation: session.session_generation,
                client_contract_revision: session.client_contract_revision.clone(),
                security_policy_revision: session.security_policy_revision.clone(),
            },
            challenge_binding: ProjectCommandChallengeBinding {
                project_scope: scope.clone(),
                client_session_binding_digest: binding_ref,
                client_session_generation: session.session_generation,
                client_contract_revision: session.client_contract_revision.clone(),
                security_policy_revision: session.security_policy_revision.clone(),
                limit_profile_revision: contracts::LIMIT_PROFILE_REVISION.to_owned(),
                challenge_rate_policy_revision:
                    storyos_application::PROJECT_COMMAND_CHALLENGE_RATE_POLICY_REVISION.to_owned(),
                method: contracts::CREATE_AGENT_RUN_METHOD.to_owned(),
                route_template: contracts::CREATE_AGENT_RUN_PATH.to_owned(),
                command_schema: body.command_schema.clone(),
                command_kind: "createAgentRun".to_owned(),
                canonical_command_digest,
                idempotency_key: idempotency_key.to_owned(),
            },
            nonce_digest: plain_digest(nonce.as_bytes()),
            canonical_command_bytes,
            correlation_id: input.correlation_id.clone(),
            conversation: match &input.conversation {
                contracts::ConversationSelection::New => ConversationSelection::New,
                contracts::ConversationSelection::Existing { conversation_id } => {
                    ConversationSelection::Existing {
                        conversation_id: conversation_id.clone(),
                    }
                }
            },
            author_message: input.author_message.text.clone(),
            chapter_id: chapter_id.clone(),
            ids: AuthorCommandAdmissionIds {
                command_id: Uuid::now_v7().to_string(),
                author_command_admission_id: Uuid::now_v7().to_string(),
                receipt_id: Uuid::now_v7().to_string(),
            },
            run_id: Uuid::now_v7().to_string(),
            conversation_id,
            project_agent_id: Uuid::now_v7().to_string(),
        },
    )
    .await
    .map_err(create_agent_run_error)?;
    super::acknowledgement_hold::hold_first_acknowledgement_if_requested(idempotency_key).await;
    let project = admission.response_project;
    Ok((
        StatusCode::ACCEPTED,
        Json(contracts::CreateAgentRunResponse {
            schema_id: contracts::CREATE_AGENT_RUN_RESPONSE_SCHEMA_ID.to_owned(),
            correlation_id: input.correlation_id.clone(),
            project_scope: contract_scope(&scope),
            command_id: admission.ids.command_id,
            author_command_admission_id: admission.ids.author_command_admission_id,
            acknowledgement: contracts::ExportAcknowledgement::Accepted,
            operation_ref: Some(contracts::AgentRunRef::AgentRun {
                run_id: admission.run_id.clone(),
            }),
            project: contracts::ControlledProject {
                project_id: project.project_id.as_ref().to_owned(),
                title: project.title,
                open: match project.current_chapter_id {
                    Some(chapter_id) => contracts::ProjectOpenState::CurrentChapter {
                        current_chapter_id: chapter_id.as_ref().to_owned(),
                    },
                    None => contracts::ProjectOpenState::Empty,
                },
            },
            project_agent_id: admission.project_agent_id.clone(),
            conversation_id: admission.conversation_id.clone(),
            memory_settings_revision: admission.memory_settings_revision.clone(),
            effect: contracts::CreateAgentRunEffect::Admitted {
                project_agent_id: admission.project_agent_id,
                conversation_id: admission.conversation_id,
                memory_settings_revision: admission.memory_settings_revision,
                run_id: admission.run_id,
                project_activity_position: admission.project_activity_position.to_string(),
            },
        }),
    ))
}

pub(super) async fn get_agent_run(
    State(state): State<Arc<ServerState>>,
    Path((project_id, run_id)): Path<(String, String)>,
    Query(query): Query<contracts::GetAgentRunRequest>,
    headers: HeaderMap,
) -> Result<Json<contracts::GetAgentRunResponse>, ApiError> {
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    valid_uuid(&run_id)?;
    if let Some(model_attempt_id) = query.model_attempt_id.as_deref() {
        valid_uuid(model_attempt_id)?;
    }
    let reader = project_reader(&state).await?;
    let Some(record) = open_agent_run(&reader, &scope, &run_id)
        .await
        .map_err(create_agent_run_error)?
    else {
        return Err(resource_unavailable());
    };
    if let Some(model_attempt_id) = query.model_attempt_id.as_deref() {
        let matches = record
            .model
            .as_ref()
            .is_some_and(|model| model.model_attempt_id == model_attempt_id);
        if !matches {
            return Err(resource_unavailable());
        }
    }
    Ok(Json(contracts::GetAgentRunResponse {
        schema_id: contracts::GET_AGENT_RUN_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: Uuid::now_v7().to_string(),
        project_scope: contract_scope(&scope),
        project_agent_id: record.project_agent_id,
        conversation_id: record.conversation_id,
        memory_settings_revision: record.memory_settings_revision,
        run_id: record.run_id,
        status: inspect_status(record.status),
        context: inspect_context(&record.context),
        decision: inspect_decision(&record.decision),
        model_attempt: inspect_model(record.model.as_ref()),
        evidence: record
            .model
            .as_ref()
            .map(|model| inspect_evidence(&model.evidence))
            .unwrap_or_default(),
        items: record
            .model
            .as_ref()
            .map(|model| inspect_items(&model.items))
            .unwrap_or_default(),
        usage: contracts::AgentRunUsageInspect {
            kind: record
                .model
                .as_ref()
                .map(|model| model.usage_kind.clone())
                .unwrap_or_else(|| "unknown".to_owned()),
        },
        redaction_profile: "storyos.author.v1".to_owned(),
    }))
}

fn inspect_status(status: storyos_application::AgentRunStatus) -> contracts::AgentRunStatus {
    match status {
        storyos_application::AgentRunStatus::Queued => contracts::AgentRunStatus::Queued,
        storyos_application::AgentRunStatus::Claimed => contracts::AgentRunStatus::Claimed,
        storyos_application::AgentRunStatus::Waiting => contracts::AgentRunStatus::Waiting,
        storyos_application::AgentRunStatus::Completed => contracts::AgentRunStatus::Completed,
        storyos_application::AgentRunStatus::Refused => contracts::AgentRunStatus::Refused,
    }
}

fn inspect_decision(
    decision: &storyos_application::AgentRunDecisionInspect,
) -> contracts::OptionalDecisionInspect {
    match decision {
        storyos_application::AgentRunDecisionInspect::Absent => {
            contracts::OptionalDecisionInspect::Absent
        }
        storyos_application::AgentRunDecisionInspect::ExecutionRefused { capability } => {
            contracts::OptionalDecisionInspect::ExecutionRefused {
                capability: capability.clone(),
            }
        }
        storyos_application::AgentRunDecisionInspect::Advisory {
            decision_id,
            selected,
            text,
            continuation_binding_id,
        } => contracts::OptionalDecisionInspect::Advisory {
            decision_id: decision_id.clone(),
            selected: *selected,
            text: text.clone(),
            continuation: inspect_continuation(continuation_binding_id.as_deref()),
        },
        storyos_application::AgentRunDecisionInspect::ProseChange {
            decision_id,
            selected,
            text,
            producer_input,
            continuation_binding_id,
        } => contracts::OptionalDecisionInspect::ProseChange {
            decision_id: decision_id.clone(),
            selected: *selected,
            text: text.clone(),
            producer_input: producer_input.clone(),
            continuation: inspect_continuation(continuation_binding_id.as_deref()),
            authoritative: false,
        },
        storyos_application::AgentRunDecisionInspect::Clarification {
            decision_id,
            selected,
            question,
            required_reply,
            continuation_binding_id,
        } => contracts::OptionalDecisionInspect::Clarification {
            decision_id: decision_id.clone(),
            selected: *selected,
            question: question.clone(),
            required_reply: required_reply.clone(),
            continuation: inspect_continuation(continuation_binding_id.as_deref()),
        },
    }
}

fn inspect_continuation(
    continuation_binding_id: Option<&str>,
) -> contracts::OptionalContinuationInspect {
    match continuation_binding_id {
        Some(continuation_binding_id) => contracts::OptionalContinuationInspect::Present {
            continuation_binding_id: continuation_binding_id.to_owned(),
        },
        None => contracts::OptionalContinuationInspect::Absent,
    }
}

fn inspect_model(
    model: Option<&storyos_application::AgentRunModelInspect>,
) -> contracts::OptionalModelAttemptInspect {
    match model {
        Some(model) => contracts::OptionalModelAttemptInspect::Present {
            model_attempt_id: model.model_attempt_id.clone(),
            destination_attempt_id: model.destination_attempt_id.clone(),
            outbound_disclosure_event_id: model.outbound_disclosure_event_id.clone(),
            model_invocation_id: model.model_invocation_id.clone(),
            dispatch_state: model.dispatch_state.clone(),
        },
        None => contracts::OptionalModelAttemptInspect::Absent,
    }
}

fn inspect_evidence(
    evidence: &[storyos_application::AgentRunEvidence],
) -> Vec<contracts::AttemptEvidence> {
    evidence
        .iter()
        .map(|item| match item {
            storyos_application::AgentRunEvidence::SentContent {
                attempt_id,
                availability,
                content,
            } => contracts::AttemptEvidence::SentContent {
                attempt_id: attempt_id.clone(),
                availability: inspect_availability(*availability),
                content: content.clone(),
            },
            storyos_application::AgentRunEvidence::StoredReference {
                attempt_id,
                availability,
                reference_id,
            } => contracts::AttemptEvidence::StoredReference {
                attempt_id: attempt_id.clone(),
                availability: inspect_availability(*availability),
                reference_id: reference_id.clone(),
            },
            storyos_application::AgentRunEvidence::ProviderReport {
                attempt_id,
                availability,
                report,
            } => contracts::AttemptEvidence::ProviderReport {
                attempt_id: attempt_id.clone(),
                availability: inspect_availability(*availability),
                report: report.clone(),
            },
            storyos_application::AgentRunEvidence::ProviderOpaque {
                attempt_id,
                availability,
                unknown_facts,
            } => contracts::AttemptEvidence::ProviderOpaque {
                attempt_id: attempt_id.clone(),
                availability: inspect_availability(*availability),
                unknown_facts: unknown_facts.clone(),
            },
        })
        .collect()
}

fn inspect_items(
    items: &[storyos_application::AgentRunStreamItem],
) -> Vec<contracts::AgentRunStreamItemInspect> {
    items
        .iter()
        .map(|item| contracts::AgentRunStreamItemInspect {
            item_id: item.item_id.clone(),
            role: item.role.clone(),
            state: item.state.clone(),
            phase: item.phase.clone(),
            text: item.text.clone(),
            summary: item.summary.clone(),
            call_id: item.call_id.clone(),
            arguments: item.arguments.clone(),
            refusal: item.refusal.clone(),
            hosted_report: item.hosted_report.clone(),
        })
        .collect()
}

fn inspect_availability(
    availability: storyos_application::EvidenceAvailability,
) -> contracts::EvidenceAvailability {
    match availability {
        storyos_application::EvidenceAvailability::Current => {
            contracts::EvidenceAvailability::Current
        }
        storyos_application::EvidenceAvailability::Unknown => {
            contracts::EvidenceAvailability::Unknown
        }
    }
}

fn inspect_context(
    context: &storyos_application::AgentRunContext,
) -> contracts::AgentRunContextInspect {
    use storyos_core::{ContextBlockReason, ContextSufficiency, RejectionReason};
    let record = &context.record;
    let dispatched = context.destination_context_manifest_id.is_some();
    contracts::AgentRunContextInspect {
        operation_requirement_id: record
            .operation_requirement
            .operation_requirement_id
            .clone(),
        input_snapshot_id: record.operation_requirement.input_snapshot_id.clone(),
        purpose: contracts::ContextPurpose::CurrentPassageAssistance,
        token_counting_profile: contracts::TokenCountingProfileInspect {
            profile_revision: record.token_counting_profile_revision.clone(),
            algorithm_revision: record.token_counting_algorithm_revision.clone(),
            item_token_limit: record.operation_requirement.item_token_limit.to_string(),
        },
        sufficiency: match &record.sufficiency {
            ContextSufficiency::Complete => contracts::ContextSufficiency::Complete,
            ContextSufficiency::Blocked { reasons } => contracts::ContextSufficiency::Blocked {
                reasons: reasons
                    .iter()
                    .map(|reason| match reason {
                        ContextBlockReason::ExactRequiredOverLimit { source_class } => {
                            contracts::ContextBlockReason::ExactRequiredOverLimit {
                                source_class: inspect_source_class(*source_class),
                            }
                        }
                        ContextBlockReason::RequiredInstructionRevisionUnavailable => {
                            contracts::ContextBlockReason::RequiredInstructionRevisionUnavailable
                        }
                        ContextBlockReason::WorkingTargetRevisionUnavailable => {
                            contracts::ContextBlockReason::WorkingTargetRevisionUnavailable
                        }
                    })
                    .collect(),
            },
        },
        considered: record
            .considered
            .iter()
            .map(|source| contracts::ContextSourceInspect {
                source_class: inspect_source_class(source.source_class),
                source_version: source.source_version.clone(),
                token_count: source.token_count.to_string(),
                eligible: source.eligible,
            })
            .collect(),
        selected: record
            .selected
            .iter()
            .map(|source| contracts::ContextProjectionInspect {
                source_class: inspect_source_class(source.source_class),
                source_version: source.source_version.clone(),
                projection_mode: contracts::ProjectionMode::ExactRequired,
                token_count: source.token_count.to_string(),
                content: source.content.clone(),
            })
            .collect(),
        rejected: record
            .rejected
            .iter()
            .map(|source| contracts::ContextRejectionInspect {
                source_class: inspect_source_class(source.source_class),
                source_version: source.source_version.clone(),
                token_count: source.token_count.to_string(),
                reason: match source.reason {
                    RejectionReason::OverItemTokenLimit => {
                        contracts::ContextRejectionReason::OverItemTokenLimit
                    }
                    RejectionReason::RequiredRevisionUnavailable => {
                        contracts::ContextRejectionReason::RequiredRevisionUnavailable
                    }
                    RejectionReason::WorkingTargetRevisionUnavailable => {
                        contracts::ContextRejectionReason::WorkingTargetRevisionUnavailable
                    }
                },
            })
            .collect(),
        host_control: contracts::HostControlInspect {
            distinct_from_destination: record.host_control.distinct_from_destination,
            destination_visible: dispatched || record.host_control.destination_visible,
        },
        assembly_manifest_id: context.assembly_manifest_id.clone(),
        destination_context_manifest: optional_manifest(
            context.destination_context_manifest_id.as_deref(),
        ),
        outbound_disclosure_manifest: optional_manifest(
            context.outbound_disclosure_manifest_id.as_deref(),
        ),
        destination_io: if dispatched {
            contracts::DestinationIo::HostFake
        } else {
            contracts::DestinationIo::None
        },
        current_availability: contracts::CurrentAvailabilityInspect {
            working_target: match &context.working_target_availability {
                storyos_application::WorkingTargetAvailability::Current => {
                    contracts::SourceAvailability::Current
                }
                storyos_application::WorkingTargetAvailability::Unavailable => {
                    contracts::SourceAvailability::Unavailable
                }
                storyos_application::WorkingTargetAvailability::Superseded {
                    current_revision_id,
                } => contracts::SourceAvailability::Superseded {
                    current_revision_id: current_revision_id.clone(),
                },
            },
        },
    }
}

fn optional_manifest(manifest_id: Option<&str>) -> contracts::OptionalManifestRef {
    match manifest_id {
        Some(manifest_id) => contracts::OptionalManifestRef::Present {
            manifest_id: manifest_id.to_owned(),
        },
        None => contracts::OptionalManifestRef::Absent,
    }
}

fn inspect_source_class(class: storyos_core::ContextSourceClass) -> contracts::ContextSourceClass {
    match class {
        storyos_core::ContextSourceClass::HostControl => contracts::ContextSourceClass::HostControl,
        storyos_core::ContextSourceClass::AuthorInstruction => {
            contracts::ContextSourceClass::AuthorInstruction
        }
        storyos_core::ContextSourceClass::WorkingTarget => {
            contracts::ContextSourceClass::WorkingTarget
        }
        storyos_core::ContextSourceClass::InstructionBinding => {
            contracts::ContextSourceClass::InstructionBinding
        }
    }
}

fn canonical_body_bytes(body: &contracts::CreateAgentRunRequest) -> Result<Vec<u8>, ApiError> {
    let canonical =
        canonical_json(serde_json::to_value(body).map_err(|_| invalid_request_shape())?);
    serde_json::to_vec(&canonical).map_err(|_| invalid_request_shape())
}

fn canonical_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(canonical_json).collect())
        }
        serde_json::Value::Object(values) => serde_json::Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, canonical_json(value)))
                .collect(),
        ),
        scalar => scalar,
    }
}

fn create_agent_run_error(error: CreateAgentRunError) -> ApiError {
    match error {
        CreateAgentRunError::AssistanceUnavailable => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "assistance_unavailable",
            "Project assistance is unavailable.",
        ),
        CreateAgentRunError::ArchivedProject => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "archived_project",
            "The Project is archived.",
        ),
        CreateAgentRunError::BindingConflict => problem(
            StatusCode::CONFLICT,
            "idempotency_binding_conflict",
            "The createAgentRun binding conflicts.",
        ),
        CreateAgentRunError::ConversationBusy => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "conversation_busy",
            "The Project Conversation already has a non-terminal Run.",
        ),
        CreateAgentRunError::HistoricalAcknowledgementUnavailable => problem(
            StatusCode::CONFLICT,
            "historical_acknowledgement_unavailable",
            "The original createAgentRun acknowledgement cannot be recovered. Refresh to inspect the current Project.",
        ),
        CreateAgentRunError::InaccessibleConversation => resource_unavailable(),
        CreateAgentRunError::InvalidChapterJoin => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_chapter_join",
            "The Working Target Chapter is invalid.",
        ),
        CreateAgentRunError::InvalidChallenge => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "challenge_invalid",
            "The createAgentRun challenge is invalid.",
        ),
        CreateAgentRunError::MissingProject => resource_unavailable(),
        CreateAgentRunError::Unavailable(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "project_store_unavailable",
            "The Project store is unavailable.",
        ),
    }
}
