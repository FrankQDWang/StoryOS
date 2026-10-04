use storyos_application::{
    ClaimedAgentRun, CompleteAgentRun, CompleteAgentRunError, DispatchClaim, ModelUsage,
    ProjectScope, ReferenceRetrieval, RequestAttempt, ResponseReference, RetrievePurpose,
    RetrieveRequest,
};
use storyos_core::{
    HOST_FAKE_MAPPING_REVISION, OriginalResultRetrievalDecision, OriginalResultRetrievalFacts,
    RetainedResponseReference, RetrievalBounds, RetrievalCapability, RetrievedOriginalResult,
    decide_original_result_retrieval,
};
use uuid::Uuid;

use crate::agent_run_work::update_run;
use crate::update_project_assistance::read_assistance_record;

pub(crate) enum RetrievalFence {
    Open,
    Fenced,
}

pub(crate) enum RetrievalRunWrite {
    Write,
    Defer,
}

pub(crate) struct RetrievalAdvance<'a> {
    pub attempt_id: &'a str,
    pub conversation_id: &'a str,
    pub run_status: &'a str,
}

pub(crate) enum RetrievalWork {
    Done(CompleteAgentRun),
    Retrieve(RetrieveRequest),
}

/// The retained retrieval subject of one unknown create, from the reference it reported.
pub(crate) fn subject(
    reference: Option<&ResponseReference>,
    claim: &ClaimedAgentRun,
    conversation_id: &str,
    destination_identity: &str,
) -> serde_json::Value {
    let binding = reference.map(|reference| &reference.reported_binding);
    let retrieval = reference.map(|reference| reference.retrieval);
    serde_json::json!({
        "reconciliation_id": Uuid::now_v7().to_string(),
        "reconciled": false,
        "owner_user_id": binding
            .and_then(|binding| binding.owner_user_id.as_deref())
            .unwrap_or(claim.project_scope.owner_user_id.as_ref()),
        "project_id": claim.project_scope.project_id.as_ref(),
        "conversation_id": binding
            .and_then(|binding| binding.conversation_id.as_deref())
            .unwrap_or(conversation_id),
        "destination_identity": binding
            .and_then(|binding| binding.destination_identity.as_deref())
            .unwrap_or(destination_identity),
        "mapping_revision": binding
            .map_or(HOST_FAKE_MAPPING_REVISION, |binding| binding.mapping_revision.as_str()),
        "capability": match retrieval {
            Some(ReferenceRetrieval::Unsupported) => "unsupported",
            Some(ReferenceRetrieval::Supported { .. }) | None => "supported",
        },
        "bounds": match retrieval {
            Some(ReferenceRetrieval::Supported { bounds: RetrievalBounds::Unknown }) => "unknown",
            Some(ReferenceRetrieval::Supported {
                bounds: RetrievalBounds::DeclaredReadOnly,
            })
            | Some(ReferenceRetrieval::Unsupported)
            | None => "declared_read_only",
        },
        "reference_id": reference.map(|reference| reference.reference_id.as_str()),
    })
}

