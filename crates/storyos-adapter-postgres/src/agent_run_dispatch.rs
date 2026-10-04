use storyos_application::{
    ClaimedAgentRun, CompleteAgentRun, CompleteAgentRunError, CreateRequest, DeclaredTarget,
    DestinationRequest, DispatchClaim, DispatchRecord, ModelDispatchStore, ModelUsage,
    NextDispatchWork, Observation, PreDispatchRefusal, RequestAttempt, RetrievePurpose, StreamStop,
    WirePayloadProjection,
};
use storyos_core::{AgentDecisionOutcome, NativeStreamItem, validate_agent_decision};

use super::PostgresProjectReader;
use crate::agent_run_observation::{CreateResult, encode_items, persist_create_result};
use crate::agent_run_work::{
    CreateAdmission, RunPhaseRow, WorkPhase, admit_create, complete_challenge_error,
    complete_database_error, hold_if_requested, load_run_phase, settle_one_phase, update_run,
};
use crate::update_project_assistance::read_assistance_record;

impl ModelDispatchStore for PostgresProjectReader {
    async fn next_dispatch_work(
        &self,
        claim: &ClaimedAgentRun,
    ) -> Result<NextDispatchWork, CompleteAgentRunError> {
        let lease_seconds = self.agent_run_lease_seconds()?;
        loop {
            let transaction = self
                .begin_serializable_project_command_transaction(&claim.project_scope)
                .await
                .map_err(complete_challenge_error)?;
            let phase = async {
                let phase = settle_one_phase(&transaction.client, claim, lease_seconds).await?;
                advance_steering(&transaction.client, claim, lease_seconds, phase).await
            }
            .await;
            let phase = match phase {
                Ok(phase) => phase,
                Err(error) => {
                    let _rollback = transaction.rollback().await;
                    return Err(error);
                }
            };
            transaction
                .commit()
                .await
                .map_err(complete_challenge_error)?;
            match phase {
                WorkPhase::Done(result) => return Ok(NextDispatchWork::Settled(result)),
                WorkPhase::Hold(kind) => hold_if_requested(kind).await,
                WorkPhase::Dispatch(request) => return Ok(NextDispatchWork::Request(*request)),
            }
        }
    }

    async fn commit_dispatch_claim(
        &self,
        claim: &ClaimedAgentRun,
        request: &DestinationRequest,
        projection: &WirePayloadProjection,
    ) -> Result<Option<DispatchClaim>, CompleteAgentRunError> {
        let lease_seconds = self.agent_run_lease_seconds()?;
        let transaction = self
            .begin_serializable_project_command_transaction(&claim.project_scope)
            .await
            .map_err(complete_challenge_error)?;
        let client = &transaction.client;
        let claimed = async {
            let run = load_run_phase(client, claim).await?;
            let request = match request {
                DestinationRequest::Create(create) => create,
                DestinationRequest::Retrieve(retrieve) => {
                    return commit_retrieve(client, claim, &run, retrieve, projection).await;
                }
            };
            if run.settled()
                || run.attempt_id.is_some()
                || run.author_message != request.author_message
                || run.chapter_id != request.chapter_id
                || (run.decision_position == "0"
                    && crate::agent_run_steering::advance(client, claim, lease_seconds).await?)
            {
                return Ok(None);
            }
            let assistance = read_assistance_record(client, &claim.project_scope)
                .await
                .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
            let CreateAdmission::Dispatch(rebuild) =
                admit_create(client, claim, &run, assistance.as_ref()).await?
            else {
                return Ok(None);
            };
            let assistance = assistance.ok_or_else(|| {
                CompleteAgentRunError::Unavailable(Box::new(std::io::Error::other(
                    "Dispatch requires current assistance admission",
                )))
            })?;
            let model_attempt_id = crate::agent_run_attempt::persist_uncertain_attempt(
                client,
                claim,
                &run,
                &assistance,
                rebuild.as_deref(),
                projection,
            )
            .await?;
            if let Some(dispatch) = rebuild.as_deref() {
                crate::agent_run_expiry::insert_recovery(
                    client,
                    claim,
                    &run.conversation_id,
                    &dispatch.row,
                    Some(model_attempt_id.as_str()),
                )
                .await?;
            }
            Ok(Some(DispatchClaim { model_attempt_id }))
        }
        .await;
        match claimed {
            Ok(Some(dispatch)) => {
                transaction
                    .commit()
                    .await
                    .map_err(complete_challenge_error)?;
                Ok(Some(dispatch))
            }
            Ok(None) => {
                let _rollback = transaction.rollback().await;
                Ok(None)
            }
            Err(error) => {
                let _rollback = transaction.rollback().await;
                Err(error)
            }
        }
    }

