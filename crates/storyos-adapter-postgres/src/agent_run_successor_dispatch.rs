//! The bounded unknown-create successor as a Create request through the Model Gateway.

use storyos_application::{
    ClaimedAgentRun, CompleteAgentRun, CompleteAgentRunError, CreateRequest, DeclaredTarget,
    DispatchClaim, ModelUsage, Observation, RequestAttempt, RetrievePurpose, RetrieveRequest,
    WirePayloadProjection,
};
use storyos_core::{
    AgentDecisionKind, AgentDecisionOutcome, FinishRuleOutcome, ResponseDisposition, ResponseFacts,
    SuccessorPauseReason, response_disposition, validate_agent_decision,
};
use uuid::Uuid;

use crate::agent_run_create_dispatch::PriorContext;
use crate::agent_run_successor::{
    DecisionRow, SuccessorWork, flag, load_decision, seal, text, unavailable,
    write_decision_payload,
};
use crate::agent_run_work::{RunPhaseRow, WorkPhase, update_run};

/// The one successor Create of the fenced predecessor. It repeats the same request, or
/// re-observes the successor Attempt that a lost claim committed.
pub(crate) async fn successor_request(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
) -> Result<CreateRequest, CompleteAgentRunError> {
    let record: serde_json::Value =
        serde_json::from_str(&run.assembly_payload).map_err(unavailable)?;
    let attempt = match client
        .query_opt(
            "SELECT model_attempt_id::text FROM storyos.model_attempts
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid AND attempt_role = 'successor'
                AND decision_position = $4::text::numeric",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &run.decision_position,
            ],
        )
        .await
        .map_err(unavailable)?
    {
        Some(row) => RequestAttempt::Claimed(DispatchClaim {
            model_attempt_id: row.get(0),
        }),
        None => RequestAttempt::New,
    };
    let mut request = crate::agent_run_create_dispatch::create_request(
        client,
        claim,
        run,
        &record,
        attempt,
        PriorContext::Continue,
    )
    .await?;
    request.successor_of = run.attempt_id.clone();
    Ok(request)
}

/// Whether the successor sends the same request as its predecessor, with the Tool Exposure that
/// the claim transaction computes now. A current Exposure that is not the Exposure of the
/// prepared `request`, or a different wire digest, pauses the recovery with `request_changed`
/// and sends nothing: the successor can only repeat the request of its predecessor, and that
/// request no longer fits the current Exposure. Returns `false` after that pause.
pub(crate) async fn same_request(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    request: &CreateRequest,
    projection: &WirePayloadProjection,
) -> Result<bool, CompleteAgentRunError> {
    let exposure_current = crate::agent_run_tool_exposure::tool_exposure(client, claim, run)
        .await?
        .digest()
        == request.tool_exposure.digest();
    let mut row = load_decision(client, claim).await?;
    if exposure_current
        && row
            .payload
            .pointer("/wire/digest")
            .and_then(serde_json::Value::as_str)
            == Some(projection.digest.as_str())
    {
        return Ok(true);
    }
    let Some(marker) = row.payload.get("unknown_create_successor").cloned() else {
        return Ok(true);
    };
    crate::agent_run_successor::pause(
        client,
        claim,
        &mut row,
        marker,
        SuccessorPauseReason::RequestChanged,
    )
    .await?;
    Ok(false)
}

