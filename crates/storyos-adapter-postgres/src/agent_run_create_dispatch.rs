//! Records Create observations and builds the Create request of the active decision.

use storyos_application::{
    ClaimedAgentRun, CompleteAgentRunError, CreateRequest, DeclaredTarget, DispatchClaim,
    ModelUsage, Observation, RequestAttempt, RequestContextItem,
};
use storyos_core::{AgentDecisionOutcome, validate_agent_decision};

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
    let retained: serde_json::Value =
        serde_json::from_str(run.attempt_payload.as_deref().unwrap_or("null"))
            .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
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
        Observation::Rejected { reason }
            if request.previous_response_reference.is_some()
                && matches!(
                    reason.as_str(),
                    "continuation_expired" | "continuation_unusable"
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
                &retained,
            )
            .await;
        }
        Observation::Rejected { reason } => {
            return persist_create_result(
                client,
                claim,
                run,
                no_response(dispatch, CreateSettlement::Rejected(&reason)),
                &retained,
            )
            .await;
        }
    };
    let declared: Vec<_> = request
        .declared_targets
        .iter()
        .map(DeclaredTarget::location)
        .collect();
    let result = CreateResult {
        attempt_id: &dispatch.model_attempt_id,
        items: &response.items,
        outcome: validate_agent_decision(response.output.as_ref(), &declared),
        producer_output: response
            .output
            .as_ref()
            .and_then(|output| output.prose_changes.as_deref()),
        usage: response.usage,
        response_reference: response.response_reference.as_deref(),
        settlement: CreateSettlement::Response,
    };
    persist_create_result(client, claim, run, result, &retained).await
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
        response_reference: None,
        settlement,
    }
}

/// Whether a new Create may continue the prior response, or must rebuild the full Context.
pub(crate) enum PriorContext {
    Continue,
    Rebuild,
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
    let declared_targets =
        crate::admitted_proposal_target::load_admitted_targets(client, claim, &run.chapter_id)
            .await?
            .into_iter()
            .map(|target| DeclaredTarget {
                chapter_id: target.chapter_id,
                block_id: target.block_id,
                base_revision_id: target.revision_id,
                collection: target.collection,
                block_text: target.block_text,
            })
            .collect();
    let candidate_revision = crate::candidate_revision_target::admitted(client, claim)
        .await?
        .map(|candidate| {
            let target = candidate
                .operation_requirement
                .candidate_target
                .as_ref()
                .expect("candidate binding");
            candidate
                .selected
                .iter()
                .find(|source| source.source_version == target.revision_id)
                .map(|source| source.content.clone())
                .unwrap_or_default()
        });
    Ok(CreateRequest {
        attempt,
        route: crate::model_registration::request_route(client, claim).await?,
        author_message: run.author_message.clone(),
        chapter_id: run.chapter_id.clone(),
        passage_resolution: assembly.operation_requirement.ordinary_resolution,
        passage_input: crate::passage_collection::passage_input(record, &run.author_message),
        context: assembly
            .selected
            .into_iter()
            .map(|item| RequestContextItem {
                source_class: item.source_class,
                content: item.content,
            })
            .collect(),
        declared_targets,
        candidate_revision,
        previous_response_reference: previous_response_reference(client, claim, run, prior_context)
            .await?,
        successor_of: None,
    })
}

/// The destination reference of the prior binding that an incremental continuation sends.
async fn previous_response_reference(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    prior_context: PriorContext,
) -> Result<Option<String>, CompleteAgentRunError> {
    let prior_binding = match (&run.attempt_payload, prior_context) {
        (Some(payload), _) => serde_json::from_str::<serde_json::Value>(payload)
            .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?
            .pointer("/wire/prior_continuation_binding_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        (None, PriorContext::Rebuild) => None,
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
                    )
                    .await?
                    .prior_binding_id
                }
                None => None,
            }
        }
    };
    let Some(binding) = prior_binding else {
        return Ok(None);
    };
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
    let condition = if reason == "continuation_expired" {
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