/// Keeps a blocked subject unknown, or returns the Retrieve request that the gates admit.
pub(crate) async fn advance_original_result_retrieval(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    payload: &serde_json::Value,
    advance: RetrievalAdvance<'_>,
    fence: RetrievalFence,
    run_write: RetrievalRunWrite,
) -> Result<RetrievalWork, CompleteAgentRunError> {
    let fenced = matches!(fence, RetrievalFence::Fenced);
    let subject = payload
        .get("original_result_retrieval")
        .ok_or_else(|| unavailable(std::io::Error::other("The retrieval subject is missing")))?;
    if flag(subject, "reconciled") {
        if !fenced && matches!(run_write, RetrievalRunWrite::Write) {
            restore_reconciled(client, claim, subject).await?;
        }
        return Ok(RetrievalWork::Done(CompleteAgentRun::AlreadySettled));
    }
    let decision = decide(
        client,
        claim,
        subject,
        &advance,
        fenced,
        RetrievedOriginalResult::NotRetrieved,
    )
    .await?;
    if let OriginalResultRetrievalDecision::KeepUnknown {
        admit_retrieval: false,
        ..
    } = decision
    {
        settle(
            client,
            claim,
            payload,
            &advance,
            SettledRetrieval {
                decision,
                retrieval: None,
                supplied: None,
                usage: ModelUsage::Unknown,
                fenced,
                run_write,
            },
        )
        .await?;
        return Ok(RetrievalWork::Done(CompleteAgentRun::Settled));
    }
    let existing = client
        .query_opt(
            "SELECT model_attempt_id::text FROM storyos.model_attempts
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid
                AND attempt_role = 'retrieval' AND decision_position = 0",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(RetrievalWork::Retrieve(RetrieveRequest {
        attempt: match existing {
            Some(row) => RequestAttempt::Claimed(DispatchClaim {
                model_attempt_id: row.get(0),
            }),
            None => RequestAttempt::New,
        },
        purpose: RetrievePurpose::OriginalResult,
        original_model_attempt_id: advance.attempt_id.to_owned(),
        response_reference: text(subject, "reference_id").unwrap_or_default().to_owned(),
    }))
}

/// Decides one subject with the current assistance admission and the retrieved result.
pub(crate) async fn decide(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    subject: &serde_json::Value,
    advance: &RetrievalAdvance<'_>,
    fenced: bool,
    retrieved: RetrievedOriginalResult,
) -> Result<OriginalResultRetrievalDecision, CompleteAgentRunError> {
    let assistance = read_assistance_record(client, &claim.project_scope)
        .await
        .map_err(unavailable)?
        .ok_or_else(|| {
            unavailable(std::io::Error::other(
                "Retrieval requires current assistance admission",
            ))
        })?;
    Ok(decide_original_result_retrieval(&facts(
        subject,
        claim,
        advance.conversation_id,
        &assistance.processing_destination_identity,
        fenced || advance.run_status == "cancelled",
        retrieved,
    )))
}

pub(crate) async fn load_original_result_retrieval(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<
    Option<storyos_application::OriginalResultRetrievalInspect>,
    storyos_application::CreateAgentRunError,
> {
    let Some(payload) = client
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
    let payload: serde_json::Value =
        serde_json::from_str(&payload.get::<_, String>(0)).map_err(read_error)?;
    let Some(subject) = payload.get("original_result_retrieval") else {
        return Ok(None);
    };
    if subject
        .get("reconciled")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        return Ok(None);
    }
    let disposition = match text(subject, "disposition") {
        Some("kept_unknown") => {
            storyos_application::OriginalResultRetrievalDisposition::KeptUnknown
        }
        Some("evidence_only") => {
            storyos_application::OriginalResultRetrievalDisposition::EvidenceOnly
        }
        Some("settled") => storyos_application::OriginalResultRetrievalDisposition::Settled,
        _ => {
            return Err(read_error(std::io::Error::other(
                "The original-result retrieval disposition is unknown",
            )));
        }
    };
    Ok(Some(storyos_application::OriginalResultRetrievalInspect {
        reconciliation_id: required_text(subject, "reconciliation_id").map_err(read_error)?,
        disposition,
        keep_reason: text(subject, "keep_reason").map(str::to_owned),
        original_model_attempt_id: required_text(subject, "original_model_attempt_id")
            .map_err(read_error)?,
        response_reference_id: text(subject, "response_reference_id").map(str::to_owned),
        retrieval_attempt_id: text(subject, "retrieval_attempt_id").map(str::to_owned),
        assembly_manifest_id: text(subject, "assembly_manifest_id").map(str::to_owned),
        repeats_original_create: flag(subject, "repeats_original_create"),
        resumes_stream: flag(subject, "resumes_stream"),
        proves_create_idempotency: flag(subject, "proves_create_idempotency"),
        supplies_decision: flag(subject, "supplies_decision"),
        supplies_tool_call: flag(subject, "supplies_tool_call"),
        advances_continuation: flag(subject, "advances_continuation"),
        reservation_released: flag(subject, "reservation_released"),
        usage_kind: text(subject, "usage_kind").unwrap_or("unknown").to_owned(),
    }))
}

