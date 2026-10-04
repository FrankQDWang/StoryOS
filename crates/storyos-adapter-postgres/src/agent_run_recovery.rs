//! Recovery phases of one unknown create: original-result retrieval and the bounded successor.

use storyos_application::{
    ClaimedAgentRun, CompleteAgentRun, CompleteAgentRunError, DestinationRequest, DispatchClaim,
    Observation, ProjectAssistanceRecord, ProjectScope, ReferenceRetrieval, ResponseReference,
};
use storyos_core::{LookupUnavailable, SuccessorLookup};

use crate::agent_run_retrieval::{
    RetrievalAdvance, RetrievalFence, RetrievalRunWrite, RetrievalWork,
    advance_original_result_retrieval,
};
use crate::agent_run_successor::SuccessorWork;
use crate::agent_run_work::{RunPhaseRow, WorkPhase, complete_database_error, update_run};
use crate::update_project_assistance::read_assistance_record;

/// Advances the retrieval subject and then the successor marker of the active decision.
pub(crate) async fn advance(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    payload: &serde_json::Value,
    assistance: Option<&ProjectAssistanceRecord>,
) -> Result<WorkPhase, CompleteAgentRunError> {
    let follow_successor = payload.get("unknown_create_successor").is_some();
    if payload.get("original_result_retrieval").is_some() {
        let work = advance_original_result_retrieval(
            client,
            claim,
            payload,
            RetrievalAdvance {
                attempt_id: run.attempt_id.as_deref().unwrap_or_default(),
                conversation_id: &run.conversation_id,
                run_status: &run.status,
            },
            RetrievalFence::Open,
            if follow_successor {
                RetrievalRunWrite::Defer
            } else {
                RetrievalRunWrite::Write
            },
        )
        .await?;
        match work {
            RetrievalWork::Retrieve(request) => {
                return Ok(WorkPhase::Dispatch(Box::new(DestinationRequest::Retrieve(
                    request,
                ))));
            }
            RetrievalWork::Done(result) if !follow_successor => {
                return Ok(WorkPhase::Done(result));
            }
            RetrievalWork::Done(_) => {}
        }
    }
    Ok(
        match crate::agent_run_successor::advance(client, claim, assistance).await? {
            SuccessorWork::Done(result) => WorkPhase::Done(result),
            SuccessorWork::Hold(kind) => WorkPhase::Hold(kind),
            SuccessorWork::Retrieve(request) => {
                WorkPhase::Dispatch(Box::new(DestinationRequest::Retrieve(request)))
            }
        },
    )
}