/// Commits the successor Destination Attempt and Outbound Disclosure Event at the Decision
/// position of its predecessor. The predecessor already holds the consumed one-successor
/// allowance.
pub(crate) async fn commit(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    projection: &WirePayloadProjection,
) -> Result<Option<DispatchClaim>, CompleteAgentRunError> {
    let row = load_decision(client, claim).await?;
    let Some(marker) = row.payload.get("unknown_create_successor") else {
        return Ok(None);
    };
    if text(marker, "successor_model_attempt_id").is_some()
        || marker
            .get("allowance_consumed")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        || marker
            .get("dispatch_prohibited")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
    {
        return Ok(None);
    }
    let attempt_id = Uuid::now_v7().to_string();
    let manifests = crate::agent_run_retrieval_dispatch::copy_active_context(
        client,
        claim,
        &run.decision_position,
        "successor",
    )
    .await?;
    let manifest_id = manifests.assembly.unwrap_or_default();
    let mut successor_payload = serde_json::json!({
        "operation": "unknown_create_successor",
        "predecessor_model_attempt_id": row.attempt_id,
        "model_invocation_id": row.invocation_id,
        "wire": row.payload.get("wire").cloned().unwrap_or(serde_json::Value::Null),
        "projection": { "digest": projection.digest },
        "items": [],
        "decision": null,
        "usage": { "kind": "unknown" },
        "reservation": { "kind": "worst_case", "released": false },
        "settles_predecessor": false,
        "evidence": crate::agent_run_attempt::evidence_values(
            &attempt_id,
            row.payload
                .pointer("/wire/author_message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            &manifest_id,
            /*known_prior_binding_id*/ None,
        )
    });
    // The admitted successor pins the binding, and thus the grant, of its predecessor.
    if let Some(grant) = row.payload.get("disclosure_grant") {
        successor_payload["disclosure_grant"] = grant.clone();
    }
    if let Some(wire) = crate::agent_run_continuation::parse_wire(&row.payload) {
        successor_payload["continuation"] = crate::agent_run_continuation::encode_wire(&wire);
    }
    client
        .execute(
            "INSERT INTO storyos.model_attempts
               (owner_user_id, project_id, run_id, model_attempt_id, destination_attempt_id,
                outbound_disclosure_event_id, destination_context_manifest_id,
                outbound_disclosure_manifest_id, wire_payload_projection_id,
                model_invocation_id, conversation_id, dispatch_state, payload, attempt_role,
                decision_position)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::uuid, $7::text::uuid, $8::text::uuid,
                     $9::text::uuid, $10::text::uuid, $11::text::uuid, 'uncertain',
                     $12::text::jsonb, 'successor', $13::text::numeric)",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &attempt_id,
                &Uuid::now_v7().to_string(),
                &Uuid::now_v7().to_string(),
                &manifests.destination,
                &manifests.outbound,
                &Uuid::now_v7().to_string(),
                &row.invocation_id,
                &row.conversation_id,
                &successor_payload.to_string(),
                &run.decision_position,
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(Some(DispatchClaim {
        model_attempt_id: attempt_id,
    }))
}

/// Records the successor observation. Core validation decides its Agent Decision with the
/// declared targets of the repeated request, and a selected prose change opens its Proposal. A
/// repeated observation of a settled successor Attempt changes nothing: its Decision and its
/// ToolCalls stay.
pub(crate) async fn record(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    dispatch: &DispatchClaim,
    request: &CreateRequest,
    observation: Observation,
) -> Result<WorkPhase, CompleteAgentRunError> {
    let mut row = load_decision(client, claim).await?;
    let mut marker = row
        .payload
        .get("unknown_create_successor")
        .cloned()
        .ok_or_else(|| unavailable(std::io::Error::other("The successor marker is missing")))?;
    let loaded = client
        .query_one(
            "SELECT payload::text, dispatch_state FROM storyos.model_attempts
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND model_attempt_id = $3::text::uuid AND attempt_role = 'successor'",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &dispatch.model_attempt_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    if loaded.get::<_, &str>(1) == "settled" {
        return Ok(WorkPhase::Hold(
            if late_check_reference(&row.payload, &marker).is_some() {
                "successor_late"
            } else {
                "recovery"
            },
        ));
    }
    let mut payload: serde_json::Value =
        serde_json::from_str(&loaded.get::<_, String>(0)).map_err(unavailable)?;
    let mut opens_next_run_step = false;
    let (dispatch_state, decision_id, binding_id, function_requested) = match observation {
        Observation::Terminal(mut response) => {
            payload["items"] =
                crate::agent_run_observation::merge_items(payload["items"].take(), &response.items);
            payload["usage"] = crate::agent_run_observation::encode_usage(response.usage);
            // Only known usage settles the worst-case reservation of the successor (ADR 0033).
            payload["reservation"]["released"] =
                serde_json::json!(!matches!(response.usage, ModelUsage::Unknown));
            if let Some(reference) = &response.response_reference {
                payload["response_reference"] = serde_json::json!(reference);
            }
            let declared: Vec<_> = request
                .declared_targets
                .iter()
                .map(DeclaredTarget::location)
                .collect();
            // The successor of a probe makes no Agent Decision (ADR 0049). The successor sends
            // the request of its predecessor again, so its batch validates against the Step
            // Snapshot of the predecessor (ADR 0084).
            let probe = crate::agent_run_tool_probe::is_probe(&row.payload);
            let outcome = if probe {
                AgentDecisionOutcome::NoDecision
            } else {
                let batch =
                    crate::agent_run_tool_call::validate_batch(client, &row.payload, &response)
                        .await?;
                let batch = crate::agent_run_propose_edit::screen(
                    client, claim, &declared, &response, batch,
                )
                .await?;
                if let Some(evidence) = crate::agent_run_tool_call::batch_evidence(&batch) {
                    payload["tool_call_batch"] = evidence;
                }
                validate_agent_decision(batch)
            };
            let (outcome, rule) = match crate::agent_run_finish_rule::recovered_outcome(
                client,
                claim,
                run,
                &mut response,
                outcome,
            )
            .await?
            {
                FinishRuleOutcome::Settle(outcome) => (outcome, None),
                rule @ (FinishRuleOutcome::NextRunStep | FinishRuleOutcome::NoResult) => {
                    (AgentDecisionOutcome::NoDecision, Some(rule))
                }
            };
            opens_next_run_step = rule == Some(FinishRuleOutcome::NextRunStep);
            match outcome {
                AgentDecisionOutcome::Decision {
                    kind,
                    selected: true,
                    advances_continuation,
                } => {
                    let decision_id = Uuid::now_v7().to_string();
                    let binding_id = advances_continuation.then(|| Uuid::now_v7().to_string());
                    let tool_calls_pending = match &kind {
                        AgentDecisionKind::ToolRequest { calls } => {
                            crate::agent_run_tool_call::insert_tool_calls(
                                client,
                                claim,
                                &crate::agent_run_tool_call::ToolCallOwner {
                                    run_id: &claim.run_id,
                                    decision_position: &run.decision_position,
                                    model_attempt_id: &dispatch.model_attempt_id,
                                    decision_id: &decision_id,
                                },
                                calls,
                            )
                            .await?;
                            true
                        }
                        AgentDecisionKind::Advisory { .. } => false,
                    };
                    payload["decision"] = crate::agent_run_observation::encode_decision(
                        &kind,
                        &decision_id,
                        /*selected*/ true,
                        advances_continuation,
                    );
                    if let (Some(binding_id), Some(wire)) = (
                        &binding_id,
                        crate::agent_run_continuation::parse_wire(&row.payload),
                    ) {
                        payload["produced_binding"] =
                            crate::agent_run_continuation::encode_produced_binding(
                                binding_id,
                                &dispatch.model_attempt_id,
                                &wire.admission,
                            );
                    }
                    ("settled", Some(decision_id), binding_id, tool_calls_pending)
                }
                // A successor that asks for a function keeps its items as evidence, and the Run
                // settles once the late check is done (ADR 0084, ADR 0091).
                AgentDecisionOutcome::NoDecision
                    if response_disposition(&ResponseFacts {
                        probe,
                        outcome: &AgentDecisionOutcome::NoDecision,
                        items: &response.items,
                    }) == ResponseDisposition::FunctionRequested =>
                {
                    ("settled", None, None, true)
                }
                // A closing response with no text gets the continuation binding, so the next
                // Run continues from it. A text before a counted `finish` call gets its binding
                // when the next RunStep opens.
                AgentDecisionOutcome::NoDecision if rule == Some(FinishRuleOutcome::NoResult) => {
                    let binding_id = Uuid::now_v7().to_string();
                    if let Some(wire) = crate::agent_run_continuation::parse_wire(&row.payload) {
                        payload["produced_binding"] =
                            crate::agent_run_continuation::encode_produced_binding(
                                &binding_id,
                                &dispatch.model_attempt_id,
                                &wire.admission,
                            );
                    }
                    ("settled", None, Some(binding_id), false)
                }
                AgentDecisionOutcome::Decision { .. } | AgentDecisionOutcome::NoDecision => {
                    ("settled", None, None, false)
                }
            }
        }
        Observation::OutcomeUnknown { response_reference } => {
            if let Some(reference) = response_reference {
                payload["response_reference"] = serde_json::json!(reference.reference_id);
            }
            ("uncertain", None, None, false)
        }
        Observation::NotSubmitted => {
            payload["submission"] = serde_json::json!("not_submitted");
            ("settled", None, None, false)
        }
        Observation::Rejected { reason } => {
            payload["rejection"] = serde_json::json!(reason);
            ("settled", None, None, false)
        }
    };
    client
        .execute(
            "UPDATE storyos.model_attempts
                SET dispatch_state = $4, decision_id = $5::text::uuid,
                    continuation_binding_id = $6::text::uuid, payload = $7::text::jsonb
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND model_attempt_id = $3::text::uuid AND attempt_role = 'successor'",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &dispatch.model_attempt_id,
                &dispatch_state,
                &decision_id,
                &binding_id,
                &payload.to_string(),
            ],
        )
        .await
        .map_err(unavailable)?;
    let manifest_id: String = client
        .query_one(
            "SELECT context_assembly_manifest_id::text FROM storyos.context_assembly_manifests
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid AND manifest_role = 'successor'
                AND decision_position = $4::text::numeric",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &run.decision_position,
            ],
        )
        .await
        .map_err(unavailable)?
        .get(0);
    marker["successor_model_attempt_id"] = serde_json::json!(dispatch.model_attempt_id);
    marker["successor_manifest_id"] = serde_json::json!(manifest_id);
    marker["successor_decision_id"] = serde_json::json!(decision_id);
    marker["successor_requests_function"] = serde_json::json!(function_requested);
    marker["successor_opens_next_run_step"] = serde_json::json!(opens_next_run_step);
    marker["successor_outcome_unknown"] = serde_json::json!(dispatch_state == "uncertain");
    seal(
        &mut marker,
        "dispatched",
        /*pause_reason*/ None,
        /*allowance_consumed*/ true,
        /*predecessor_fenced*/ true,
    );
    marker["supplies_tool_call"] = serde_json::json!(function_requested && decision_id.is_some());
    let late = late_check_reference(&row.payload, &marker).is_some();
    row.payload["unknown_create_successor"] = marker;
    row.payload["reservation"] = serde_json::json!({"kind": "worst_case", "released": false});
    row.payload["usage"] = serde_json::json!({"kind": "unknown"});
    write_decision_payload(client, claim, &row.attempt_id, &row.payload).await?;
    Ok(WorkPhase::Hold(if late {
        "successor_late"
    } else {
        "recovery"
    }))
}

