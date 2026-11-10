//! The Tool Gateway store (ADR 0039, ADR 0084). Each operation is one serializable transaction
//! that checks the Run Lease fence first. The dispatch claim checks the current authority again
//! and commits the Admission Decision, the Wire Payload Projection, the Destination Attempt, and
//! the Outbound Disclosure Event in OutcomeUnknown before I/O. A ToolCall that a crash left
//! `dispatched` settles `unknown` and is not sent again.

use serde_json::{Value, json};
use storyos_application::{
    ClaimedAgentRun, CompleteAgentRunError, CredentialReference, PreDispatchRefusal,
    ToolCallRequest, ToolDispatchClaim, ToolDispatchRecord, ToolDispatchStore,
    WirePayloadProjection,
};
use storyos_core::{ToolCallState, ToolObservation};
use uuid::Uuid;

use super::PostgresProjectReader;
use crate::agent_run_tool_authority::{
    Resolution, ToolCallRow, locked, next_unresolved, quarantine, read_authority, refuse, resolve,
};
use crate::agent_run_tool_context::{ToolContext, commit_tool_context, committed_tool_context};
use crate::agent_run_work::{
    WorkPhase, complete_challenge_error, complete_database_error, load_run_phase,
};

impl ToolDispatchStore for PostgresProjectReader {
    #[tracing::instrument(skip_all, level = "debug")]
    async fn next_tool_call(
        &self,
        claim: &ClaimedAgentRun,
    ) -> Result<Option<ToolCallRequest>, CompleteAgentRunError> {
        self.in_tool_transaction(claim, async |client| {
            if load_run_phase(client, claim).await?.settled() {
                return Ok(None);
            }
            while let Some(row) = next_unresolved(client, claim).await? {
                if row.state == ToolCallState::Dispatched {
                    settle_interrupted(client, claim, &row).await?;
                    continue;
                }
                let plan = row.plan()?;
                let authority = read_authority(client, claim, &row, &plan).await?;
                if let Err(refusal) = authority.admit(&row, &plan) {
                    refuse(client, claim, &row, refusal.state(), refusal.code()).await?;
                    continue;
                }
                // Gates one to six commit with this transaction, before the preparation and
                // before the dispatch claim.
                let endpoint = authority.endpoint.clone().unwrap_or_default();
                let manifest = commit_tool_context(
                    client,
                    claim,
                    ToolContext {
                        row: &row,
                        plan: &plan,
                        endpoint: &endpoint,
                        authority: &authority.evidence,
                    },
                )
                .await?;
                return Ok(Some(ToolCallRequest {
                    tool_call_id: row.tool_call_id,
                    registration: plan.registration,
                    tool: plan.tool,
                    server_arguments: plan.server_arguments,
                    endpoint,
                    credential_reference: authority.credential_reference.map(CredentialReference),
                    context_assembly_manifest_id: manifest,
                }));
            }
            Ok(None)
        })
        .await
    }

