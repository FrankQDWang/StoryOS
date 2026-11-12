//! Records Create observations and builds the Create request of the active decision.

use storyos_application::{
    CacheUse, ClaimedAgentRun, CompleteAgentRunError, CreatePurpose, CreateRequest, DeclaredTarget,
    DispatchClaim, ModelResponse, ModelUsage, Observation, RequestAttempt, RequestContextItem,
};
use storyos_core::{
    AgentDecisionOutcome, FinishRuleOutcome, ResponseDisposition, ResponseFacts, apply_finish_rule,
    response_disposition, validate_agent_decision,
};

use crate::agent_run_observation::{CreateResult, CreateSettlement, persist_create_result};
use crate::agent_run_work::{RunPhaseRow, WorkPhase};

pub(crate) async fn record_create(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    dispatch: &DispatchClaim,
    request: &CreateRequest,
    observation: Observation,
) -> Result<WorkPhase, CompleteAgentRunError> {
    let mut retained: serde_json::Value =
        serde_json::from_str(run.attempt_payload.as_deref().unwrap_or("null"))
            .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    let probe = matches!(request.purpose, CreatePurpose::ToolQualification(_));
    let response = match observation {
        Observation::Terminal(response) => response,
        Observation::OutcomeUnknown { response_reference } => {
            return crate::agent_run_recovery::record_unknown_create(
                client,
                claim,
                run,
                dispatch,
                response_reference.as_ref(),
            )
            .await;
        }
        // A real adapter appends the native code after the condition.
        Observation::Rejected { reason }
            if !probe
                && request.previous_response_reference.is_some()
                && matches!(
                    reason.split(':').next(),
                    Some("continuation_expired" | "continuation_unusable")
                ) =>
        {
            return record_continuation_rejection(client, claim, run, dispatch, request, &reason)
                .await;
        }
        Observation::NotSubmitted => {
            return persist_create_result(
                client,
                claim,
                run,
                no_response(dispatch, CreateSettlement::NotSubmitted),
                retained,
            )
            .await;
        }
        Observation::Rejected { reason } => {
            return persist_create_result(
                client,
                claim,
                run,
                no_response(dispatch, CreateSettlement::Rejected(&reason)),
                retained,
            )
            .await;
        }
    };
    let declared: Vec<_> = request
        .declared_targets
        .iter()
        .map(DeclaredTarget::location)
        .collect();
    // A Tool qualification probe makes no Agent Decision (ADR 0049). The batch validates
    // against the Exposure that the Step Snapshot of this Attempt pinned and against the current
    // authority (ADR 0084).
    let outcome = if probe {
        AgentDecisionOutcome::NoDecision
    } else {
        let batch =
            crate::agent_run_tool_call::validate_batch(client, &retained, &response).await?;
        if let Some(evidence) = crate::agent_run_tool_call::batch_evidence(&batch) {
            retained["tool_call_batch"] = evidence;
        }
        let output = response.output.as_ref();
        let citable = crate::research_artifact::decision_citable(client, claim, output).await?;
        if let Some(refusal) = crate::research_artifact::refusal_evidence(output, &citable) {
            retained["synthesis_refusal"] = refusal;
        }
        validate_agent_decision(output, &declared, &citable, batch)
    };
    let outcome = match apply_finish_rule(request.tool_choice, outcome, &response.items) {
        FinishRuleOutcome::Settle(outcome) => outcome,
        rule @ (FinishRuleOutcome::NextRunStep | FinishRuleOutcome::NoResult) => {
            let settlement = match rule {
                FinishRuleOutcome::NextRunStep => CreateSettlement::NextRunStep,
                _ => CreateSettlement::NoResult,
            };
            let result = CreateResult {
                settlement,
                ..response_result(dispatch, &response)
            };
            return persist_create_result(client, claim, run, result, retained).await;
        }
    };
    // Core decides the disposition of the response (ADR 0084).
    let settlement = match response_disposition(&ResponseFacts {
        probe,
        outcome: &outcome,
        items: &response.items,
    }) {
        ResponseDisposition::FunctionRequested => CreateSettlement::FunctionRequested,
        ResponseDisposition::ToolCallsPending => CreateSettlement::ToolCallsPending,
        ResponseDisposition::Decision | ResponseDisposition::NoDecision => {
            CreateSettlement::Response
        }
    };
    let result = CreateResult {
        outcome,
        producer_output: response
            .output
            .as_ref()
            .and_then(|output| output.prose_changes.as_deref()),
        settlement,
        ..response_result(dispatch, &response)
    };
    persist_create_result(client, claim, run, result, retained).await
}