    async fn append_model_stream_events(
        &self,
        claim: &ClaimedAgentRun,
        dispatch: &DispatchClaim,
        events: &[NativeStreamItem],
    ) -> Result<Option<StreamStop>, CompleteAgentRunError> {
        let transaction = self
            .begin_serializable_project_command_transaction(&claim.project_scope)
            .await
            .map_err(complete_challenge_error)?;
        let appended = async {
            let run = match load_run_phase(&transaction.client, claim).await {
                Err(CompleteAgentRunError::StaleFence) => return Ok(Some(StreamStop::StaleFence)),
                loaded => loaded?,
            };
            if run.status == "cancelled" {
                return Ok(Some(StreamStop::Cancelled));
            }
            if run.settled()
                || run.decision_id.is_some()
                || run.attempt_id.as_deref() != Some(dispatch.model_attempt_id.as_str())
            {
                return Ok(Some(StreamStop::StaleFence));
            }
            transaction
                .client
                .execute(
                    "UPDATE storyos.model_attempts
                        SET payload = jsonb_set(payload, '{items}', $5::text::jsonb)
                      WHERE owner_user_id = $1::text::uuid
                        AND project_id = $2::text::uuid
                        AND run_id = $3::text::uuid
                        AND model_attempt_id = $4::text::uuid
                        AND attempt_role = 'decision' AND decision_id IS NULL",
                    &[
                        &claim.project_scope.owner_user_id.as_ref(),
                        &claim.project_scope.project_id.as_ref(),
                        &claim.run_id,
                        &dispatch.model_attempt_id,
                        &encode_items(events).to_string(),
                    ],
                )
                .await
                .map_err(complete_database_error)?;
            Ok(None)
        }
        .await;
        match appended {
            Ok(None) => {
                transaction
                    .commit()
                    .await
                    .map_err(complete_challenge_error)?;
                Ok(None)
            }
            stopped => {
                let _rollback = transaction.rollback().await;
                stopped
            }
        }
    }

    async fn record(
        &self,
        claim: &ClaimedAgentRun,
        record: DispatchRecord<'_>,
    ) -> Result<(), CompleteAgentRunError> {
        let lease_seconds = self.agent_run_lease_seconds()?;
        let transaction = self
            .begin_serializable_project_command_transaction(&claim.project_scope)
            .await
            .map_err(complete_challenge_error)?;
        let client = &transaction.client;
        let phase = async {
            let run = match load_run_phase(client, claim).await {
                Err(CompleteAgentRunError::StaleFence) => {
                    return cancelled_evidence(client, claim, record).await;
                }
                loaded => loaded?,
            };
            let settled = WorkPhase::Done(CompleteAgentRun::AlreadySettled);
            let phase = match record {
                DispatchRecord::Refusal(_) if run.settled() => return Ok(settled),
                DispatchRecord::Refusal(refusal) => {
                    let capability = match refusal {
                        PreDispatchRefusal::CredentialUnavailable => {
                            "destination_credential_unavailable"
                        }
                        PreDispatchRefusal::UnsupportedRequest => "destination_request_unsupported",
                    };
                    update_run(
                        client,
                        claim,
                        "refused",
                        Some(&serde_json::json!({
                            "kind": "execution_refused",
                            "capability": capability
                        })),
                        /*clear_lease*/ true,
                    )
                    .await?;
                    WorkPhase::Done(CompleteAgentRun::Settled)
                }
                DispatchRecord::Exchange {
                    claim: dispatch,
                    request: DestinationRequest::Create(request),
                    observation,
                } => {
                    if run.settled()
                        || run.decision_id.is_some()
                        || run.attempt_id.as_deref() != Some(dispatch.model_attempt_id.as_str())
                    {
                        return Ok(settled);
                    }
                    record_create(client, claim, &run, dispatch, request, observation).await?
                }
                DispatchRecord::Exchange {
                    request: DestinationRequest::Retrieve(_),
                    ..
                } if run.settled() && run.status != "cancelled" => return Ok(settled),
                DispatchRecord::Exchange {
                    claim: dispatch,
                    request: DestinationRequest::Retrieve(retrieve),
                    observation,
                } => match retrieve.purpose {
                    RetrievePurpose::OriginalResult => {
                        crate::agent_run_retrieval_dispatch::record_original_result(
                            client,
                            claim,
                            &run,
                            dispatch,
                            observation,
                        )
                        .await?
                    }
                    RetrievePurpose::LateResult => {
                        crate::agent_run_retrieval_dispatch::record_late_result(
                            client,
                            claim,
                            dispatch,
                            observation,
                        )
                        .await?;
                        WorkPhase::Hold("recovery")
                    }
                },
            };
            advance_steering(client, claim, lease_seconds, phase).await
        }
        .await;
        let phase = match phase {
            Ok(phase) => phase,
            Err(error) => {
                let _rollback = transaction.rollback().await;
                return Err(error);
            }
        };
        transaction
            .commit()
            .await
            .map_err(complete_challenge_error)?;
        if let WorkPhase::Hold(kind) = phase {
            hold_if_requested(kind).await;
        }
        Ok(())
    }
}

