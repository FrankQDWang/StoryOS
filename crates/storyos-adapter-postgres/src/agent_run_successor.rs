use storyos_application::{
    ClaimedAgentRun, CompleteAgentRun, CompleteAgentRunError, ProjectAssistanceRecord,
    ProjectScope, UnknownCreateSuccessorDisposition, UnknownCreateSuccessorInspect,
};
use storyos_core::{
    ADVISORY_TEXT, AssistanceAvailability, HOST_FAKE_MAPPING_REVISION, LookupUnavailable,
    SuccessorAllowance, SuccessorEffect, SuccessorLookup, UnknownCreateSuccessorDecision,
    UnknownCreateSuccessorFacts, decide_unknown_create_successor, scripted_successor_conditions,
    unknown_create_script,
};
use uuid::Uuid;

use crate::agent_run_work::update_run;

pub(crate) struct SuccessorOrigin<'a> {
    pub owner_user_id: &'a str,
    pub project_id: &'a str,
    pub conversation_id: &'a str,
    pub destination_identity: &'a str,
    pub predecessor_attempt_id: &'a str,
    pub model_invocation_id: &'a str,
}

pub(crate) struct PreparedSuccessor {
    pub retrieval: serde_json::Value,
    pub marker: serde_json::Value,
}

pub(crate) enum SuccessorWork {
    Done(CompleteAgentRun),
    Hold(&'static str),
}

pub(crate) struct SuccessorSelection {
    pub payload: serde_json::Value,
    pub continuation_binding_id: Option<String>,
}

pub(crate) fn prepare_subject(
    author_message: &str,
    origin: &SuccessorOrigin<'_>,
) -> Option<PreparedSuccessor> {
    let script = unknown_create_script(author_message);
    let conditions = scripted_successor_conditions(script)?;
    let reference_present = !matches!(
        conditions.lookup,
        SuccessorLookup::Unavailable {
            reason: LookupUnavailable::MissingReference,
        }
    );
    let reference_id = reference_present.then(|| Uuid::now_v7().to_string());
    let capability = if matches!(
        conditions.lookup,
        SuccessorLookup::Unavailable {
            reason: LookupUnavailable::UnsupportedRetrieval,
        }
    ) {
        "unsupported"
    } else {
        "supported"
    };
    let result = if reference_present {
        "unknown"
    } else {
        "absent"
    };
    let retrieval = serde_json::json!({
        "reconciliation_id": Uuid::now_v7().to_string(),
        "reconciled": false,
        "owner_user_id": origin.owner_user_id,
        "project_id": origin.project_id,
        "conversation_id": origin.conversation_id,
        "destination_identity": origin.destination_identity,
        "mapping_revision": HOST_FAKE_MAPPING_REVISION,
        "capability": capability,
        "bounds": "declared_read_only",
        "reference_id": reference_id,
        "result": result
    });
    let lookup_reason = match conditions.lookup {
        SuccessorLookup::StillUnknown => serde_json::Value::Null,
        SuccessorLookup::Unavailable { reason } => serde_json::json!(reason.label()),
    };
    let marker = serde_json::json!({
        "recovery_id": Uuid::now_v7().to_string(),
        "decided": false,
        "lookup_unavailable_reason": lookup_reason,
        "same_request": conditions.same_request,
        "same_route": conditions.same_route,
        "current_authority": conditions.current_authority,
        "budget_covers_both": conditions.budget_covers_both,
        "effect": effect_label(conditions.effect),
        "context_changed": conditions.context_changed,
        "late_result": if conditions.late_complete {
            serde_json::json!("complete_selected")
        } else {
            serde_json::Value::Null
        },
        "allowance_consumed": false,
        "predecessor_fenced": false,
        "disposition": serde_json::Value::Null,
        "pause_reason": serde_json::Value::Null,
        "successor_model_attempt_id": serde_json::Value::Null,
        "successor_manifest_id": serde_json::Value::Null,
        "successor_decision_id": serde_json::Value::Null,
        "dispatch_prohibited": false,
        "late_evidence_applied": false,
        "predecessor_model_attempt_id": origin.predecessor_attempt_id,
        "model_invocation_id": origin.model_invocation_id,
        "predecessor_usage_kind": "unknown",
        "predecessor_reservation_released": false,
        "successor_settles_predecessor": false,
        "supplies_tool_call": false,
        "advances_predecessor_continuation": false,
        "reuses_changed_context": false
    });
    Some(PreparedSuccessor { retrieval, marker })
}

pub(crate) async fn advance(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    assistance: Option<&ProjectAssistanceRecord>,
) -> Result<SuccessorWork, CompleteAgentRunError> {
    let mut row = load_decision(client, claim).await?;
    let Some(mut marker) = row.payload.get("unknown_create_successor").cloned() else {
        return Err(unavailable(std::io::Error::other(
            "The unknown-create successor subject is missing",
        )));
    };
    if flag(&marker, "dispatch_prohibited") {
        return Ok(SuccessorWork::Done(CompleteAgentRun::AlreadySettled));
    }
    let successor_id = text(&marker, "successor_model_attempt_id").map(str::to_owned);
    let decision = decide_unknown_create_successor(&facts_from(
        &marker,
        &row.status,
        successor_id.as_deref(),
        assistance,
    ));
    match decision {
        UnknownCreateSuccessorDecision::AlreadyDispatched => {
            finish_dispatched(client, claim, &mut row, &mut marker).await
        }
        UnknownCreateSuccessorDecision::ProhibitedByCancellation => {
            let consumed = flag(&marker, "allowance_consumed");
            let fenced = flag(&marker, "predecessor_fenced");
            seal(
                &mut marker,
                "prohibited",
                /*pause_reason*/ None,
                consumed,
                fenced,
            );
            marker["dispatch_prohibited"] = serde_json::json!(true);
            row.payload["unknown_create_successor"] = marker;
            write_decision_payload(client, claim, &row.attempt_id, &row.payload).await?;
            Ok(SuccessorWork::Done(CompleteAgentRun::Settled))
        }
        UnknownCreateSuccessorDecision::Pause { reason } => {
            seal(
                &mut marker,
                "paused",
                Some(reason.label()),
                /*allowance_consumed*/ false,
                /*predecessor_fenced*/ false,
            );
            row.payload["unknown_create_successor"] = marker;
            write_decision_payload(client, claim, &row.attempt_id, &row.payload).await?;
            update_run(
                client,
                claim,
                "paused",
                Some(&serde_json::json!({
                    "kind": "unknown_create_successor",
                    "reason": reason.label()
                })),
                /*clear_lease*/ true,
            )
            .await?;
            Ok(SuccessorWork::Done(CompleteAgentRun::Settled))
        }
        UnknownCreateSuccessorDecision::FenceAndDispatch => {
            seal(
                &mut marker,
                "fenced",
                /*pause_reason*/ None,
                /*allowance_consumed*/ true,
                /*predecessor_fenced*/ true,
            );
            row.payload["unknown_create_successor"] = marker;
            row.payload["reservation"] =
                serde_json::json!({"kind": "worst_case", "released": false});
            row.payload["usage"] = serde_json::json!({"kind": "unknown"});
            write_decision_payload(client, claim, &row.attempt_id, &row.payload).await?;
            Ok(SuccessorWork::Hold("successor_fence"))
        }
        UnknownCreateSuccessorDecision::ResumePendingDispatch => {
            dispatch_pending(client, claim, &mut row, &mut marker).await
        }
    }
}

pub(crate) async fn prohibit_automatic_successor(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<(), CompleteAgentRunError> {
    let Some(row) = client
        .query_opt(
            "SELECT model_attempt_id::text, payload::text
               FROM storyos.model_attempts
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid
                AND attempt_role = 'decision' AND decision_position = 0",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
            ],
        )
        .await
        .map_err(unavailable)?
    else {
        return Ok(());
    };
    let attempt_id: String = row.get(0);
    let mut payload: serde_json::Value =
        serde_json::from_str(&row.get::<_, String>(1)).map_err(unavailable)?;
    let Some(marker) = payload.get_mut("unknown_create_successor") else {
        return Ok(());
    };
    if text(marker, "successor_model_attempt_id").is_some() {
        return Ok(());
    }
    let consumed = flag(marker, "allowance_consumed");
    let fenced = flag(marker, "predecessor_fenced");
    seal(
        marker,
        "prohibited",
        /*pause_reason*/ None,
        consumed,
        fenced,
    );
    marker["dispatch_prohibited"] = serde_json::json!(true);
    let claim = ClaimedAgentRun {
        project_scope: scope.clone(),
        run_id: run_id.to_owned(),
        fence_token: 0,
    };
    write_decision_payload(client, &claim, &attempt_id, &payload).await
}

pub(crate) async fn load_unknown_create_successor(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<Option<UnknownCreateSuccessorInspect>, storyos_application::CreateAgentRunError> {
    let Some(payload) = decision_payload(client, scope, run_id).await? else {
        return Ok(None);
    };
    let Some(marker) = payload.get("unknown_create_successor") else {
        return Ok(None);
    };
    if !flag(marker, "decided") {
        return Ok(None);
    }
    let successor_model_attempt_id = text(marker, "successor_model_attempt_id").map(str::to_owned);
    let prohibited =
        flag(marker, "dispatch_prohibited") || text(marker, "disposition") == Some("prohibited");
    let disposition = if prohibited {
        UnknownCreateSuccessorDisposition::Prohibited
    } else if successor_model_attempt_id.is_some()
        || text(marker, "disposition") == Some("dispatched")
    {
        UnknownCreateSuccessorDisposition::Dispatched
    } else if text(marker, "disposition") == Some("paused") {
        UnknownCreateSuccessorDisposition::Paused
    } else {
        UnknownCreateSuccessorDisposition::Fenced
    };
    Ok(Some(UnknownCreateSuccessorInspect {
        recovery_id: required_text(marker, "recovery_id").map_err(read_error)?,
        disposition,
        pause_reason: text(marker, "pause_reason").map(str::to_owned),
        lookup_unavailable_reason: text(marker, "lookup_unavailable_reason").map(str::to_owned),
        predecessor_model_attempt_id: required_text(marker, "predecessor_model_attempt_id")
            .map_err(read_error)?,
        successor_model_attempt_id,
        model_invocation_id: required_text(marker, "model_invocation_id").map_err(read_error)?,
        predecessor_fenced: flag(marker, "predecessor_fenced"),
        allowance_consumed: flag(marker, "allowance_consumed"),
        predecessor_usage_kind: text(marker, "predecessor_usage_kind")
            .unwrap_or("unknown")
            .to_owned(),
        predecessor_reservation_released: flag(marker, "predecessor_reservation_released"),
        successor_settles_predecessor: flag(marker, "successor_settles_predecessor"),
        supplies_tool_call: flag(marker, "supplies_tool_call"),
        advances_predecessor_continuation: flag(marker, "advances_predecessor_continuation"),
        reuses_changed_context: flag(marker, "reuses_changed_context"),
    }))
}

pub(crate) async fn load_successor_selection(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<Option<SuccessorSelection>, storyos_application::CreateAgentRunError> {
    let Some(row) = client
        .query_opt(
            "SELECT continuation_binding_id::text, payload::text
               FROM storyos.model_attempts
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid
                AND attempt_role = 'successor'",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
            ],
        )
        .await
        .map_err(read_error)?
    else {
        return Ok(None);
    };
    let payload: serde_json::Value =
        serde_json::from_str(&row.get::<_, String>(1)).map_err(read_error)?;
    let selected = payload
        .pointer("/decision/selected")
        .and_then(serde_json::Value::as_bool)
        == Some(true);
    if !selected {
        return Ok(None);
    }
    Ok(Some(SuccessorSelection {
        payload,
        continuation_binding_id: row.get(0),
    }))
}

struct DecisionRow {
    status: String,
    attempt_id: String,
    invocation_id: String,
    conversation_id: String,
    payload: serde_json::Value,
}

async fn finish_dispatched(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &mut DecisionRow,
    marker: &mut serde_json::Value,
) -> Result<SuccessorWork, CompleteAgentRunError> {
    if late_pending(marker) {
        apply_late(marker, &mut row.payload);
        row.payload["unknown_create_successor"] = marker.clone();
        write_decision_payload(client, claim, &row.attempt_id, &row.payload).await?;
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

async fn dispatch_pending(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &mut DecisionRow,
    marker: &mut serde_json::Value,
) -> Result<SuccessorWork, CompleteAgentRunError> {
    if text(marker, "successor_model_attempt_id").is_none() {
        let created = insert_successor(client, claim, row).await?;
        marker["successor_model_attempt_id"] = serde_json::json!(created.attempt_id);
        marker["successor_manifest_id"] = serde_json::json!(created.manifest_id);
        marker["successor_decision_id"] = serde_json::json!(created.decision_id);
        seal(
            marker,
            "dispatched",
            /*pause_reason*/ None,
            /*allowance_consumed*/ true,
            /*predecessor_fenced*/ true,
        );
        row.payload["unknown_create_successor"] = marker.clone();
        row.payload["reservation"] = serde_json::json!({"kind": "worst_case", "released": false});
        row.payload["usage"] = serde_json::json!({"kind": "unknown"});
        write_decision_payload(client, claim, &row.attempt_id, &row.payload).await?;
        if late_pending(marker) {
            return Ok(SuccessorWork::Hold("successor_late"));
        }
    }
    finish_dispatched(client, claim, row, marker).await
}

fn apply_late(marker: &mut serde_json::Value, payload: &mut serde_json::Value) {
    payload["usage"] = serde_json::json!({"kind": "reported"});
    payload["reservation"] = serde_json::json!({"kind": "worst_case", "released": true});
    let attempt_id = text(marker, "predecessor_model_attempt_id").unwrap_or("");
    if let Some(evidence) = payload
        .get_mut("evidence")
        .and_then(|value| value.as_array_mut())
    {
        evidence.push(serde_json::json!({
            "kind": "provider_report",
            "attempt_id": attempt_id,
            "availability": "current",
            "report": "host_fake_late_predecessor"
        }));
    }
    marker["late_evidence_applied"] = serde_json::json!(true);
    marker["predecessor_usage_kind"] = serde_json::json!("reported");
    marker["predecessor_reservation_released"] = serde_json::json!(true);
    marker["successor_settles_predecessor"] = serde_json::json!(false);
}

struct CreatedSuccessor {
    attempt_id: String,
    manifest_id: String,
    decision_id: String,
}

async fn insert_successor(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &DecisionRow,
) -> Result<CreatedSuccessor, CompleteAgentRunError> {
    let attempt_id = Uuid::now_v7().to_string();
    let manifest_id = Uuid::now_v7().to_string();
    let decision_id = Uuid::now_v7().to_string();
    let binding_id = Uuid::now_v7().to_string();
    let requirement_id = Uuid::now_v7().to_string();
    let snapshot_id = Uuid::now_v7().to_string();
    let destination_manifest_id = Uuid::now_v7().to_string();
    let outbound_manifest_id = Uuid::now_v7().to_string();
    copy_requirement(client, claim, &requirement_id, &snapshot_id).await?;
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
        "items": [complete_item()],
        "decision": {
            "kind": "advisory",
            "decision_id": decision_id,
            "selected": true,
            "text": ADVISORY_TEXT,
            "authoritative": false,
            "advances_continuation": true
        },
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
        successor_payload["produced_binding"] =
            crate::agent_run_continuation::encode_produced_binding(
                &binding_id,
                &attempt_id,
                &wire.admission,
            );
        successor_payload["continuation"] = crate::agent_run_continuation::encode_wire(&wire);
    }
    client
        .execute(
            "INSERT INTO storyos.model_attempts
               (owner_user_id, project_id, run_id, model_attempt_id, destination_attempt_id,
                outbound_disclosure_event_id, destination_context_manifest_id,
                outbound_disclosure_manifest_id, wire_payload_projection_id,
                model_invocation_id, conversation_id, decision_id, continuation_binding_id,
                dispatch_state, payload, attempt_role)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::uuid, $7::text::uuid, $8::text::uuid,
                     $9::text::uuid, $10::text::uuid, $11::text::uuid, $12::text::uuid,
                     $13::text::uuid, 'settled', $14::text::jsonb, 'successor')",
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
                &decision_id,
                &binding_id,
                &successor_payload.to_string(),
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(CreatedSuccessor {
        attempt_id,
        manifest_id,
        decision_id,
    })
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

async fn load_decision(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<DecisionRow, CompleteAgentRunError> {
    let row = client
        .query_one(
            "SELECT run.status, attempt.model_attempt_id::text, attempt.model_invocation_id::text,
                    attempt.conversation_id::text, attempt.payload::text
               FROM storyos.agent_runs AS run
               JOIN storyos.model_attempts AS attempt
                 ON (attempt.owner_user_id, attempt.project_id, attempt.run_id) =
                    (run.owner_user_id, run.project_id, run.run_id)
                AND attempt.attempt_role = 'decision' AND attempt.decision_position = 0
              WHERE run.owner_user_id = $1::text::uuid
                AND run.project_id = $2::text::uuid
                AND run.run_id = $3::text::uuid",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(DecisionRow {
        status: row.get(0),
        attempt_id: row.get(1),
        invocation_id: row.get(2),
        conversation_id: row.get(3),
        payload: serde_json::from_str(&row.get::<_, String>(4)).map_err(unavailable)?,
    })
}

async fn decision_payload(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<Option<serde_json::Value>, storyos_application::CreateAgentRunError> {
    let Some(row) = client
        .query_opt(
            "SELECT payload::text
               FROM storyos.model_attempts
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid
                AND attempt_role = 'decision' AND decision_position = 0",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
            ],
        )
        .await
        .map_err(read_error)?
    else {
        return Ok(None);
    };
    serde_json::from_str(&row.get::<_, String>(0))
        .map(Some)
        .map_err(read_error)
}

async fn write_decision_payload(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    attempt_id: &str,
    payload: &serde_json::Value,
) -> Result<(), CompleteAgentRunError> {
    let updated = client
        .execute(
            "UPDATE storyos.model_attempts
                SET payload = $4::text::jsonb
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND model_attempt_id = $3::text::uuid
                AND attempt_role = 'decision' AND decision_position = 0
                AND decision_id IS NULL",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &attempt_id,
                &payload.to_string(),
            ],
        )
        .await
        .map_err(unavailable)?;
    if updated != 1 {
        return Err(unavailable(std::io::Error::other(
            "The predecessor Attempt could not record the successor fence",
        )));
    }
    Ok(())
}

fn facts_from(
    marker: &serde_json::Value,
    status: &str,
    successor_id: Option<&str>,
    assistance: Option<&ProjectAssistanceRecord>,
) -> UnknownCreateSuccessorFacts {
    let allowance = if successor_id.is_some() {
        SuccessorAllowance::Dispatched
    } else if flag(marker, "allowance_consumed") {
        SuccessorAllowance::ConsumedPendingDispatch
    } else {
        SuccessorAllowance::Available
    };
    let admitted = matches!(
        assistance.map(|record| record.availability),
        Some(AssistanceAvailability::Available)
    );
    UnknownCreateSuccessorFacts {
        lookup: match text(marker, "lookup_unavailable_reason") {
            Some("missing_reference") => SuccessorLookup::Unavailable {
                reason: LookupUnavailable::MissingReference,
            },
            Some("unsupported_retrieval") => SuccessorLookup::Unavailable {
                reason: LookupUnavailable::UnsupportedRetrieval,
            },
            _ => SuccessorLookup::StillUnknown,
        },
        same_request: flag(marker, "same_request"),
        same_route: flag(marker, "same_route"),
        current_authority: flag(marker, "current_authority") && admitted,
        budget_covers_both: flag(marker, "budget_covers_both"),
        effect: match text(marker, "effect") {
            Some("none") => SuccessorEffect::None,
            Some("unresolved") => SuccessorEffect::Unresolved,
            _ => SuccessorEffect::AbsentUnsupported,
        },
        cancelled: status == "cancelled",
        allowance,
        context_changed: marker
            .get("context_changed")
            .and_then(serde_json::Value::as_bool)
            != Some(false),
    }
}

fn seal(
    marker: &mut serde_json::Value,
    disposition: &str,
    pause_reason: Option<&str>,
    allowance_consumed: bool,
    predecessor_fenced: bool,
) {
    marker["decided"] = serde_json::json!(true);
    marker["disposition"] = serde_json::json!(disposition);
    marker["pause_reason"] = match pause_reason {
        Some(reason) => serde_json::json!(reason),
        None => serde_json::Value::Null,
    };
    marker["allowance_consumed"] = serde_json::json!(allowance_consumed);
    marker["predecessor_fenced"] = serde_json::json!(predecessor_fenced);
    marker["successor_settles_predecessor"] = serde_json::json!(false);
    marker["supplies_tool_call"] = serde_json::json!(false);
    marker["advances_predecessor_continuation"] = serde_json::json!(false);
    marker["reuses_changed_context"] = serde_json::json!(false);
}

fn late_pending(marker: &serde_json::Value) -> bool {
    text(marker, "late_result") == Some("complete_selected")
        && !flag(marker, "late_evidence_applied")
}

fn effect_label(effect: SuccessorEffect) -> &'static str {
    match effect {
        SuccessorEffect::None => "none",
        SuccessorEffect::Unresolved => "unresolved",
        SuccessorEffect::AbsentUnsupported => "absent_unsupported",
    }
}

fn complete_item() -> serde_json::Value {
    serde_json::json!({
        "item_id": "1",
        "role": "assistant",
        "state": "complete",
        "phase": "complete",
        "text": ADVISORY_TEXT,
        "summary": "host_fake_native_text",
        "call_id": null,
        "arguments": null,
        "refusal": null,
        "hosted_report": null
    })
}

fn text<'a>(value: &'a serde_json::Value, field: &str) -> Option<&'a str> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|text| !text.is_empty())
}

fn required_text(value: &serde_json::Value, field: &str) -> Result<String, std::io::Error> {
    text(value, field)
        .map(str::to_owned)
        .ok_or_else(|| std::io::Error::other("The successor record is incomplete"))
}

fn flag(value: &serde_json::Value, field: &str) -> bool {
    value
        .get(field)
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn unavailable(error: impl std::error::Error + Send + Sync + 'static) -> CompleteAgentRunError {
    CompleteAgentRunError::Unavailable(Box::new(error))
}

fn read_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> storyos_application::CreateAgentRunError {
    storyos_application::CreateAgentRunError::Unavailable(Box::new(error))
}
