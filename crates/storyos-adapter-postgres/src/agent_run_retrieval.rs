use storyos_application::{ClaimedAgentRun, CompleteAgentRun, CompleteAgentRunError, ProjectScope};
use storyos_core::{
    ADVISORY_TEXT, HOST_FAKE_MAPPING_REVISION, OriginalResultKeepReason,
    OriginalResultRetrievalDecision, OriginalResultRetrievalFacts, OriginalResultScript,
    RetainedResponseReference, RetrievalBounds, RetrievalCapability, RetrievedOriginalResult,
    decide_original_result_retrieval, original_result_script,
};
use uuid::Uuid;

use crate::agent_run_work::update_run;
use crate::update_project_assistance::read_assistance_record;

const FOREIGN_SCOPE: &str = "018f0000-0000-7000-8000-00000000f033";
const FOREIGN_CONVERSATION: &str = "018f0000-0000-7000-8000-00000000f011";
const FOREIGN_DESTINATION: &str = "018f0000-0000-7000-8000-00000000f022";
const FOREIGN_MAPPING: &str = "storyos.host-fake.mapping.rejected";

pub(crate) enum RetrievalFence {
    Open,
    Fenced,
}

pub(crate) struct RetrievalAdvance<'a> {
    pub attempt_id: &'a str,
    pub conversation_id: &'a str,
    pub run_status: &'a str,
}

pub(crate) fn subject_record(
    author_message: &str,
    owner_user_id: &str,
    project_id: &str,
    conversation_id: &str,
    destination_identity: &str,
) -> Option<serde_json::Value> {
    let script = original_result_script(author_message);
    if matches!(script, OriginalResultScript::NotSubject) {
        return None;
    }
    let reference_present = !matches!(script, OriginalResultScript::MissingReference);
    let reference_id = reference_present.then(|| Uuid::now_v7().to_string());
    Some(serde_json::json!({
        "reconciliation_id": Uuid::now_v7().to_string(),
        "reconciled": false,
        "owner_user_id": if matches!(script, OriginalResultScript::ForeignScope) {
            FOREIGN_SCOPE
        } else {
            owner_user_id
        },
        "project_id": project_id,
        "conversation_id": if matches!(script, OriginalResultScript::ForeignConversation) {
            FOREIGN_CONVERSATION
        } else {
            conversation_id
        },
        "destination_identity": if matches!(script, OriginalResultScript::ForeignDestination) {
            FOREIGN_DESTINATION
        } else {
            destination_identity
        },
        "mapping_revision": if matches!(script, OriginalResultScript::ForeignMapping) {
            FOREIGN_MAPPING
        } else {
            HOST_FAKE_MAPPING_REVISION
        },
        "capability": if matches!(script, OriginalResultScript::Unsupported) {
            "unsupported"
        } else {
            "supported"
        },
        "bounds": if matches!(script, OriginalResultScript::UnknownBounds) {
            "unknown"
        } else {
            "declared_read_only"
        },
        "reference_id": reference_id,
        "result": match script {
            OriginalResultScript::Incomplete => "incomplete",
            OriginalResultScript::UnknownResult => "unknown",
            OriginalResultScript::CompleteSelected
            | OriginalResultScript::ForeignScope
            | OriginalResultScript::ForeignConversation
            | OriginalResultScript::ForeignMapping
            | OriginalResultScript::ForeignDestination
            | OriginalResultScript::Unsupported
            | OriginalResultScript::UnknownBounds => "complete_selected",
            OriginalResultScript::MissingReference | OriginalResultScript::NotSubject => "absent",
        }
    }))
}

pub(crate) async fn advance_original_result_retrieval(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    payload: &serde_json::Value,
    advance: RetrievalAdvance<'_>,
    fence: RetrievalFence,
) -> Result<CompleteAgentRun, CompleteAgentRunError> {
    let fenced = matches!(fence, RetrievalFence::Fenced);
    apply(client, claim, payload, &advance, fenced).await
}

