use axum::extract::Query;
use storyos_application::{
    ConversationSelection, CreateAgentRunError, CreateAgentRunInput, ProjectCommandError,
    RefusableCommandError, inspect_agent_run,
};
use storyos_core::{CreateAgentRunRefusal, TransitionOutcome};

use super::command_admission::{
    BodyValidation, ProblemMapping, ProjectCommandRoute, RevisionMismatch, SchemaMismatch,
    TargetValidation, admit, controlled_project,
};
use super::*;

const CREATE_AGENT_RUN: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "createAgentRun",
    command_kind: "createAgentRun",
    method: contracts::CREATE_AGENT_RUN_METHOD,
    path: contracts::CREATE_AGENT_RUN_PATH,
    schema_id: contracts::CREATE_AGENT_RUN_REQUEST_SCHEMA_ID,
    digest_profile: contracts::CREATE_AGENT_RUN_DIGEST_PROFILE,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    body_validation: BodyValidation::AfterChallengeSecret,
    target_validation: TargetValidation::AfterContentType,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    problem_mapping: ProblemMapping::Standard,
};

pub(super) async fn create_agent_run(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    request: Request,
) -> Result<(StatusCode, Json<contracts::CreateAgentRunResponse>), ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[],
        request,
        &CREATE_AGENT_RUN,
        create_agent_run_input,
    )
    .await?;
    let message = &admitted.input.author_message;
    if message.is_empty() || message.chars().count() > 8000 {
        return Err(CREATE_AGENT_RUN.problem(ProjectCommandError::BindingConflict));
    }
    let settlement = admitted
        .store
        .create_agent_run(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| match error {
            RefusableCommandError::RefusedBeforeAdmission(refusal) => refusal_problem(refusal),
            RefusableCommandError::Command(error) => CREATE_AGENT_RUN.problem(error),
        })?;
    admitted.hold_first_acknowledgement().await;
    let applied = match settlement.outcome {
        TransitionOutcome::Applied(applied) => applied,
        TransitionOutcome::NoEffect(reason) => match reason {},
        TransitionOutcome::Conflicted(reason) => match reason {},
        TransitionOutcome::Refused(reason) => match reason {},
    };
    let run = applied.effect;
    Ok((
        StatusCode::ACCEPTED,
        Json(contracts::CreateAgentRunResponse {
            schema_id: contracts::CREATE_AGENT_RUN_RESPONSE_SCHEMA_ID.to_owned(),
            correlation_id: admitted.envelope.correlation_id.clone(),
            project_scope: contract_scope(&admitted.envelope.project_scope),
            command_id: settlement.ids.command_id,
            author_command_admission_id: settlement.ids.author_command_admission_id,
            acknowledgement: contracts::ExportAcknowledgement::Accepted,
            operation_ref: Some(contracts::AgentRunRef::AgentRun {
                run_id: run.run_id.clone(),
            }),
            project: controlled_project(settlement.response),
            project_agent_id: run.project_agent_id.clone(),
            conversation_id: run.conversation_id.clone(),
            memory_settings_revision: run.memory_settings_revision.clone(),
            effect: contracts::CreateAgentRunEffect::Admitted {
                project_agent_id: run.project_agent_id,
                conversation_id: run.conversation_id,
                memory_settings_revision: run.memory_settings_revision,
                run_id: run.run_id,
                project_activity_position: applied.project_activity_position.to_string(),
            },
        }),
    ))
}

