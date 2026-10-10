use storyos_application::{
    ClaimedAgentRun, CommittedCancellation, CompleteAgentRun, CompleteAgentRunError,
    DestinationRequest, DispatchClaim, DispatchRecord, ModelDispatchStore, NextDispatchWork,
    PreDispatchRefusal, RequestAttempt, RetrievePurpose, StreamStop, WirePayloadProjection,
};
use storyos_core::NativeStreamItem;

use super::PostgresProjectReader;
use crate::agent_run_observation::merge_items;
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
                WorkPhase::Abort(cancellation) => {
                    return Ok(NextDispatchWork::Abort(cancellation));
                }
            }
        }
    }

    #[tracing::instrument(skip_all, level = "debug")]
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
                DestinationRequest::Create(create) if create.successor_of.is_some() => {
                    let assistance = read_assistance_record(client, &claim.project_scope)
                        .await
                        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
                    let route = crate::agent_run_work::route_facts(&run, assistance.as_ref());
                    if run.settled() || storyos_core::admit_route(&route).is_err() {
                        return Ok(None);
                    }
                    return crate::agent_run_successor_dispatch::commit(client, claim, projection)
                        .await;
                }
                DestinationRequest::Create(create) => create,
                DestinationRequest::Retrieve(retrieve) => {
                    return commit_retrieve(client, claim, &run, retrieve, projection).await;
                }
                DestinationRequest::Abort(abort) => {
                    return crate::agent_run_abort::commit(client, claim, &run, abort, projection)
                        .await;
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

    /// Appends the events in order. An event with a known item ID replaces that item.
    #[tracing::instrument(skip_all, level = "debug")]
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
            let target = transaction
                .client
                .query_opt(
                    "SELECT payload::text FROM storyos.model_attempts
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                        AND run_id = $3::text::uuid AND model_attempt_id = $4::text::uuid
                        AND attempt_role IN ('decision', 'successor') AND decision_id IS NULL",
                    &[
                        &claim.project_scope.owner_user_id.as_ref(),
                        &claim.project_scope.project_id.as_ref(),
                        &claim.run_id,
                        &dispatch.model_attempt_id,
                    ],
                )
                .await
                .map_err(complete_database_error)?;
            let Some(target) = target else {
                return Ok(Some(StreamStop::StaleFence));
            };
            if run.status == "cancelled" {
                return Ok(Some(StreamStop::Cancelled(CommittedCancellation {
                    route: crate::model_registration::request_route(&transaction.client, claim)
                        .await?,
                    model_attempt_id: dispatch.model_attempt_id.clone(),
                    response_reference: None,
                    abort_attempt: RequestAttempt::New,
                })));
            }
            if run.settled() {
                return Ok(Some(StreamStop::StaleFence));
            }
            let committed = serde_json::from_str::<serde_json::Value>(&target.get::<_, String>(0))
                .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
            let items = merge_items(&committed["items"], events);
            transaction
                .client
                .execute(
                    "UPDATE storyos.model_attempts
                        SET payload = jsonb_set(payload, '{items}', $5::text::jsonb)
                      WHERE owner_user_id = $1::text::uuid
                        AND project_id = $2::text::uuid
                        AND run_id = $3::text::uuid
                        AND model_attempt_id = $4::text::uuid AND decision_id IS NULL",
                    &[
                        &claim.project_scope.owner_user_id.as_ref(),
                        &claim.project_scope.project_id.as_ref(),
                        &claim.run_id,
                        &dispatch.model_attempt_id,
                        &items.to_string(),
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

    #[tracing::instrument(skip_all, level = "debug")]
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
            let run = load_run_phase(client, claim).await?;
            let settled = WorkPhase::Done(CompleteAgentRun::AlreadySettled);
            let phase = match record {
                DispatchRecord::Refusal(refusal) => {
                    let capability = match refusal {
                        PreDispatchRefusal::CredentialUnavailable => {
                            "destination_credential_unavailable"
                        }
                        PreDispatchRefusal::UnsupportedRequest => "destination_request_unsupported",
                    };
                    if run.status == "cancelled" {
                        crate::agent_run_recovery::refuse_cancellation_duties(
                            client, claim, &run, capability,
                        )
                        .await?;
                        return Ok(settled);
                    }
                    if run.settled() {
                        return Ok(settled);
                    }
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
                } if request.successor_of.is_some() => {
                    if run.settled() {
                        return Ok(settled);
                    }
                    crate::agent_run_successor_dispatch::record(
                        client,
                        claim,
                        dispatch,
                        observation,
                    )
                    .await?
                }
                DispatchRecord::Exchange {
                    claim: dispatch,
                    request: DestinationRequest::Create(request),
                    observation,
                } => {
                    if run.decision_id.is_some()
                        || run.attempt_id.as_deref() != Some(dispatch.model_attempt_id.as_str())
                    {
                        return Ok(settled);
                    }
                    if run.status == "cancelled" {
                        return crate::agent_run_recovery::record_cancelled_create(
                            client,
                            claim,
                            &run,
                            dispatch,
                            observation,
                        )
                        .await;
                    }
                    if run.settled() {
                        return Ok(settled);
                    }
                    crate::agent_run_create_dispatch::record_create(
                        client,
                        claim,
                        &run,
                        dispatch,
                        request,
                        observation,
                    )
                    .await?
                }
                DispatchRecord::Exchange {
                    claim: dispatch,
                    request: DestinationRequest::Abort(abort),
                    observation,
                } => {
                    crate::agent_run_abort::record(client, claim, dispatch, abort, observation)
                        .await?
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

#[cfg(test)]
#[path = "model_gateway_tests.rs"]
pub(crate) mod tests;