pub(crate) struct SettledRetrieval {
    pub decision: OriginalResultRetrievalDecision,
    pub retrieval: Option<RecordedRetrieval>,
    pub supplied: Option<SuppliedDecision>,
    /// The usage of the original response. It belongs to the original Attempt only.
    pub usage: ModelUsage,
    pub fenced: bool,
    pub run_write: RetrievalRunWrite,
}

pub(crate) struct RecordedRetrieval {
    pub attempt_id: String,
    pub manifest_id: String,
}

/// The retrieved native items and advisory text that a selected original result supplies.
pub(crate) struct SuppliedDecision {
    pub items: serde_json::Value,
    pub text: String,
}

/// Seals the subject and settles the original Attempt and the Run from one decision.
pub(crate) async fn settle(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    payload: &serde_json::Value,
    advance: &RetrievalAdvance<'_>,
    settled: SettledRetrieval,
) -> Result<(), CompleteAgentRunError> {
    let SettledRetrieval {
        decision,
        retrieval,
        supplied,
        usage,
        fenced,
        run_write,
    } = settled;
    let defer_run_status = matches!(run_write, RetrievalRunWrite::Defer);
    let subject = payload
        .get("original_result_retrieval")
        .ok_or_else(|| unavailable(std::io::Error::other("The retrieval subject is missing")))?;
    let retrieval_attempt_id = retrieval
        .as_ref()
        .map(|recorded| recorded.attempt_id.clone());
    let assembly_manifest_id = retrieval.map(|recorded| recorded.manifest_id);
    let (disposition, keep_reason, supplies_decision, advances_continuation) = match decision {
        OriginalResultRetrievalDecision::KeepUnknown { reason, .. } => {
            ("kept_unknown", Some(reason.label()), false, false)
        }
        OriginalResultRetrievalDecision::EvidenceOnly => ("evidence_only", None, false, false),
        OriginalResultRetrievalDecision::SettleSelected => ("settled", None, true, true),
    };
    let mut next = payload.clone();
    let mut marker = subject.clone();
    marker["reconciled"] = serde_json::json!(true);
    marker["disposition"] = serde_json::json!(disposition);
    marker["keep_reason"] = match keep_reason {
        Some(reason) => serde_json::json!(reason),
        None => serde_json::Value::Null,
    };
    marker["original_model_attempt_id"] = serde_json::json!(advance.attempt_id);
    marker["response_reference_id"] = subject
        .get("reference_id")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    marker["retrieval_attempt_id"] = retrieval_attempt_id
        .clone()
        .map_or(serde_json::Value::Null, serde_json::Value::String);
    marker["assembly_manifest_id"] = assembly_manifest_id
        .clone()
        .map_or(serde_json::Value::Null, serde_json::Value::String);
    marker["repeats_original_create"] = serde_json::json!(false);
    marker["resumes_stream"] = serde_json::json!(false);
    marker["proves_create_idempotency"] = serde_json::json!(false);
    marker["supplies_decision"] = serde_json::json!(supplies_decision);
    marker["supplies_tool_call"] = serde_json::json!(false);
    marker["advances_continuation"] = serde_json::json!(advances_continuation);
    let reservation_released = supplies_decision;
    marker["reservation_released"] = serde_json::json!(reservation_released);
    let usage = crate::agent_run_observation::encode_usage(usage);
    marker["usage_kind"] = usage["kind"].clone();
    next["original_result_retrieval"] = marker;
    next["reservation"] =
        serde_json::json!({"kind": "worst_case", "released": reservation_released});
    next["usage"] = usage;
    if let (true, Some(supplied)) = (supplies_decision, supplied) {
        let binding = Uuid::now_v7().to_string();
        let decision_id = Uuid::now_v7().to_string();
        next["items"] = supplied.items;
        next["decision"] = serde_json::json!({
            "kind": "advisory",
            "decision_id": decision_id,
            "selected": true,
            "text": supplied.text,
            "authoritative": false,
            "advances_continuation": true
        });
        if let Some(wire) = crate::agent_run_continuation::parse_wire(payload) {
            next["produced_binding"] = crate::agent_run_continuation::encode_produced_binding(
                &binding,
                advance.attempt_id,
                &wire.admission,
            );
        }
        write_settled_decision(client, claim, &decision_id, &binding, &next).await?;
        if !fenced && !defer_run_status {
            update_run(
                client,
                claim,
                "completed",
                /*settlement*/ None,
                /*clear_lease*/ true,
            )
            .await?;
        }
    } else {
        write_unsettled_payload(client, claim, advance.attempt_id, &next).await?;
        if !fenced
            && !defer_run_status
            && matches!(
                decision,
                OriginalResultRetrievalDecision::KeepUnknown { .. }
            )
        {
            let reason = keep_reason.unwrap_or("unknown_result");
            update_run(
                client,
                claim,
                "waiting",
                Some(&serde_json::json!({"kind": "outcome_unknown", "reason": reason})),
                /*clear_lease*/ true,
            )
            .await?;
        }
    }
    Ok(())
}