/// After the recorded successor: read a late predecessor result once, or complete the Run.
pub(crate) async fn finish_dispatched(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    row: &DecisionRow,
    marker: &serde_json::Value,
) -> Result<SuccessorWork, CompleteAgentRunError> {
    if let Some(response_reference) = late_check_reference(&row.payload, marker) {
        let existing = client
            .query_opt(
                "SELECT model_attempt_id::text FROM storyos.model_attempts
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND run_id = $3::text::uuid
                    AND attempt_role = 'late_retrieval'
                    AND decision_position = $4::text::numeric",
                &[
                    &claim.project_scope.owner_user_id.as_ref(),
                    &claim.project_scope.project_id.as_ref(),
                    &claim.run_id,
                    &run.decision_position,
                ],
            )
            .await
            .map_err(unavailable)?;
        return Ok(SuccessorWork::Retrieve(RetrieveRequest {
            route: crate::model_registration::request_route(client, claim).await?,
            attempt: match existing {
                Some(found) => RequestAttempt::Claimed(DispatchClaim {
                    model_attempt_id: found.get(0),
                }),
                None => RequestAttempt::New,
            },
            purpose: RetrievePurpose::LateResult,
            original_model_attempt_id: row.attempt_id.clone(),
            response_reference,
            declared_targets: crate::agent_run_create_dispatch::declared_targets(
                client,
                claim,
                &run.chapter_id,
            )
            .await?,
        }));
    }
    let supplies_tool_call = flag(marker, "supplies_tool_call");
    let unresolved = supplies_tool_call
        && crate::agent_run_tool_call::pending_tool_call_exists(client, claim).await?;
    let step = storyos_core::function_request_step(
        flag(marker, "successor_requests_function"),
        supplies_tool_call,
        unresolved,
    );
    if step == storyos_core::FunctionRequestStep::ToolGateway {
        return Ok(SuccessorWork::ToolCalls);
    }
    // The ToolCalls of the successor Decision are resolved, or the successor response is a text
    // before a `finish` call that counted: the next RunStep continues the successor response
    // (ADR 0085, ADR 0088, ADR 0091).
    let opens_next_run_step = flag(marker, "successor_opens_next_run_step");
    if step == storyos_core::FunctionRequestStep::Continue || opens_next_run_step {
        let (attempt_id, payload) = successor_payload(client, claim, marker).await?;
        if !opens_next_run_step {
            text(marker, "successor_decision_id").ok_or_else(|| {
                unavailable(std::io::Error::other("The successor Decision is missing"))
            })?;
        }
        crate::agent_run_continuation::bind_produced_continuation(
            client,
            claim,
            attempt_id,
            &payload,
            &row.payload,
        )
        .await?;
        crate::agent_run_tool_result::open_next_step(client, claim, run).await?;
        return Ok(SuccessorWork::Hold("tool_result_continuation"));
    }
    // The allowance is spent and the successor outcome is unknown: the Run waits with
    // `outcome_unknown` and keeps the evidence of each Attempt (ADR 0033, ADR 0088 section 3).
    if flag(marker, "successor_outcome_unknown") {
        update_run(
            client,
            claim,
            "waiting",
            Some(&serde_json::json!({"kind": "outcome_unknown", "reason": "unknown_result"})),
            /*clear_lease*/ true,
        )
        .await?;
        return Ok(SuccessorWork::Done(CompleteAgentRun::Settled));
    }
    let refusal = if step == storyos_core::FunctionRequestStep::Refuse {
        // The recorded successor Attempt keeps the batch evidence of its refusal.
        let (_, payload) = successor_payload(client, claim, marker).await?;
        crate::agent_run_observation::function_request_refusal(&payload)
    } else {
        None
    };
    update_run(
        client,
        claim,
        if refusal.is_some() {
            "refused"
        } else {
            "completed"
        },
        refusal.as_ref(),
        /*clear_lease*/ true,
    )
    .await?;
    Ok(SuccessorWork::Done(CompleteAgentRun::Settled))
}

/// The identity and the payload of the recorded successor Attempt.
async fn successor_payload<'a>(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    marker: &'a serde_json::Value,
) -> Result<(&'a str, serde_json::Value), CompleteAgentRunError> {
    let attempt_id = text(marker, "successor_model_attempt_id")
        .ok_or_else(|| unavailable(std::io::Error::other("The successor Attempt is missing")))?;
    let row = client
        .query_one(
            "SELECT payload::text FROM storyos.model_attempts
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND model_attempt_id = $3::text::uuid AND attempt_role = 'successor'",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &attempt_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    let payload = serde_json::from_str(&row.get::<_, String>(0)).map_err(unavailable)?;
    Ok((attempt_id, payload))
}

/// The predecessor reference to read for a late result, once, after the original retrieval.
fn late_check_reference(payload: &serde_json::Value, marker: &serde_json::Value) -> Option<String> {
    let subject = payload.get("original_result_retrieval")?;
    (!flag(marker, "late_result_checked") && text(subject, "retrieval_attempt_id").is_some())
        .then(|| text(subject, "reference_id").map(str::to_owned))
        .flatten()
}

#[cfg(test)]
#[path = "agent_run_successor_route_tests.rs"]
mod tests;