fn response_result<'a>(
    dispatch: &'a DispatchClaim,
    response: &'a ModelResponse,
) -> CreateResult<'a> {
    CreateResult {
        attempt_id: &dispatch.model_attempt_id,
        items: &response.items,
        outcome: AgentDecisionOutcome::NoDecision,
        producer_output: None,
        usage: response.usage,
        cache_use: response.cache_use,
        response_reference: response.response_reference.as_deref(),
        settlement: CreateSettlement::Response,
    }
}

fn no_response<'a>(
    dispatch: &'a DispatchClaim,
    settlement: CreateSettlement<'a>,
) -> CreateResult<'a> {
    CreateResult {
        attempt_id: &dispatch.model_attempt_id,
        items: &[],
        outcome: AgentDecisionOutcome::NoDecision,
        producer_output: None,
        usage: ModelUsage::Unknown,
        cache_use: CacheUse::Unknown,
        response_reference: None,
        settlement,
    }
}

/// Whether a new Create may continue the prior response, or must rebuild the full Context.
pub(crate) enum PriorContext {
    Continue,
    Rebuild,
    /// The later request of a compaction: the summary replaces the prior response, and the
    /// request starts a new transport.
    Summary,
}

/// Builds the provider-neutral Create request for the active decision.
pub(crate) async fn create_request(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    record: &serde_json::Value,
    attempt: RequestAttempt,
    prior_context: PriorContext,
) -> Result<CreateRequest, CompleteAgentRunError> {
    let assembly = storyos_core::decode_assembly_record(record).ok_or_else(|| {
        CompleteAgentRunError::Unavailable(Box::new(std::io::Error::other(
            "Invalid retained Context",
        )))
    })?;
    let declared_targets = declared_targets(client, claim, &run.chapter_id).await?;
    let probe = crate::agent_run_tool_probe::probe_request(client, claim, run).await?;
    let tool_exposure = crate::agent_run_tool_exposure::tool_exposure(client, claim, run).await?;
    let tool_outputs = crate::agent_run_tool_result::tool_outputs(client, claim, &assembly).await?;
    let tool_choice = crate::agent_run_finish_rule::tool_choice(client, claim, run).await?;
    let previous_response_reference = match &probe {
        Some(probe) => probe.previous_response_reference.clone(),
        None => {
            previous_response_reference(client, claim, run, prior_context, &tool_exposure.digest())
                .await?
        }
    };
    let input = record_input(record, assembly, &run.author_message);
    Ok(CreateRequest {
        attempt,
        purpose: probe.map_or(CreatePurpose::Decision, |probe| {
            CreatePurpose::ToolQualification(probe.request)
        }),
        route: crate::model_registration::request_route(client, claim).await?,
        author_message: run.author_message.clone(),
        chapter_id: run.chapter_id.clone(),
        passage_resolution: input.passage_resolution,
        passage_input: input.passage_input,
        context: input.context,
        declared_targets,
        selection_references: input.selection_references,
        previous_response_reference,
        tool_exposure,
        tool_choice,
        successor_of: None,
        tool_outputs,
    })
}

/// The parts of the request input that the decision record of the active RunStep gives.
pub(crate) struct RecordInput {
    pub passage_resolution: Option<storyos_core::OrdinaryPassageResolution>,
    pub passage_input: Option<serde_json::Value>,
    pub context: Vec<RequestContextItem>,
    pub selection_references: Vec<storyos_core::AdmittedSelection>,
}

pub(crate) fn record_input(
    record: &serde_json::Value,
    assembly: storyos_core::CurrentPassageAssemblyRecord,
    author_message: &str,
) -> RecordInput {
    RecordInput {
        passage_resolution: assembly.operation_requirement.ordinary_resolution,
        passage_input: crate::passage_collection::passage_input(record, author_message),
        context: assembly
            .selected
            .into_iter()
            .map(|item| RequestContextItem {
                source_class: item.source_class,
                content: item.content,
            })
            .collect(),
        selection_references: assembly
            .operation_requirement
            .selection_references
            .unwrap_or_default(),
    }
}