async fn write_settled_decision(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    decision_id: &str,
    binding_id: &str,
    payload: &serde_json::Value,
) -> Result<(), CompleteAgentRunError> {
    let updated = client
        .execute(
            "UPDATE storyos.model_attempts
                SET decision_id = $4::text::uuid,
                    continuation_binding_id = $5::text::uuid,
                    dispatch_state = 'settled',
                    payload = $6::text::jsonb
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid
                AND attempt_role = 'decision' AND decision_position = 0
                AND decision_id IS NULL",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &decision_id,
                &binding_id,
                &payload.to_string(),
            ],
        )
        .await
        .map_err(unavailable)?;
    if updated != 1 {
        return Err(unavailable(std::io::Error::other(
            "The original Attempt could not accept the retrieved Decision",
        )));
    }
    Ok(())
}

async fn write_unsettled_payload(
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
                AND dispatch_state = 'uncertain'
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
            "The original Attempt could not record retrieval",
        )));
    }
    Ok(())
}

async fn restore_reconciled(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    subject: &serde_json::Value,
) -> Result<(), CompleteAgentRunError> {
    match text(subject, "disposition") {
        Some("settled") => {
            update_run(
                client,
                claim,
                "completed",
                /*settlement*/ None,
                /*clear_lease*/ true,
            )
            .await
        }
        Some("kept_unknown") => {
            let reason = text(subject, "keep_reason").unwrap_or("unknown_result");
            update_run(
                client,
                claim,
                "waiting",
                Some(&serde_json::json!({"kind": "outcome_unknown", "reason": reason})),
                /*clear_lease*/ true,
            )
            .await
        }
        _ => Ok(()),
    }
}

fn facts(
    subject: &serde_json::Value,
    claim: &ClaimedAgentRun,
    conversation_id: &str,
    destination_identity: &str,
    run_fenced: bool,
    retrieved: RetrievedOriginalResult,
) -> OriginalResultRetrievalFacts {
    OriginalResultRetrievalFacts {
        capability: match text(subject, "capability") {
            Some("unsupported") => RetrievalCapability::Unsupported,
            _ => RetrievalCapability::Supported,
        },
        bounds: match text(subject, "bounds") {
            Some("unknown") => RetrievalBounds::Unknown,
            _ => RetrievalBounds::DeclaredReadOnly,
        },
        reference: match text(subject, "reference_id") {
            Some(_) => RetainedResponseReference::Present,
            None => RetainedResponseReference::Absent,
        },
        scope_permitted: text(subject, "owner_user_id")
            == Some(claim.project_scope.owner_user_id.as_ref())
            && text(subject, "project_id") == Some(claim.project_scope.project_id.as_ref()),
        conversation_permitted: text(subject, "conversation_id") == Some(conversation_id),
        destination_permitted: text(subject, "destination_identity") == Some(destination_identity),
        mapping_permitted: text(subject, "mapping_revision") == Some(HOST_FAKE_MAPPING_REVISION),
        retrieved,
        run_fenced,
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
        .ok_or_else(|| std::io::Error::other("The retrieval record is incomplete"))
}

fn flag(value: &serde_json::Value, field: &str) -> bool {
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