pub(crate) async fn reconcile_fenced_original_result(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<(), CompleteAgentRunError> {
    let Some(row) = client
        .query_opt(
            "SELECT attempt.model_attempt_id::text, attempt.payload::text,
                    run.conversation_id::text, run.status
               FROM storyos.agent_runs AS run
               JOIN storyos.model_attempts AS attempt
                 ON (attempt.owner_user_id, attempt.project_id, attempt.run_id) =
                    (run.owner_user_id, run.project_id, run.run_id)
                AND attempt.attempt_role = 'decision'
              WHERE run.owner_user_id = $1::text::uuid
                AND run.project_id = $2::text::uuid
                AND run.run_id = $3::text::uuid",
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
    let payload: serde_json::Value =
        serde_json::from_str(&row.get::<_, String>(1)).map_err(unavailable)?;
    if payload.get("original_result_retrieval").is_none() {
        return Ok(());
    }
    let attempt_id: String = row.get(0);
    let conversation_id: String = row.get(2);
    let claim = ClaimedAgentRun {
        project_scope: scope.clone(),
        run_id: run_id.to_owned(),
        fence_token: 0,
    };
    advance_original_result_retrieval(
        client,
        &claim,
        &payload,
        RetrievalAdvance {
            attempt_id: &attempt_id,
            conversation_id: &conversation_id,
            run_status: &row.get::<_, String>(3),
        },
        RetrievalFence::Fenced,
    )
    .await?;
    Ok(())
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
                AND attempt_role = 'decision'",
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

async fn apply(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    payload: &serde_json::Value,
    advance: &RetrievalAdvance<'_>,
    fenced: bool,
) -> Result<CompleteAgentRun, CompleteAgentRunError> {
    let subject = payload
        .get("original_result_retrieval")
        .ok_or_else(|| unavailable(std::io::Error::other("The retrieval subject is missing")))?;
    if subject
        .get("reconciled")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        if !fenced {
            restore_reconciled(client, claim, subject).await?;
        }
        return Ok(CompleteAgentRun::AlreadySettled);
    }
    let assistance = read_assistance_record(client, &claim.project_scope)
        .await
        .map_err(unavailable)?
        .ok_or_else(|| {
            unavailable(std::io::Error::other(
                "Retrieval requires current assistance admission",
            ))
        })?;
    let decision = decide_original_result_retrieval(&facts(
        subject,
        claim,
        advance.conversation_id,
        &assistance.processing_destination_identity,
        fenced || advance.run_status == "cancelled",
    )?);
    let mut retrieval_attempt_id = None;
    let mut assembly_manifest_id = None;
    let admit = matches!(
        decision,
        OriginalResultRetrievalDecision::EvidenceOnly
            | OriginalResultRetrievalDecision::SettleSelected
            | OriginalResultRetrievalDecision::KeepUnknown {
                admit_retrieval: true,
                ..
            }
    );
    if admit {
        let admitted = admit_retrieval(client, claim, advance, subject, &decision).await?;
        retrieval_attempt_id = Some(admitted.attempt_id);
        assembly_manifest_id = Some(admitted.manifest_id);
    }
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
    marker["usage_kind"] = serde_json::json!("unknown");
    next["original_result_retrieval"] = marker;
    next["reservation"] =
        serde_json::json!({"kind": "worst_case", "released": reservation_released});
    next["usage"] = serde_json::json!({"kind": "unknown"});
    if supplies_decision {
        let binding = Uuid::now_v7().to_string();
        let decision_id = Uuid::now_v7().to_string();
        next["items"] = serde_json::json!([complete_item()]);
        next["decision"] = serde_json::json!({
            "kind": "advisory",
            "decision_id": decision_id,
            "selected": true,
            "text": ADVISORY_TEXT,
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
        if !fenced {
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
    Ok(CompleteAgentRun::Settled)
}

struct AdmittedRetrieval {
    attempt_id: String,
    manifest_id: String,
}

async fn admit_retrieval(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    advance: &RetrievalAdvance<'_>,
    subject: &serde_json::Value,
    decision: &OriginalResultRetrievalDecision,
) -> Result<AdmittedRetrieval, CompleteAgentRunError> {
    let invocation_id: String = client
        .query_one(
            "SELECT model_invocation_id::text
               FROM storyos.model_attempts
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND model_attempt_id = $3::text::uuid",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &advance.attempt_id,
            ],
        )
        .await
        .map_err(unavailable)?
        .get(0);
    let attempt_id = Uuid::now_v7().to_string();
    let manifest_id = Uuid::now_v7().to_string();
    let requirement_id = Uuid::now_v7().to_string();
    let snapshot_id = Uuid::now_v7().to_string();
    let destination_manifest_id = Uuid::now_v7().to_string();
    let outbound_manifest_id = Uuid::now_v7().to_string();
    let inserted = client
        .execute(
            "INSERT INTO storyos.operation_requirements
               (owner_user_id, project_id, operation_requirement_id, run_id,
                input_snapshot_id, receipt_id, payload, requirement_role)
             SELECT owner_user_id, project_id, $4::text::uuid, run_id,
                    $5::text::uuid, receipt_id, payload, 'retrieval'
               FROM storyos.operation_requirements
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid
                AND requirement_role = 'primary'",
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
    let manifested = client
        .execute(
            "INSERT INTO storyos.context_assembly_manifests
               (owner_user_id, project_id, context_assembly_manifest_id,
                operation_requirement_id, run_id, sufficiency,
                destination_context_manifest_id, outbound_disclosure_manifest_id,
                payload, receipt_id, manifest_role)
             SELECT owner_user_id, project_id, $4::text::uuid, $5::text::uuid, run_id,
                    sufficiency, $6::text::uuid, $7::text::uuid, payload, receipt_id, 'retrieval'
               FROM storyos.context_assembly_manifests
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid
                AND manifest_role = 'decision'",
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
    let reference_id = text(subject, "reference_id").unwrap_or("");
    let result = text(subject, "result").unwrap_or("unknown");
    let dispatch_state = if matches!(
        decision,
        OriginalResultRetrievalDecision::KeepUnknown {
            reason: OriginalResultKeepReason::UnknownResult,
            ..
        }
    ) {
        "uncertain"
    } else {
        "settled"
    };
    let items = if result == "complete_selected" {
        serde_json::json!([complete_item()])
    } else if result == "incomplete" {
        serde_json::json!([{
            "item_id": "1",
            "role": "assistant",
            "state": "incomplete",
            "phase": "incomplete",
            "text": ADVISORY_TEXT,
            "summary": "host_fake_native_text",
            "call_id": null,
            "arguments": null,
            "refusal": null,
            "hosted_report": null
        }])
    } else {
        serde_json::json!([])
    };
    let retrieval_payload = serde_json::json!({
        "operation": "retrieve_original_result",
        "original_model_attempt_id": advance.attempt_id,
        "response_reference_id": reference_id,
        "repeats_original_create": false,
        "resumes_stream": false,
        "proves_create_idempotency": false,
        "bounds": text(subject, "bounds").unwrap_or("declared_read_only"),
        "result": result,
        "items": items,
        "decision": null,
        "usage": { "kind": "unknown" },
        "reservation": { "kind": "worst_case", "released": false },
        "evidence": [
            {
                "kind": "stored_reference",
                "attempt_id": attempt_id,
                "availability": "current",
                "reference_id": reference_id
            },
            {
                "kind": "provider_report",
                "attempt_id": attempt_id,
                "availability": "current",
                "report": "host_fake_original_result_retrieval"
            },
            {
                "kind": "provider_opaque",
                "attempt_id": attempt_id,
                "availability": "unknown",
                "unknown_facts": ["provider_internal_content"]
            }
        ]
    });
    client
        .execute(
            "INSERT INTO storyos.model_attempts
               (owner_user_id, project_id, run_id, model_attempt_id, destination_attempt_id,
                outbound_disclosure_event_id, destination_context_manifest_id,
                outbound_disclosure_manifest_id, wire_payload_projection_id,
                model_invocation_id, conversation_id, dispatch_state, payload, attempt_role)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::uuid, $7::text::uuid, $8::text::uuid,
                     $9::text::uuid, $10::text::uuid, $11::text::uuid, $12,
                     $13::text::jsonb, 'retrieval')",
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
                &invocation_id,
                &advance.conversation_id,
                &dispatch_state,
                &retrieval_payload.to_string(),
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(AdmittedRetrieval {
        attempt_id,
        manifest_id,
    })
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
                AND attempt_role = 'decision'
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
                AND attempt_role = 'decision'
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
) -> Result<OriginalResultRetrievalFacts, CompleteAgentRunError> {
    let reference = match text(subject, "reference_id") {
        Some(_) => RetainedResponseReference::Present,
        None => RetainedResponseReference::Absent,
    };
    let capability = match text(subject, "capability") {
        Some("unsupported") => RetrievalCapability::Unsupported,
        _ => RetrievalCapability::Supported,
    };
    let bounds = match text(subject, "bounds") {
        Some("unknown") => RetrievalBounds::Unknown,
        _ => RetrievalBounds::DeclaredReadOnly,
    };
    let retrieved = match text(subject, "result") {
        Some("incomplete") => RetrievedOriginalResult::Incomplete,
        Some("unknown") => RetrievedOriginalResult::Unknown,
        Some("complete_selected") => RetrievedOriginalResult::CompleteSelected,
        _ => RetrievedOriginalResult::NotRetrieved,
    };
    Ok(OriginalResultRetrievalFacts {
        capability,
        bounds,
        reference,
        scope_permitted: text(subject, "owner_user_id")
            == Some(claim.project_scope.owner_user_id.as_ref())
            && text(subject, "project_id") == Some(claim.project_scope.project_id.as_ref()),
        conversation_permitted: text(subject, "conversation_id") == Some(conversation_id),
        destination_permitted: text(subject, "destination_identity") == Some(destination_identity),
        mapping_permitted: text(subject, "mapping_revision") == Some(HOST_FAKE_MAPPING_REVISION),
        retrieved,
        run_fenced,
    })
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
        .ok_or_else(|| std::io::Error::other("The retrieval record is incomplete"))
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