/// Validates the conversation and Working Target identities and allocates the new identities.
fn create_agent_run_input(
    body: &contracts::CreateAgentRunRequest,
) -> Result<CreateAgentRunInput, ApiError> {
    let input = &body.create_agent_run_input;
    let conversation_id = match &input.conversation {
        contracts::ConversationSelection::New => Uuid::now_v7().to_string(),
        contracts::ConversationSelection::Existing { conversation_id } => {
            valid_uuid(conversation_id)?;
            conversation_id.clone()
        }
    };
    let mut candidate_target = None;
    let (chapter_id, passage_targets) = match &input.working_target {
        contracts::AssistanceWorkingTarget::CurrentChapter { chapter_id } => (chapter_id, None),
        contracts::AssistanceWorkingTarget::ProposalCandidate {
            source_chapter_id,
            target,
        } => {
            for value in [
                &target.proposal_id,
                &target.operation_id,
                &target.revision_id,
            ] {
                valid_uuid(value)?;
            }
            candidate_target = Some(storyos_core::ProposalCandidateTarget {
                proposal_id: target.proposal_id.clone(),
                operation_id: target.operation_id.clone(),
                revision_id: target.revision_id.clone(),
            });
            (source_chapter_id, None)
        }
        contracts::AssistanceWorkingTarget::PassageCollection {
            source_chapter_id,
            targets,
        } => (
            source_chapter_id,
            Some(super::passage_targets::resolve(targets)?),
        ),
    };
    valid_uuid(chapter_id)?;
    Ok(CreateAgentRunInput {
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
        passage_targets,
        candidate_target,
        run_id: Uuid::now_v7().to_string(),
        conversation_id,
        project_agent_id: Uuid::now_v7().to_string(),
    })
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
    let selection = query
        .model_attempt_id
        .map(storyos_application::AgentRunReadSelection::ModelAttempt)
        .unwrap_or(storyos_application::AgentRunReadSelection::Current);
    let Some(record) = inspect_agent_run(&reader, &scope, &run_id, &selection)
        .await
        .map_err(agent_run_read_error)?
    else {
        return Err(resource_unavailable());
    };
    Ok(Json(contracts::GetAgentRunResponse {
        schema_id: contracts::GET_AGENT_RUN_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: Uuid::now_v7().to_string(),
        project_scope: contract_scope(&scope),
        project_agent_id: record.project_agent_id,
        conversation_id: record.conversation_id,
        captured_memory_settings: match record.captured_memory_settings {
            Some(settings) => contracts::CapturedMemorySettingsInspect::Available {
                memory_settings_revision: record.memory_settings_revision.clone(),
                use_enabled: settings.use_enabled,
                contribution_enabled: settings.contribution_enabled,
            },
            None => contracts::CapturedMemorySettingsInspect::Unavailable,
        },
        memory_settings_revision: record.memory_settings_revision,
        run_id: record.run_id,
        steering_inputs: record
            .steering_inputs
            .into_iter()
            .map(|input| contracts::AgentRunSteeringInspect {
                steering_input_id: input.steering_input_id,
                input_position: input.input_position,
                author_message: input.author_message,
                input_snapshot_id: input.input_snapshot_id,
                model_attempt_id: input.model_attempt_id,
            })
            .collect(),
        status: inspect_status(record.status),
        context: inspect_context(&record.context),
        decision: inspect_decision(&record.decision),
        model_attempt: inspect_model(record.model.as_ref()),
        active_compaction: inspect_active_compaction(record.active_compaction.as_ref()),
        reference_recovery: match record.reference_recovery.as_ref() {
            None => contracts::OptionalReferenceRecoveryInspect::Absent,
            Some(recovery) => contracts::OptionalReferenceRecoveryInspect::Present {
                recovery_id: recovery.recovery_id.clone(),
                disposition: match recovery.disposition {
                    storyos_application::ReferenceRecoveryDisposition::Rebuilt => {
                        contracts::ReferenceRecoveryDisposition::Rebuilt
                    }
                    storyos_application::ReferenceRecoveryDisposition::Blocked => {
                        contracts::ReferenceRecoveryDisposition::Blocked
                    }
                    storyos_application::ReferenceRecoveryDisposition::UnknownCreate => {
                        contracts::ReferenceRecoveryDisposition::UnknownCreate
                    }
                },
                block_reason: recovery.block_reason.clone(),
                predecessor_run_id: recovery.predecessor_run_id.clone(),
                predecessor_continuation_binding_id: recovery
                    .predecessor_continuation_binding_id
                    .clone(),
                run_step_id: recovery.run_step_id.clone(),
                model_invocation_id: recovery.model_invocation_id.clone(),
                model_attempt_id: recovery.model_attempt_id.clone(),
                assembly_manifest_id: recovery.assembly_manifest_id.clone(),
                lossless_provider_reconstruction: recovery.lossless_provider_reconstruction,
                semantic_erasure: recovery.semantic_erasure,
                opaque_reused: recovery.opaque_reused,
                covered_content_included: recovery.covered_content_included,
                predecessor_terminal: recovery.predecessor_terminal,
            },
        },
        original_result_retrieval: inspect_original_result_retrieval(
            record.original_result_retrieval.as_ref(),
        ),
        unknown_create_successor: inspect_unknown_create_successor(
            record.unknown_create_successor.as_ref(),
        ),
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

fn inspect_original_result_retrieval(
    retrieval: Option<&storyos_application::OriginalResultRetrievalInspect>,
) -> contracts::OptionalOriginalResultRetrievalInspect {
    match retrieval {
        None => contracts::OptionalOriginalResultRetrievalInspect::Absent,
        Some(retrieval) => contracts::OptionalOriginalResultRetrievalInspect::Present {
            reconciliation_id: retrieval.reconciliation_id.clone(),
            disposition: match retrieval.disposition {
                storyos_application::OriginalResultRetrievalDisposition::KeptUnknown => {
                    contracts::OriginalResultRetrievalDisposition::KeptUnknown
                }
                storyos_application::OriginalResultRetrievalDisposition::EvidenceOnly => {
                    contracts::OriginalResultRetrievalDisposition::EvidenceOnly
                }
                storyos_application::OriginalResultRetrievalDisposition::Settled => {
                    contracts::OriginalResultRetrievalDisposition::Settled
                }
            },
            keep_reason: retrieval.keep_reason.clone(),
            original_model_attempt_id: retrieval.original_model_attempt_id.clone(),
            response_reference_id: retrieval.response_reference_id.clone(),
            retrieval_attempt_id: retrieval.retrieval_attempt_id.clone(),
            assembly_manifest_id: retrieval.assembly_manifest_id.clone(),
            repeats_original_create: retrieval.repeats_original_create,
            resumes_stream: retrieval.resumes_stream,
            proves_create_idempotency: retrieval.proves_create_idempotency,
            supplies_decision: retrieval.supplies_decision,
            supplies_tool_call: retrieval.supplies_tool_call,
            advances_continuation: retrieval.advances_continuation,
            reservation_released: retrieval.reservation_released,
            usage_kind: retrieval.usage_kind.clone(),
        },
    }
}

fn inspect_unknown_create_successor(
    successor: Option<&storyos_application::UnknownCreateSuccessorInspect>,
) -> contracts::OptionalUnknownCreateSuccessorInspect {
    match successor {
        None => contracts::OptionalUnknownCreateSuccessorInspect::Absent,
        Some(successor) => contracts::OptionalUnknownCreateSuccessorInspect::Present {
            recovery_id: successor.recovery_id.clone(),
            disposition: match successor.disposition {
                storyos_application::UnknownCreateSuccessorDisposition::Fenced => {
                    contracts::UnknownCreateSuccessorDisposition::Fenced
                }
                storyos_application::UnknownCreateSuccessorDisposition::Dispatched => {
                    contracts::UnknownCreateSuccessorDisposition::Dispatched
                }
                storyos_application::UnknownCreateSuccessorDisposition::Paused => {
                    contracts::UnknownCreateSuccessorDisposition::Paused
                }
                storyos_application::UnknownCreateSuccessorDisposition::Prohibited => {
                    contracts::UnknownCreateSuccessorDisposition::Prohibited
                }
            },
            pause_reason: successor.pause_reason.clone(),
            lookup_unavailable_reason: successor.lookup_unavailable_reason.clone(),
            predecessor_model_attempt_id: successor.predecessor_model_attempt_id.clone(),
            successor_model_attempt_id: successor.successor_model_attempt_id.clone(),
            model_invocation_id: successor.model_invocation_id.clone(),
            predecessor_fenced: successor.predecessor_fenced,
            allowance_consumed: successor.allowance_consumed,
            predecessor_usage_kind: successor.predecessor_usage_kind.clone(),
            predecessor_reservation_released: successor.predecessor_reservation_released,
            successor_settles_predecessor: successor.successor_settles_predecessor,
            supplies_tool_call: successor.supplies_tool_call,
            advances_predecessor_continuation: successor.advances_predecessor_continuation,
            reuses_changed_context: successor.reuses_changed_context,
        },
    }
}

fn inspect_status(status: storyos_application::AgentRunStatus) -> contracts::AgentRunStatus {
    match status {
        storyos_application::AgentRunStatus::Queued => contracts::AgentRunStatus::Queued,
        storyos_application::AgentRunStatus::Claimed => contracts::AgentRunStatus::Claimed,
        storyos_application::AgentRunStatus::Waiting => contracts::AgentRunStatus::Waiting,
        storyos_application::AgentRunStatus::Paused => contracts::AgentRunStatus::Paused,
        storyos_application::AgentRunStatus::Completed => contracts::AgentRunStatus::Completed,
        storyos_application::AgentRunStatus::Refused => contracts::AgentRunStatus::Refused,
        storyos_application::AgentRunStatus::Cancelled => contracts::AgentRunStatus::Cancelled,
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
            locations,
            continuation_binding_id,
            opened_proposal_id,
        } => contracts::OptionalDecisionInspect::ProseChange {
            decision_id: decision_id.clone(),
            selected: *selected,
            text: text.clone(),
            producer_input: producer_input.clone(),
            locations: locations.clone(),
            continuation: inspect_continuation(continuation_binding_id.as_deref()),
            authoritative: false,
            opened_proposal: match opened_proposal_id.as_deref() {
                Some(proposal_id) => contracts::OptionalOpenedProposalInspect::Present {
                    proposal_id: proposal_id.to_owned(),
                },
                None => contracts::OptionalOpenedProposalInspect::Absent,
            },
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

fn inspect_active_compaction(
    compaction: Option<&storyos_application::ActiveCompactionInspect>,
) -> contracts::OptionalActiveCompactionInspect {
    let Some(compaction) = compaction else {
        return contracts::OptionalActiveCompactionInspect::Absent;
    };
    let installed = match (
        compaction.installed_run_step_id.clone(),
        compaction.installed_model_invocation_id.clone(),
        compaction.installed_model_attempt_id.clone(),
    ) {
        (Some(run_step_id), Some(model_invocation_id), Some(model_attempt_id)) => {
            contracts::OptionalCompactionInstallInspect::Present {
                run_step_id,
                model_invocation_id,
                model_attempt_id,
            }
        }
        _ => contracts::OptionalCompactionInstallInspect::Absent,
    };
    contracts::OptionalActiveCompactionInspect::Present {
        compaction_id: compaction.compaction_id.clone(),
        install_state: match compaction.install_state {
            storyos_application::ActiveCompactionInstallState::Staged => {
                contracts::ActiveCompactionInstallState::Staged
            }
            storyos_application::ActiveCompactionInstallState::Installed => {
                contracts::ActiveCompactionInstallState::Installed
            }
            storyos_application::ActiveCompactionInstallState::Refused => {
                contracts::ActiveCompactionInstallState::Refused
            }
        },
        prior_model_attempt_id: compaction.prior_model_attempt_id.clone(),
        prior_manifest_id: compaction.prior_manifest_id.clone(),
        prior_run_step_id: compaction.prior_run_step_id.clone(),
        producer_model_attempt_id: compaction.producer_model_attempt_id.clone(),
        producer_manifest_id: compaction.producer_manifest_id.clone(),
        producer_invocation_id: compaction.producer_invocation_id.clone(),
        producer: compaction.producer.clone(),
        mapping_kind: match compaction.mapping_kind {
            storyos_application::ActiveCompactionMappingKind::HostManaged => {
                contracts::ActiveCompactionMappingKind::HostManaged
            }
            storyos_application::ActiveCompactionMappingKind::Native => {
                contracts::ActiveCompactionMappingKind::Native
            }
        },
        mapping_revision: compaction.mapping_revision.clone(),
        known_inputs: compaction
            .known_inputs
            .iter()
            .map(|input| match input {
                storyos_application::ActiveCompactionKnownInput::ModelAttempt { id } => {
                    contracts::ActiveCompactionKnownInput::ModelAttempt { id: id.clone() }
                }
                storyos_application::ActiveCompactionKnownInput::Manifest { id } => {
                    contracts::ActiveCompactionKnownInput::Manifest { id: id.clone() }
                }
                storyos_application::ActiveCompactionKnownInput::Projection {
                    source_class,
                    source_version,
                } => contracts::ActiveCompactionKnownInput::Projection {
                    source_class: source_class.clone(),
                    source_version: source_version.clone(),
                },
            })
            .collect(),
        output_text: compaction.output_text.clone(),
        usage: contracts::AgentRunUsageInspect {
            kind: compaction.usage_kind.clone(),
        },
        loss_facts: compaction.loss_facts.clone(),
        refusal_reason: compaction.refusal_reason.clone(),
        installed,
        preserved_item_ids: compaction.preserved_item_ids.clone(),
        admission: contracts::ContinuationAdmissionInspect {
            processing_destination_identity: compaction
                .admission
                .processing_destination_identity
                .clone(),
            evidence_revision: compaction.admission.evidence_revision.clone(),
            model_registration_revision: compaction.admission.model_registration_revision.clone(),
            adapter_mapping: compaction.admission.adapter_mapping.clone(),
            project_model_use_binding_revision: compaction
                .admission
                .project_model_use_binding_revision
                .clone(),
            external_compatibility_decision: compaction
                .admission
                .external_compatibility_decision
                .clone(),
        },
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
            prior_continuation: inspect_continuation(
                model.prior_continuation_binding_id.as_deref(),
            ),
            known_prior_continuation: inspect_continuation(
                model.known_prior_continuation_binding_id.as_deref(),
            ),
            input_mapping: match model.input_mapping {
                storyos_application::AgentRunInputMapping::None => {
                    contracts::ContinuationInputMappingInspect::None
                }
                storyos_application::AgentRunInputMapping::Incremental => {
                    contracts::ContinuationInputMappingInspect::Incremental
                }
                storyos_application::AgentRunInputMapping::Full => {
                    contracts::ContinuationInputMappingInspect::Full
                }
                storyos_application::AgentRunInputMapping::NewTransport => {
                    contracts::ContinuationInputMappingInspect::NewTransport
                }
            },
            admission: Box::new(contracts::ContinuationAdmissionInspect {
                processing_destination_identity: model
                    .admission
                    .processing_destination_identity
                    .clone(),
                evidence_revision: model.admission.evidence_revision.clone(),
                model_registration_revision: model.admission.model_registration_revision.clone(),
                adapter_mapping: model.admission.adapter_mapping.clone(),
                project_model_use_binding_revision: model
                    .admission
                    .project_model_use_binding_revision
                    .clone(),
                external_compatibility_decision: model
                    .admission
                    .external_compatibility_decision
                    .clone(),
            }),
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
        candidate_target: record
            .operation_requirement
            .candidate_target
            .as_ref()
            .map(|target| contracts::ProposalCandidateTarget {
                proposal_id: target.proposal_id.clone(),
                operation_id: target.operation_id.clone(),
                revision_id: target.revision_id.clone(),
            }),
        passage_targets: record
            .operation_requirement
            .passage_targets
            .as_ref()
            .map(|targets| {
                targets
                    .iter()
                    .map(|target| contracts::PassageTarget {
                        chapter_id: target.chapter_id.clone(),
                        base_authoritative_revision_id: target
                            .base_authoritative_revision_id
                            .clone(),
                        manuscript_block_ids: target.manuscript_block_ids.clone(),
                    })
                    .collect()
            }),
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

fn refusal_problem(refusal: CreateAgentRunRefusal) -> ApiError {
    match refusal {
        CreateAgentRunRefusal::AssistanceUnavailable => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "assistance_unavailable",
            "Project assistance is unavailable.",
        ),
        CreateAgentRunRefusal::ArchivedProject => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "archived_project",
            "The Project is archived.",
        ),
        CreateAgentRunRefusal::ConversationBusy => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "conversation_busy",
            "The Project Conversation already has a non-terminal Run.",
        ),
        CreateAgentRunRefusal::InvalidChapterJoin => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_chapter_join",
            "The Working Target Chapter is invalid.",
        ),
        CreateAgentRunRefusal::InaccessibleConversation | CreateAgentRunRefusal::MissingProject => {
            resource_unavailable()
        }
    }
}

fn agent_run_read_error(error: CreateAgentRunError) -> ApiError {
    CREATE_AGENT_RUN.problem(match error {
        CreateAgentRunError::BindingConflict => ProjectCommandError::BindingConflict,
        CreateAgentRunError::Unavailable(source) => ProjectCommandError::Unavailable(source),
    })
}