/// A cancelled Run keeps only evidence work: the fenced retrieval of its unknown create.
pub(crate) async fn settle_cancelled(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
) -> Result<WorkPhase, CompleteAgentRunError> {
    let payload: serde_json::Value =
        serde_json::from_str(run.attempt_payload.as_deref().unwrap_or("null"))
            .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    if payload.get("original_result_retrieval").is_some() {
        let work = advance_original_result_retrieval(
            client,
            claim,
            &payload,
            RetrievalAdvance {
                attempt_id: run.attempt_id.as_deref().unwrap_or_default(),
                conversation_id: &run.conversation_id,
                run_status: &run.status,
            },
            RetrievalFence::Fenced,
            RetrievalRunWrite::Write,
        )
        .await?;
        if let RetrievalWork::Retrieve(request) = work {
            return Ok(WorkPhase::Dispatch(Box::new(DestinationRequest::Retrieve(
                request,
            ))));
        }
    }
    if let Some(abort) = crate::agent_run_abort::pending(client, claim, run).await? {
        return Ok(WorkPhase::Dispatch(Box::new(DestinationRequest::Abort(
            abort,
        ))));
    }
    client
        .execute(
            "UPDATE storyos.agent_runs
                SET wakeup_pending = false, lease_expires_at = NULL
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid
                AND fence_token = $4",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &claim.fence_token,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    Ok(WorkPhase::Done(CompleteAgentRun::AlreadySettled))
}

/// Records the reference of an unknown create as the retrieval subject of the first decision.
pub(crate) async fn record_unknown_create(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    dispatch: &DispatchClaim,
    reference: Option<&ResponseReference>,
) -> Result<WorkPhase, CompleteAgentRunError> {
    if run.decision_position != "0" {
        update_run(
            client,
            claim,
            "waiting",
            Some(&serde_json::json!({"kind": "outcome_unknown", "reason": "unknown_result"})),
            /*clear_lease*/ true,
        )
        .await?;
        return Ok(WorkPhase::Done(CompleteAgentRun::Settled));
    }
    write_subject(client, claim, run, dispatch, reference).await?;
    Ok(WorkPhase::Hold("recovery"))
}

/// A cancelled Run keeps a retrievable reference of its unknown create as evidence, for a
/// later fenced retrieval. Nothing else of the exchange is recorded.
pub(crate) async fn record_cancelled_create(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    dispatch: &DispatchClaim,
    observation: Observation,
) -> Result<WorkPhase, CompleteAgentRunError> {
    let Observation::OutcomeUnknown {
        response_reference: Some(reference),
    } = observation
    else {
        return Ok(WorkPhase::Done(CompleteAgentRun::AlreadySettled));
    };
    if run.decision_position != "0"
        || !matches!(reference.retrieval, ReferenceRetrieval::Supported { .. })
    {
        return Ok(WorkPhase::Done(CompleteAgentRun::AlreadySettled));
    }
    write_subject(client, claim, run, dispatch, Some(&reference)).await?;
    client
        .execute(
            "UPDATE storyos.agent_runs SET wakeup_pending = true
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid AND fence_token = $4",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &claim.fence_token,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    Ok(WorkPhase::Hold("recovery"))
}

async fn write_subject(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    dispatch: &DispatchClaim,
    reference: Option<&ResponseReference>,
) -> Result<(), CompleteAgentRunError> {
    let mut payload: serde_json::Value =
        serde_json::from_str(run.attempt_payload.as_deref().unwrap_or("null"))
            .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    if payload.get("original_result_retrieval").is_some() {
        return Ok(());
    }
    let identity = read_assistance_record(client, &claim.project_scope)
        .await
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?
        .map(|record| record.processing_destination_identity)
        .unwrap_or_default();
    payload["original_result_retrieval"] =
        crate::agent_run_retrieval::subject(reference, claim, &run.conversation_id, &identity);
    let lookup = match reference.map(|reference| reference.retrieval) {
        None => SuccessorLookup::Unavailable {
            reason: LookupUnavailable::MissingReference,
        },
        Some(ReferenceRetrieval::Unsupported) => SuccessorLookup::Unavailable {
            reason: LookupUnavailable::UnsupportedRetrieval,
        },
        Some(ReferenceRetrieval::Supported { .. }) => SuccessorLookup::StillUnknown,
    };
    let invocation_id: String = client
        .query_one(
            "SELECT model_invocation_id::text FROM storyos.model_attempts
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND model_attempt_id = $3::text::uuid",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &dispatch.model_attempt_id,
            ],
        )
        .await
        .map_err(complete_database_error)?
        .get(0);
    if let Some(marker) = crate::agent_run_successor::prepare_subject(
        &run.author_message,
        &crate::agent_run_successor::SuccessorOrigin {
            predecessor_attempt_id: &dispatch.model_attempt_id,
            model_invocation_id: &invocation_id,
        },
        lookup,
    ) {
        payload["unknown_create_successor"] = marker;
    }
    payload["reservation"] = serde_json::json!({"kind": "worst_case", "released": false});
    let updated = client
        .execute(
            "UPDATE storyos.model_attempts SET payload = $4::text::jsonb
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND model_attempt_id = $3::text::uuid AND decision_id IS NULL",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &dispatch.model_attempt_id,
                &payload.to_string(),
            ],
        )
        .await
        .map_err(complete_database_error)?;
    if updated != 1 {
        return Err(CompleteAgentRunError::Unavailable(Box::new(
            std::io::Error::other("The unknown create could not record its reference"),
        )));
    }
    Ok(())
}

/// The cancel command sends no destination request. It leaves the abort and the fenced
/// retrieval of an in-flight decision Attempt to a Worker claim of the cancelled Run.
pub(crate) async fn mark_cancelled_evidence(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<(), CompleteAgentRunError> {
    client
        .execute(
            "UPDATE storyos.agent_runs AS run SET wakeup_pending = true
              WHERE run.owner_user_id = $1::text::uuid AND run.project_id = $2::text::uuid
                AND run.run_id = $3::text::uuid AND run.status = 'cancelled'
                AND EXISTS (
                  SELECT 1 FROM storyos.model_attempts AS attempt
                   WHERE (attempt.owner_user_id, attempt.project_id, attempt.run_id) =
                         (run.owner_user_id, run.project_id, run.run_id)
                     AND attempt.attempt_role = 'decision'
                     AND attempt.decision_position = run.active_decision_position
                     AND attempt.decision_id IS NULL AND attempt.dispatch_state = 'uncertain')",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    Ok(())
}
