//! The best-effort provider abort of one cancelled decision Model Attempt.

use storyos_application::{
    AbortRequest, ClaimedAgentRun, CommittedCancellation, CompleteAgentRunError, DispatchClaim,
    Observation, RequestAttempt, WirePayloadProjection,
};
use uuid::Uuid;

use crate::agent_run_work::{RunPhaseRow, WorkPhase, complete_database_error};

/// The next committed cancellation of a cancelled Run: a decision or successor Attempt that was
/// dispatched, has an unknown outcome, and has no recorded Abort yet. Decision Attempts come first.
pub(crate) async fn pending(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
) -> Result<Option<CommittedCancellation>, CompleteAgentRunError> {
    let row = client
        .query_opt(
            "SELECT attempt.model_attempt_id::text, abort.model_attempt_id::text,
                    COALESCE(attempt.payload->>'response_reference',
                             attempt.payload->'original_result_retrieval'->>'reference_id')
               FROM storyos.model_attempts AS attempt
               LEFT JOIN storyos.model_attempts AS abort
                 ON (abort.owner_user_id, abort.project_id, abort.run_id) =
                    (attempt.owner_user_id, attempt.project_id, attempt.run_id)
                AND abort.decision_position = attempt.decision_position
                AND abort.attempt_role = CASE attempt.attempt_role
                      WHEN 'decision' THEN 'abort' ELSE 'successor_abort' END
              WHERE attempt.owner_user_id = $1::text::uuid
                AND attempt.project_id = $2::text::uuid
                AND attempt.run_id = $3::text::uuid
                AND attempt.decision_id IS NULL AND attempt.dispatch_state = 'uncertain'
                AND ((attempt.attempt_role = 'decision'
                      AND attempt.decision_position = $4::text::numeric)
                     OR attempt.attempt_role = 'successor')
                AND (abort.model_attempt_id IS NULL OR abort.payload->>'result' IS NULL)
              ORDER BY attempt.attempt_role = 'successor'
              LIMIT 1",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &run.decision_position,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    Ok(row.map(|row| CommittedCancellation {
        model_attempt_id: row.get(0),
        response_reference: row.get(2),
        abort_attempt: match row.get::<_, Option<String>>(1) {
            Some(model_attempt_id) => RequestAttempt::Claimed(DispatchClaim { model_attempt_id }),
            None => RequestAttempt::New,
        },
    }))
}

/// Commits the Destination Attempt and Outbound Disclosure Event of one Abort request.
pub(crate) async fn commit(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    request: &AbortRequest,
    projection: &WirePayloadProjection,
) -> Result<Option<DispatchClaim>, CompleteAgentRunError> {
    if run.status != "cancelled" {
        return Ok(None);
    }
    let attempt_id = Uuid::now_v7().to_string();
    let payload = serde_json::json!({
        "operation": "abort",
        "original_model_attempt_id": request.ticket.model_attempt_id(),
        "response_reference_id": request.response_reference,
        "wire": { "digest": projection.digest, "mapping_revision": projection.mapping_revision },
        "result": null,
        "items": [],
        "decision": null,
        "usage": { "kind": "unknown" },
        "evidence": [{
            "kind": "provider_report",
            "attempt_id": attempt_id,
            "availability": "current",
            "report": "host_fake_abort"
        }]
    });
    let inserted = client
        .execute(
            "INSERT INTO storyos.model_attempts
               (owner_user_id, project_id, run_id, model_attempt_id, destination_attempt_id,
                outbound_disclosure_event_id, destination_context_manifest_id,
                outbound_disclosure_manifest_id, wire_payload_projection_id,
                model_invocation_id, conversation_id, dispatch_state, payload, attempt_role,
                decision_position)
             SELECT owner_user_id, project_id, run_id, $4::text::uuid, $5::text::uuid,
                    $6::text::uuid, destination_context_manifest_id,
                    outbound_disclosure_manifest_id, $7::text::uuid, model_invocation_id,
                    conversation_id, 'uncertain', $8::text::jsonb,
                    CASE attempt_role WHEN 'decision' THEN 'abort' ELSE 'successor_abort' END,
                    decision_position
               FROM storyos.model_attempts
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND run_id = $9::text::uuid
                AND model_attempt_id = $3::text::uuid
                AND attempt_role IN ('decision', 'successor')
                AND NOT EXISTS (
                  SELECT 1 FROM storyos.model_attempts AS abort
                   WHERE abort.owner_user_id = $1::text::uuid
                     AND abort.project_id = $2::text::uuid
                     AND abort.run_id = model_attempts.run_id
                     AND abort.attempt_role IN ('abort', 'successor_abort')
                     AND abort.payload->>'original_model_attempt_id' = $3)",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &request.ticket.model_attempt_id(),
                &attempt_id,
                &Uuid::now_v7().to_string(),
                &Uuid::now_v7().to_string(),
                &Uuid::now_v7().to_string(),
                &payload.to_string(),
                &claim.run_id,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    Ok((inserted == 1).then_some(DispatchClaim {
        model_attempt_id: attempt_id,
    }))
}

/// Records the abort result. The aborted Attempt stays OutcomeUnknown; the cancellation only
/// notes whether the destination confirmed the stop.
pub(crate) async fn record(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    dispatch: &DispatchClaim,
    request: &AbortRequest,
    observation: Observation,
) -> Result<WorkPhase, CompleteAgentRunError> {
    let confirmed = matches!(observation, Observation::Terminal(_));
    let parameters: [&(dyn tokio_postgres::types::ToSql + Sync); 6] = [
        &claim.project_scope.owner_user_id.as_ref(),
        &claim.project_scope.project_id.as_ref(),
        &dispatch.model_attempt_id,
        &request.ticket.model_attempt_id(),
        &if confirmed { "settled" } else { "uncertain" },
        &if confirmed { "acknowledged" } else { "unknown" },
    ];
    client
        .execute(
            "WITH aborted AS (
               UPDATE storyos.model_attempts
                  SET dispatch_state = $5,
                      payload = payload || jsonb_build_object('result', $6::text)
                WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                  AND model_attempt_id = $3::text::uuid
                  AND attempt_role IN ('abort', 'successor_abort')
            RETURNING model_attempt_id)
             UPDATE storyos.model_attempts
                SET payload = payload || jsonb_build_object(
                      'model_attempt_cancellation', jsonb_build_object(
                        'abort_attempt_id', $3::text, 'provider_confirmed', $6::text = 'acknowledged'))
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND model_attempt_id = $4::text::uuid
                AND EXISTS (SELECT 1 FROM aborted)",
            &parameters,
        )
        .await
        .map_err(complete_database_error)?;
    Ok(WorkPhase::Hold("recovery"))
}