    #[tracing::instrument(skip_all, level = "debug")]
    async fn commit_tool_dispatch_claim(
        &self,
        claim: &ClaimedAgentRun,
        request: &ToolCallRequest,
        projection: &WirePayloadProjection,
    ) -> Result<Option<ToolDispatchClaim>, CompleteAgentRunError> {
        self.in_tool_transaction(claim, async |client| {
            let run = load_run_phase(client, claim).await?;
            let Some(row) =
                locked(client, claim, &request.tool_call_id, ToolCallState::Pending).await?
            else {
                return Ok(None);
            };
            if run.settled() {
                return Ok(None);
            }
            let plan = row.plan()?;
            let authority = read_authority(client, claim, &row, &plan).await?;
            if let Err(refusal) = authority.admit(&row, &plan) {
                refuse(client, claim, &row, refusal.state(), refusal.code()).await?;
                return Ok(None);
            }
            // The authority or the endpoint changed after the preparation: prepare again.
            if authority.endpoint.as_deref() != Some(request.endpoint.as_str())
                || authority.credential_reference.as_deref()
                    != request
                        .credential_reference
                        .as_ref()
                        .map(|reference| reference.0.as_str())
            {
                return Ok(None);
            }
            // The claim refers to the committed manifest of this preparation. A manifest of
            // other authority evidence means that the authority changed: prepare again.
            let Some(manifests) = committed_tool_context(
                client,
                claim,
                &row,
                &request.context_assembly_manifest_id,
                &authority.evidence,
            )
            .await?
            else {
                return Ok(None);
            };
            let destination_attempt_id = Uuid::now_v7().to_string();
            let attempt = json!({
                "context_assembly": manifests.record(),
                "admission_decision": {
                    "decision": "admitted",
                    "grant_id": authority.grant_id,
                    "grant_scope": storyos_core::GrantScope::ResearchToolV1.as_str(),
                    "authority": authority.evidence,
                    "effect_request": plan.effect.record(),
                },
                "wire_payload_projection": {
                    "execution_profile": projection.execution_profile,
                    "mapping_revision": projection.mapping_revision,
                    "digest": projection.digest,
                    "serialized_payload": projection.serialized_payload,
                },
                "destination_attempt": {
                    "endpoint": request.endpoint,
                    "settlement": Value::Null,
                },
                "outbound_disclosure_event": {
                    "event_id": Uuid::now_v7().to_string(),
                    "source_class": plan.effect.outbound_disclosure.source_class(),
                    "state": "outcome_unknown",
                },
            });
            client
                .execute(
                    "UPDATE storyos.tool_calls
                        SET state = 'dispatched', destination_attempt_id = $4::text::uuid,
                            attempt = $5::text::jsonb
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                        AND tool_call_id = $3::text::uuid AND state = 'pending'",
                    &[
                        &claim.project_scope.owner_user_id.as_ref(),
                        &claim.project_scope.project_id.as_ref(),
                        &row.tool_call_id,
                        &destination_attempt_id,
                        &attempt.to_string(),
                    ],
                )
                .await
                .map_err(complete_database_error)?;
            Ok(Some(ToolDispatchClaim {
                destination_attempt_id,
            }))
        })
        .await
    }

    #[tracing::instrument(skip_all, level = "debug")]
    async fn record_tool_call(
        &self,
        claim: &ClaimedAgentRun,
        record: ToolDispatchRecord<'_>,
    ) -> Result<(), CompleteAgentRunError> {
        self.in_tool_transaction(claim, async |client| {
            load_run_phase(client, claim).await?;
            match record {
                ToolDispatchRecord::Refusal { request, refusal } => {
                    let Some(row) =
                        locked(client, claim, &request.tool_call_id, ToolCallState::Pending)
                            .await?
                    else {
                        return Ok(());
                    };
                    let code = match refusal {
                        PreDispatchRefusal::CredentialUnavailable => {
                            "destination_credential_unavailable"
                        }
                        PreDispatchRefusal::UnsupportedRequest => "destination_request_unsupported",
                    };
                    refuse(client, claim, &row, ToolCallState::Refused, code).await
                }
                ToolDispatchRecord::Exchange {
                    claim: dispatch,
                    request,
                    observation,
                } => {
                    let Some(row) = locked(
                        client,
                        claim,
                        &request.tool_call_id,
                        ToolCallState::Dispatched,
                    )
                    .await?
                    .filter(|row| {
                        row.destination_attempt_id.as_deref()
                            == Some(dispatch.destination_attempt_id.as_str())
                    }) else {
                        return Ok(());
                    };
                    record_exchange(client, claim, &row, request, &observation).await
                }
            }
        })
        .await
    }
}

