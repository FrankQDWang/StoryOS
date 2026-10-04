use storyos_application::{
    ClaimedAgentRun, CompleteAgentRun, CompleteAgentRunError, CreateRequest, ModelUsage,
    ProjectAssistanceRecord, ProjectScope, RetrieveRequest, UnknownCreateSuccessorDisposition,
    UnknownCreateSuccessorInspect,
};
use storyos_core::{
    AssistanceAvailability, LookupUnavailable, SuccessorAllowance, SuccessorEffect,
    SuccessorLookup, UnknownCreateSuccessorDecision, UnknownCreateSuccessorFacts,
    decide_unknown_create_successor, scripted_successor_conditions, unknown_create_script,
};
use uuid::Uuid;

use crate::agent_run_work::{RunPhaseRow, update_run};

pub(crate) struct SuccessorOrigin<'a> {
    pub predecessor_attempt_id: &'a str,
    pub model_invocation_id: &'a str,
}

pub(crate) enum SuccessorWork {
    Done(CompleteAgentRun),
    Hold(&'static str),
    Create(CreateRequest),
    Retrieve(RetrieveRequest),
}

pub(crate) struct SuccessorSelection {
    pub payload: serde_json::Value,
    pub continuation_binding_id: Option<String>,
}

/// The successor marker of one scripted unknown create, with the lookup its reference allows.
pub(crate) fn prepare_subject(
    author_message: &str,
    origin: &SuccessorOrigin<'_>,
    lookup: SuccessorLookup,
) -> Option<serde_json::Value> {
    let conditions = scripted_successor_conditions(unknown_create_script(author_message))?;
    let lookup_reason = match lookup {
        SuccessorLookup::StillUnknown => serde_json::Value::Null,
        SuccessorLookup::Unavailable { reason } => serde_json::json!(reason.label()),
    };
    Some(serde_json::json!({
        "recovery_id": Uuid::now_v7().to_string(),
        "decided": false,
        "lookup_unavailable_reason": lookup_reason,
        "same_request": conditions.same_request,
        "same_route": conditions.same_route,
        "current_authority": conditions.current_authority,
        "budget_covers_both": conditions.budget_covers_both,
        "effect": effect_label(conditions.effect),
        "context_changed": conditions.context_changed,
        "late_result_checked": false,
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
    }))
}

pub(crate) async fn advance(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
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
            crate::agent_run_successor_dispatch::finish_dispatched(client, claim, &row, &marker)
                .await
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
        UnknownCreateSuccessorDecision::ResumePendingDispatch => Ok(SuccessorWork::Create(
            crate::agent_run_successor_dispatch::successor_request(client, claim, run).await?,
        )),
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

pub(crate) struct DecisionRow {
    status: String,
    pub attempt_id: String,
    pub invocation_id: String,
    pub conversation_id: String,
    pub payload: serde_json::Value,
}

/// Applies a late complete predecessor result as evidence. Only known usage releases the
/// worst-case reservation.
pub(crate) fn apply_late(
    marker: &mut serde_json::Value,
    payload: &mut serde_json::Value,
    usage: ModelUsage,
) {
    let released = !matches!(usage, ModelUsage::Unknown);
    payload["usage"] = crate::agent_run_observation::encode_usage(usage);
    payload["reservation"] = serde_json::json!({"kind": "worst_case", "released": released});
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
    marker["predecessor_usage_kind"] = payload["usage"]["kind"].clone();
    marker["predecessor_reservation_released"] = serde_json::json!(released);
    marker["successor_settles_predecessor"] = serde_json::json!(false);
}

pub(crate) async fn load_decision(
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

pub(crate) async fn write_decision_payload(
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

pub(crate) fn seal(
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

fn effect_label(effect: SuccessorEffect) -> &'static str {
    match effect {
        SuccessorEffect::None => "none",
        SuccessorEffect::Unresolved => "unresolved",
        SuccessorEffect::AbsentUnsupported => "absent_unsupported",
    }
}

pub(crate) fn text<'a>(value: &'a serde_json::Value, field: &str) -> Option<&'a str> {
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

pub(crate) fn flag(value: &serde_json::Value, field: &str) -> bool {
    value
        .get(field)
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

pub(crate) fn unavailable(
    error: impl std::error::Error + Send + Sync + 'static,
) -> CompleteAgentRunError {
    CompleteAgentRunError::Unavailable(Box::new(error))
}

fn read_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> storyos_application::CreateAgentRunError {
    storyos_application::CreateAgentRunError::Unavailable(Box::new(error))
}