impl PostgresProjectReader {
    fn agent_run_lease_seconds(&self) -> Result<i64, CompleteAgentRunError> {
        i64::try_from(self.readable_export_lease_ttl.as_secs())
            .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))
    }
}

async fn advance_steering(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    lease_seconds: i64,
    phase: WorkPhase,
) -> Result<WorkPhase, CompleteAgentRunError> {
    if let WorkPhase::Done(CompleteAgentRun::Settled) = phase
        && crate::agent_run_steering::advance(client, claim, lease_seconds).await?
    {
        return Ok(WorkPhase::Hold("steering"));
    }
    Ok(phase)
}

/// A cancelled Run fences this claim; it keeps only the reference of an unknown create.
async fn cancelled_evidence(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    record: DispatchRecord<'_>,
) -> Result<WorkPhase, CompleteAgentRunError> {
    if let DispatchRecord::Exchange {
        claim: dispatch,
        request: DestinationRequest::Create(_),
        observation: Observation::OutcomeUnknown { response_reference },
    } = record
        && crate::agent_run_recovery::record_cancelled_evidence(
            client,
            claim,
            dispatch,
            response_reference.as_ref(),
        )
        .await?
    {
        return Ok(WorkPhase::Done(CompleteAgentRun::AlreadySettled));
    }
    Err(CompleteAgentRunError::StaleFence)
}

async fn commit_retrieve(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    request: &storyos_application::RetrieveRequest,
    projection: &WirePayloadProjection,
) -> Result<Option<DispatchClaim>, CompleteAgentRunError> {
    let role = match request.purpose {
        RetrievePurpose::OriginalResult => "retrieval",
        RetrievePurpose::LateResult => "late_retrieval",
    };
    let claimed = client
        .query_one(
            "SELECT count(*) FROM storyos.model_attempts
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid AND attempt_role = $4 AND decision_position = 0",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &role,
            ],
        )
        .await
        .map_err(complete_database_error)?
        .get::<_, i64>(0);
    if claimed > 0 || (run.settled() && run.status != "cancelled") {
        return Ok(None);
    }
    crate::agent_run_retrieval_dispatch::commit(client, claim, run, request, projection)
        .await
        .map(Some)
}

async fn record_create(
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
        Observation::NotSubmitted | Observation::Rejected { .. } => {
            let result = CreateResult {
                attempt_id: &dispatch.model_attempt_id,
                items: &[],
                outcome: AgentDecisionOutcome::NoDecision,
                producer_output: None,
                usage: ModelUsage::Unknown,
            };
            return persist_create_result(client, claim, run, result, &retained).await;
        }
    };
    let declared: Vec<_> = request
        .declared_targets
        .iter()
        .map(|target| {
            (
                target.chapter_id.clone(),
                target.block_id.clone(),
                target.base_revision_id.clone(),
            )
        })
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
    };
    persist_create_result(client, claim, run, result, &retained).await
}

/// Builds the provider-neutral Create request for the active decision.
pub(crate) async fn create_request(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    record: &serde_json::Value,
    attempt: RequestAttempt,
) -> Result<CreateRequest, CompleteAgentRunError> {
    let passage_resolution = storyos_core::decode_assembly_record(record)
        .ok_or_else(|| {
            CompleteAgentRunError::Unavailable(Box::new(std::io::Error::other(
                "Invalid retained Context",
            )))
        })?
        .operation_requirement
        .ordinary_resolution;
    let declared_targets =
        crate::admitted_proposal_target::load_admitted_targets(client, claim, &run.chapter_id)
            .await?
            .into_iter()
            .map(|target| DeclaredTarget {
                chapter_id: target.chapter_id,
                block_id: target.block_id,
                base_revision_id: target.revision_id,
                collection: target.collection,
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
        author_message: run.author_message.clone(),
        chapter_id: run.chapter_id.clone(),
        passage_resolution,
        passage_input: crate::passage_collection::passage_input(record, &run.author_message),
        declared_targets,
        candidate_revision,
    })
}

#[cfg(test)]
#[path = "model_gateway_tests.rs"]
mod tests;
