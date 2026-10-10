//! The bounded unknown-create successor as a Create request through the Model Gateway.

use storyos_application::{
    ClaimedAgentRun, CompleteAgentRun, CompleteAgentRunError, CreateRequest, DispatchClaim,
    Observation, RequestAttempt, RetrievePurpose, RetrieveRequest, WirePayloadProjection,
};
use storyos_core::{AgentDecisionOutcome, validate_agent_decision};
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
                AND run_id = $3::text::uuid AND attempt_role = 'successor'",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
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

/// Commits the successor Destination Attempt and Outbound Disclosure Event. The predecessor
/// already holds the consumed one-successor allowance.
pub(crate) async fn commit(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
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
    let manifest_id = Uuid::now_v7().to_string();
    let requirement_id = Uuid::now_v7().to_string();
    let destination_manifest_id = Uuid::now_v7().to_string();
    let outbound_manifest_id = Uuid::now_v7().to_string();
    copy_requirement(client, claim, &requirement_id, &Uuid::now_v7().to_string()).await?;
    copy_manifest(
        client,
        claim,
        &manifest_id,
        &requirement_id,
        &destination_manifest_id,
        &outbound_manifest_id,
    )
    .await?;
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
    if let Some(wire) = crate::agent_run_continuation::parse_wire(&row.payload) {
        successor_payload["continuation"] = crate::agent_run_continuation::encode_wire(&wire);
    }
    client
        .execute(
            "INSERT INTO storyos.model_attempts
               (owner_user_id, project_id, run_id, model_attempt_id, destination_attempt_id,
                outbound_disclosure_event_id, destination_context_manifest_id,
                outbound_disclosure_manifest_id, wire_payload_projection_id,
                model_invocation_id, conversation_id, dispatch_state, payload, attempt_role)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::uuid, $7::text::uuid, $8::text::uuid,
                     $9::text::uuid, $10::text::uuid, $11::text::uuid, 'uncertain',
                     $12::text::jsonb, 'successor')",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &attempt_id,
                &Uuid::now_v7().to_string(),
                &Uuid::now_v7().to_string(),
                &destination_manifest_id,
                &outbound_manifest_id,
                &Uuid::now_v7().to_string(),
                &row.invocation_id,
                &row.conversation_id,
                &successor_payload.to_string(),
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(Some(DispatchClaim {
        model_attempt_id: attempt_id,
    }))
}