/// The admitted Proposal targets that a request of the active decision declares.
pub(crate) async fn declared_targets(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    chapter_id: &str,
) -> Result<Vec<DeclaredTarget>, CompleteAgentRunError> {
    let mut declared: Vec<DeclaredTarget> =
        crate::admitted_proposal_target::load_admitted_targets(client, claim, chapter_id)
            .await?
            .into_iter()
            .map(|target| DeclaredTarget {
                chapter_id: target.chapter_id,
                block_id: target.block_id,
                base_revision_id: target.revision_id,
                collection: target.collection,
                kind: storyos_core::DeclaredTargetKind::ChapterBlock,
                block_text: target.block_text,
            })
            .collect();
    declared.extend(crate::candidate_revision_target::declared(client, claim).await?);
    crate::joinable_blocks::mark(client, claim, &mut declared).await?;
    Ok(declared)
}

/// The destination reference of the prior binding that an incremental continuation sends.
async fn previous_response_reference(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    prior_context: PriorContext,
    tool_exposure_digest: &str,
) -> Result<Option<String>, CompleteAgentRunError> {
    let prior_binding = match (&run.attempt_payload, prior_context) {
        (Some(payload), _) => serde_json::from_str::<serde_json::Value>(payload)
            .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?
            .pointer("/wire/prior_continuation_binding_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        (None, PriorContext::Rebuild | PriorContext::Summary) => None,
        (None, PriorContext::Continue) => {
            match crate::update_project_assistance::read_assistance_record(
                client,
                &claim.project_scope,
            )
            .await
            .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?
            {
                Some(assistance) => {
                    crate::agent_run_continuation::decide_continuation(
                        client,
                        claim,
                        &run.conversation_id,
                        &run.author_message,
                        &assistance,
                        tool_exposure_digest,
                    )
                    .await?
                    .prior_binding_id
                }
                None => None,
            }
        }
    };
    match prior_binding {
        Some(binding) => binding_reference(client, claim, &binding).await,
        None => Ok(None),
    }
}

/// The destination reference that the Model Attempt of one continuation binding produced.
pub(crate) async fn binding_reference(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    binding: &str,
) -> Result<Option<String>, CompleteAgentRunError> {
    Ok(client
        .query_opt(
            "SELECT payload->>'response_reference' FROM storyos.model_attempts
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND continuation_binding_id = $3::text::uuid",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &binding,
            ],
        )
        .await
        .map_err(crate::agent_run_work::complete_database_error)?
        .and_then(|row| row.get(0)))
}

/// A destination rejection confirms the prior reference condition. The rejected Create keeps its
/// own Destination Attempt and Outbound Disclosure Event, and the decision is rebuilt in this Run.
async fn record_continuation_rejection(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    dispatch: &DispatchClaim,
    request: &CreateRequest,
    reason: &str,
) -> Result<WorkPhase, CompleteAgentRunError> {
    let condition = if reason.starts_with("continuation_expired") {
        "confirmed_expired"
    } else {
        "confirmed_unusable"
    };
    client
        .execute(
            "WITH confirmed AS (
               UPDATE storyos.model_attempts
                  SET payload = jsonb_set(payload, '{produced_binding,reference_condition}',
                                          to_jsonb($5::text))
                WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                  AND payload->>'response_reference' = $4
                  AND payload ? 'produced_binding'
            RETURNING model_attempt_id),
             rejected AS (
               UPDATE storyos.model_attempts
                  SET attempt_role = 'rejected_create',
                      dispatch_state = 'settled',
                      payload = payload || jsonb_build_object('rejection', $6::text)
                WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                  AND model_attempt_id = $3::text::uuid AND attempt_role = 'decision'
            RETURNING run_id)
             UPDATE storyos.context_assembly_manifests
                SET destination_context_manifest_id = NULL,
                    outbound_disclosure_manifest_id = NULL
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id IN (SELECT run_id FROM rejected)
                AND context_assembly_manifest_id = $7::text::uuid",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &dispatch.model_attempt_id,
                &request.previous_response_reference,
                &condition,
                &reason,
                &run.assembly_manifest_id,
            ],
        )
        .await
        .map_err(crate::agent_run_work::complete_database_error)?;
    Ok(WorkPhase::Hold("rejection"))
}

#[cfg(test)]
#[path = "agent_run_create_dispatch_tests.rs"]
mod tests;