/// The work phase of a Run whose durable Decision requested ToolCalls: the Tool Gateway while a
/// ToolCall is unresolved, and then the completed Run.
pub(crate) async fn advance(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<WorkPhase, CompleteAgentRunError> {
    let unresolved = crate::agent_run_tool_call::pending_tool_call_exists(client, claim).await?;
    match storyos_core::function_request_step(
        /*requests_function*/ true, /*supplies_tool_call*/ true, unresolved,
    ) {
        storyos_core::FunctionRequestStep::ToolGateway => Ok(WorkPhase::ToolCalls),
        storyos_core::FunctionRequestStep::Refuse | storyos_core::FunctionRequestStep::Complete => {
            crate::agent_run_work::complete(client, claim).await
        }
    }
}

impl PostgresProjectReader {
    /// Runs `body` in one serializable transaction, and commits only when it succeeds.
    async fn in_tool_transaction<T>(
        &self,
        claim: &ClaimedAgentRun,
        body: impl AsyncFnOnce(&tokio_postgres::Client) -> Result<T, CompleteAgentRunError>,
    ) -> Result<T, CompleteAgentRunError> {
        let transaction = self
            .begin_serializable_project_command_transaction(&claim.project_scope)
            .await
            .map_err(complete_challenge_error)?;
        match body(&transaction.client).await {
            Ok(value) => {
                transaction
                    .commit()
                    .await
                    .map_err(complete_challenge_error)?;
                Ok(value)
            }
            Err(error) => {
                let _rollback = transaction.rollback().await;
                Err(error)
            }
        }
    }
}

async fn record_exchange(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    request: &ToolCallRequest,
    observation: &ToolObservation,
) -> Result<(), CompleteAgentRunError> {
    let settlement = storyos_core::settle_tool_call(request.tool.spec, observation);
    if let ToolObservation::Drift(differences) = observation {
        quarantine(client, claim, row, differences).await?;
    }
    let attempt = settled_attempt(
        row,
        settlement.attempt.as_str(),
        settlement.outcome.record(),
    );
    resolve(
        client,
        claim,
        row,
        Resolution {
            state: settlement.state,
            refusal: settlement.refusal,
            attempt: Some(&attempt),
            outcome: Some(settlement.outcome.record()),
            result: settlement.result.as_ref(),
        },
    )
    .await
}

/// A ToolCall that stayed `dispatched` after a crash or a lost lease: its outcome is not known,
/// and the Gateway does not send it again.
/// Settles each `dispatched` ToolCall of a Run that the author cancels, in the cancel
/// transaction: the Worker that holds the old fence cannot record its observation, so the
/// ToolCall settles `unknown` with its Tool Effect Outcome and its Run Event, with no exchange.
pub(crate) async fn settle_dispatched_on_cancel(
    client: &tokio_postgres::Client,
    project_scope: &storyos_application::ProjectScope,
    run_id: &str,
) -> Result<(), CompleteAgentRunError> {
    let run = ClaimedAgentRun {
        project_scope: project_scope.clone(),
        run_id: run_id.to_owned(),
        // The cancel transaction holds the Run lock; no fence check follows.
        fence_token: 0,
    };
    let dispatched = client
        .query(
            "SELECT tool_call_id::text FROM storyos.tool_calls
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid AND state = 'dispatched'
              ORDER BY decision_position, call_position",
            &[
                &project_scope.owner_user_id.as_ref(),
                &project_scope.project_id.as_ref(),
                &run_id,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    for found in dispatched {
        let tool_call_id: String = found.get(0);
        if let Some(row) = locked(client, &run, &tool_call_id, ToolCallState::Dispatched).await? {
            settle_interrupted(client, &run, &row).await?;
        }
    }
    Ok(())
}

async fn settle_interrupted(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
) -> Result<(), CompleteAgentRunError> {
    let settlement =
        storyos_core::settle_tool_call(row.plan()?.tool.spec, &ToolObservation::OutcomeUnknown);
    let attempt = settled_attempt(
        row,
        settlement.attempt.as_str(),
        settlement.outcome.record(),
    );
    resolve(
        client,
        claim,
        row,
        Resolution {
            state: settlement.state,
            refusal: None,
            attempt: Some(&attempt),
            outcome: Some(settlement.outcome.record()),
            result: None,
        },
    )
    .await
}

/// The claimed attempt record with its settlement. The Outbound Disclosure Event stays as the
/// claim recorded it (`outcome_unknown`); a separate confirmation records what the exchange proved.
fn settled_attempt(row: &ToolCallRow, settlement: &str, outcome: Value) -> Value {
    let mut attempt = row.attempt.clone().unwrap_or_else(|| json!({}));
    attempt["destination_attempt"]["settlement"] = json!(settlement);
    attempt["disclosure_confirmation"] = json!({
        "event_id": attempt["outbound_disclosure_event"]["event_id"].clone(),
        "state": outcome["outbound_disclosure"].clone(),
    });
    attempt
}

#[cfg(test)]
#[path = "agent_run_tool_dispatch_tests.rs"]
mod tests;