/// Records the successor observation. Core validation decides its Agent Decision.
pub(crate) async fn record(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    dispatch: &DispatchClaim,
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
            "SELECT payload::text FROM storyos.model_attempts
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
    let mut payload: serde_json::Value =
        serde_json::from_str(&loaded.get::<_, String>(0)).map_err(unavailable)?;
    let (dispatch_state, decision_id, binding_id) = match observation {
        Observation::Terminal(response) => {
            payload["items"] =
                crate::agent_run_observation::merge_items(&payload["items"], &response.items);
            payload["usage"] = crate::agent_run_observation::encode_usage(response.usage);
            if let Some(reference) = &response.response_reference {
                payload["response_reference"] = serde_json::json!(reference);
            }
            match validate_agent_decision(response.output.as_ref(), &[]) {
                AgentDecisionOutcome::Decision {
                    kind,
                    selected: true,
                    advances_continuation,
                } => {
                    let decision_id = Uuid::now_v7().to_string();
                    let binding_id = advances_continuation.then(|| Uuid::now_v7().to_string());
                    payload["decision"] = crate::agent_run_observation::encode_decision(
                        &kind,
                        &decision_id,
                        /*selected*/ true,
                        advances_continuation,
                        /*opened_proposal*/ None,
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
                    ("settled", Some(decision_id), binding_id)
                }
                AgentDecisionOutcome::Decision { .. } | AgentDecisionOutcome::NoDecision => {
                    ("settled", None, None)
                }
            }
        }
        Observation::OutcomeUnknown { response_reference } => {
            if let Some(reference) = response_reference {
                payload["response_reference"] = serde_json::json!(reference.reference_id);
            }
            ("uncertain", None, None)
        }
        Observation::NotSubmitted => {
            payload["submission"] = serde_json::json!("not_submitted");
            ("settled", None, None)
        }
        Observation::Rejected { reason } => {
            payload["rejection"] = serde_json::json!(reason);
            ("settled", None, None)
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
                AND run_id = $3::text::uuid AND manifest_role = 'successor'",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map_err(unavailable)?
        .get(0);
    marker["successor_model_attempt_id"] = serde_json::json!(dispatch.model_attempt_id);
    marker["successor_manifest_id"] = serde_json::json!(manifest_id);
    marker["successor_decision_id"] = serde_json::json!(decision_id);
    seal(
        &mut marker,
        "dispatched",
        /*pause_reason*/ None,
        /*allowance_consumed*/ true,
        /*predecessor_fenced*/ true,
    );
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
    row: &DecisionRow,
    marker: &serde_json::Value,
) -> Result<SuccessorWork, CompleteAgentRunError> {
    if let Some(response_reference) = late_check_reference(&row.payload, marker) {
        let existing = client
            .query_opt(
                "SELECT model_attempt_id::text FROM storyos.model_attempts
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND run_id = $3::text::uuid
                    AND attempt_role = 'late_retrieval' AND decision_position = 0",
                &[
                    &claim.project_scope.owner_user_id.as_ref(),
                    &claim.project_scope.project_id.as_ref(),
                    &claim.run_id,
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
        }));
    }
    update_run(
        client,
        claim,
        "completed",
        /*settlement*/ None,
        /*clear_lease*/ true,
    )
    .await?;
    Ok(SuccessorWork::Done(CompleteAgentRun::Settled))
}

/// The predecessor reference to read for a late result, once, after the original retrieval.
fn late_check_reference(payload: &serde_json::Value, marker: &serde_json::Value) -> Option<String> {
    let subject = payload.get("original_result_retrieval")?;
    (!flag(marker, "late_result_checked") && text(subject, "retrieval_attempt_id").is_some())
        .then(|| text(subject, "reference_id").map(str::to_owned))
        .flatten()
}

async fn copy_requirement(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    requirement_id: &str,
    snapshot_id: &str,
) -> Result<(), CompleteAgentRunError> {
    let inserted = client
        .execute(
            "INSERT INTO storyos.operation_requirements
               (owner_user_id, project_id, operation_requirement_id, run_id,
                input_snapshot_id, receipt_id, payload, requirement_role)
             SELECT owner_user_id, project_id, $4::text::uuid, run_id,
                    $5::text::uuid, receipt_id, payload, 'successor'
               FROM storyos.operation_requirements
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid
                AND requirement_role = 'primary' AND decision_position = 0",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &requirement_id,
                &snapshot_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    if inserted != 1 {
        return Err(unavailable(std::io::Error::other(
            "The primary operation requirement is missing",
        )));
    }
    Ok(())
}

async fn copy_manifest(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    manifest_id: &str,
    requirement_id: &str,
    destination_manifest_id: &str,
    outbound_manifest_id: &str,
) -> Result<(), CompleteAgentRunError> {
    let manifested = client
        .execute(
            "INSERT INTO storyos.context_assembly_manifests
               (owner_user_id, project_id, context_assembly_manifest_id,
                operation_requirement_id, run_id, sufficiency,
                destination_context_manifest_id, outbound_disclosure_manifest_id,
                payload, receipt_id, manifest_role)
             SELECT owner_user_id, project_id, $4::text::uuid, $5::text::uuid, run_id,
                    sufficiency, $6::text::uuid, $7::text::uuid, payload, receipt_id, 'successor'
               FROM storyos.context_assembly_manifests
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid
                AND manifest_role = 'decision' AND decision_position = 0",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &manifest_id,
                &requirement_id,
                &destination_manifest_id,
                &outbound_manifest_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    if manifested != 1 {
        return Err(unavailable(std::io::Error::other(
            "The decision Context Assembly Manifest is missing",
        )));
    }
    Ok(())
}
