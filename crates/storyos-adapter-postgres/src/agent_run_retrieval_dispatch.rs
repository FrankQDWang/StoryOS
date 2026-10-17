//! Claims and records the Retrieve requests of one unknown create.

use storyos_application::{
    ClaimedAgentRun, CompleteAgentRunError, DeclaredTarget, DispatchClaim, ModelResponse,
    ModelUsage, Observation, RetrievePurpose, RetrieveRequest, WirePayloadProjection,
};
use storyos_core::{
    AgentDecisionOutcome, OriginalResultKeepReason, OriginalResultRetrievalDecision,
    RetrievedOriginalResult, StreamItemState, validate_agent_decision,
};
use uuid::Uuid;

use crate::agent_run_observation::{
    CreateResult, CreateSettlement, encode_items, persist_create_result,
};
use crate::agent_run_retrieval::{
    RecordedRetrieval, RetrievalAdvance, RetrievalRunWrite, SettledRetrieval, decide, settle,
    unavailable,
};
use crate::agent_run_work::{RunPhaseRow, WorkPhase};

/// Commits the Destination Attempt and Outbound Disclosure Event of one Retrieve request.
pub(crate) async fn commit(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    request: &RetrieveRequest,
    projection: &WirePayloadProjection,
) -> Result<DispatchClaim, CompleteAgentRunError> {
    let attempt_id = Uuid::now_v7().to_string();
    let (role, operation, manifests) = match request.purpose {
        RetrievePurpose::OriginalResult => (
            "retrieval",
            "retrieve_original_result",
            copy_retrieval_context(client, claim).await?,
        ),
        RetrievePurpose::LateResult => (
            "late_retrieval",
            "retrieve_late_result",
            retrieval_manifests(client, claim).await?,
        ),
    };
    let payload: serde_json::Value =
        serde_json::from_str(run.attempt_payload.as_deref().unwrap_or("null"))
            .map_err(unavailable)?;
    let bounds = payload
        .pointer("/original_result_retrieval/bounds")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("declared_read_only");
    let retrieval_payload = serde_json::json!({
        "operation": operation,
        "original_model_attempt_id": request.original_model_attempt_id,
        "response_reference_id": request.response_reference,
        "repeats_original_create": false,
        "resumes_stream": false,
        "proves_create_idempotency": false,
        "bounds": bounds,
        "wire": { "digest": projection.digest, "mapping_revision": projection.mapping_revision },
        "result": null,
        "items": [],
        "decision": null,
        "usage": { "kind": "unknown" },
        "reservation": { "kind": "worst_case", "released": false },
        "disclosure_grant": { "grant_id": run.grant_id, "grant_scope": run.grant_scope },
        "evidence": [
            {
                "kind": "stored_reference",
                "attempt_id": attempt_id,
                "availability": "current",
                "reference_id": request.response_reference
            },
            {
                "kind": "provider_report",
                "attempt_id": attempt_id,
                "availability": "current",
                "report": format!("{}_original_result_retrieval", request.route.adapter.kind())
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
             SELECT owner_user_id, project_id, run_id, $4::text::uuid, $5::text::uuid,
                    $6::text::uuid, $7::text::uuid, $8::text::uuid, $9::text::uuid,
                    model_invocation_id, conversation_id, 'uncertain', $10::text::jsonb, $11
               FROM storyos.model_attempts
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid
                AND attempt_role = 'decision' AND decision_position = 0",
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
                &retrieval_payload.to_string(),
                &role,
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(DispatchClaim {
        model_attempt_id: attempt_id,
    })
}

/// Records one original-result observation, then seals the retrieval subject.
pub(crate) async fn record_original_result(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    dispatch: &DispatchClaim,
    observation: Observation,
) -> Result<WorkPhase, CompleteAgentRunError> {
    let payload: serde_json::Value =
        serde_json::from_str(run.attempt_payload.as_deref().unwrap_or("null"))
            .map_err(unavailable)?;
    let subject = payload
        .get("original_result_retrieval")
        .ok_or_else(|| unavailable(std::io::Error::other("The retrieval subject is missing")))?;
    let advance = RetrievalAdvance::of(run);
    let fenced = run.status == "cancelled";
    let declared: Vec<_> =
        crate::agent_run_create_dispatch::declared_targets(client, claim, &run.chapter_id)
            .await?
            .iter()
            .map(DeclaredTarget::location)
            .collect();
    let retrieved = RetrievedResult::classify(observation, &declared);
    let decision = decide(client, claim, subject, &advance, fenced, retrieved.kind).await?;
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
    let manifest_id: String = client
        .query_one(
            "UPDATE storyos.model_attempts AS attempt
                SET dispatch_state = $5,
                    payload = attempt.payload || jsonb_build_object(
                      'result', $6::text, 'items', $7::text::jsonb)
               FROM storyos.context_assembly_manifests AS assembly
              WHERE attempt.owner_user_id = $1::text::uuid
                AND attempt.project_id = $2::text::uuid
                AND attempt.run_id = $3::text::uuid
                AND attempt.model_attempt_id = $4::text::uuid
                AND (assembly.owner_user_id, assembly.project_id, assembly.run_id) =
                    (attempt.owner_user_id, attempt.project_id, attempt.run_id)
                AND assembly.manifest_role = 'retrieval' AND assembly.decision_position = 0
          RETURNING assembly.context_assembly_manifest_id::text",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &dispatch.model_attempt_id,
                &dispatch_state,
                &retrieved.label(),
                &retrieved.items.to_string(),
            ],
        )
        .await
        .map_err(unavailable)?
        .get(0);
    let sealed = settle(
        client,
        claim,
        &payload,
        &advance,
        SettledRetrieval {
            decision,
            retrieval: Some(RecordedRetrieval {
                attempt_id: dispatch.model_attempt_id.clone(),
                manifest_id,
            }),
            usage: retrieved.usage,
            fenced,
            run_write: if payload.get("unknown_create_successor").is_some() {
                RetrievalRunWrite::Defer
            } else {
                RetrievalRunWrite::Write
            },
        },
    )
    .await?;
    let (Some(mut sealed), Some(selected)) = (sealed, retrieved.selected) else {
        return Ok(WorkPhase::Hold("recovery"));
    };
    let AgentDecisionOutcome::Decision {
        advances_continuation,
        ..
    } = selected.outcome
    else {
        return Err(unavailable(std::io::Error::other(
            "A selected original result has no Agent Decision",
        )));
    };
    sealed["original_result_retrieval"]["advances_continuation"] =
        serde_json::json!(advances_continuation);
    let response = selected.response;
    persist_create_result(
        client,
        claim,
        run,
        CreateResult {
            attempt_id: advance.attempt_id,
            items: &response.items,
            outcome: selected.outcome,
            producer_output: response
                .output
                .as_ref()
                .and_then(|output| output.prose_changes.as_deref()),
            usage: response.usage,
            cache_use: response.cache_use,
            response_reference: response.response_reference.as_deref(),
            settlement: CreateSettlement::Response,
        },
        sealed,
    )
    .await
}

/// Records the late Retrieve of the fenced predecessor. Only a complete result is evidence.
pub(crate) async fn record_late_result(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    dispatch: &DispatchClaim,
    observation: Observation,
) -> Result<(), CompleteAgentRunError> {
    let mut row = crate::agent_run_successor::load_decision(client, claim).await?;
    let mut marker = row
        .payload
        .get("unknown_create_successor")
        .cloned()
        .ok_or_else(|| unavailable(std::io::Error::other("The successor marker is missing")))?;
    let declared: Vec<_> =
        crate::agent_run_create_dispatch::declared_targets(client, claim, &run.chapter_id)
            .await?
            .iter()
            .map(DeclaredTarget::location)
            .collect();
    let retrieved = RetrievedResult::classify(observation, &declared);
    client
        .execute(
            "UPDATE storyos.model_attempts
                SET dispatch_state = $7,
                    payload = payload || jsonb_build_object('result', $5::text, 'items', $6::text::jsonb)
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid AND model_attempt_id = $4::text::uuid",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &dispatch.model_attempt_id,
                &retrieved.label(),
                &retrieved.items.to_string(),
                &if retrieved.kind == RetrievedOriginalResult::Unknown {
                    "uncertain"
                } else {
                    "settled"
                },
            ],
        )
        .await
        .map_err(unavailable)?;
    if retrieved.kind == RetrievedOriginalResult::CompleteSelected {
        let adapter = storyos_core::model_registration(&run.model_registration_revision)
            .map_or("unknown", |registration| registration.adapter.kind());
        crate::agent_run_successor::apply_late(
            &mut marker,
            &mut row.payload,
            retrieved.usage,
            &format!("{adapter}_late_predecessor"),
        );
    }
    marker["late_result_checked"] = serde_json::json!(true);
    row.payload["unknown_create_successor"] = marker;
    crate::agent_run_successor::write_decision_payload(client, claim, &row.attempt_id, &row.payload)
        .await
}

/// The retrieved result of one Retrieve observation, as the Host classifies it.
pub(crate) struct RetrievedResult {
    pub kind: RetrievedOriginalResult,
    pub items: serde_json::Value,
    pub selected: Option<SelectedResult>,
    pub usage: ModelUsage,
}

/// A complete original response whose Agent Decision core validation selected.
pub(crate) struct SelectedResult {
    pub response: ModelResponse,
    pub outcome: AgentDecisionOutcome,
}

impl RetrievedResult {
    /// Validates the original response against the targets that its Create declared.
    pub(crate) fn classify(
        observation: Observation,
        declared: &[storyos_core::DeclaredLocation],
    ) -> Self {
        let Observation::Terminal(response) = observation else {
            return Self {
                kind: RetrievedOriginalResult::Unknown,
                items: serde_json::json!([]),
                selected: None,
                usage: ModelUsage::Unknown,
            };
        };
        let items = encode_items(&response.items);
        let complete = response
            .items
            .iter()
            .all(|item| item.state == StreamItemState::Complete);
        let usage = response.usage;
        match validate_agent_decision(response.output.as_ref(), declared) {
            outcome @ AgentDecisionOutcome::Decision { selected: true, .. } if complete => Self {
                kind: RetrievedOriginalResult::CompleteSelected,
                items,
                selected: Some(SelectedResult { response, outcome }),
                usage,
            },
            AgentDecisionOutcome::Decision { .. } | AgentDecisionOutcome::NoDecision => Self {
                kind: RetrievedOriginalResult::Incomplete,
                items,
                selected: None,
                usage,
            },
        }
    }

    pub(crate) fn label(&self) -> &'static str {
        match self.kind {
            RetrievedOriginalResult::CompleteSelected => "complete_selected",
            RetrievedOriginalResult::Incomplete => "incomplete",
            RetrievedOriginalResult::Unknown | RetrievedOriginalResult::NotRetrieved => "unknown",
        }
    }
}

struct RetrievalManifests {
    destination: String,
    outbound: String,
}

async fn copy_retrieval_context(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<RetrievalManifests, CompleteAgentRunError> {
    let requirement_id = Uuid::now_v7().to_string();
    let manifests = RetrievalManifests {
        destination: Uuid::now_v7().to_string(),
        outbound: Uuid::now_v7().to_string(),
    };
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
                AND requirement_role IN ('primary', 'later_request') AND decision_position = 0
                AND (requirement_role = 'primary' OR NOT EXISTS (
                  SELECT 1 FROM storyos.active_context_compactions AS refused
                   WHERE (refused.owner_user_id, refused.project_id, refused.run_id) =
                         ($1::text::uuid, $2::text::uuid, $3::text::uuid)
                     AND refused.install_state = 'refused'))
              ORDER BY requirement_role = 'later_request' DESC
              LIMIT 1",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &requirement_id,
                &Uuid::now_v7().to_string(),
            ],
        )
        .await
        .map_err(unavailable)?;
    let manifested = client
        .execute(
            "INSERT INTO storyos.context_assembly_manifests
               (owner_user_id, project_id, context_assembly_manifest_id,
                operation_requirement_id, run_id, sufficiency,
                destination_context_manifest_id, outbound_disclosure_manifest_id,
                payload, receipt_id, manifest_role, disclosure_grant_id, disclosure_grant_scope)
             SELECT owner_user_id, project_id, $4::text::uuid, $5::text::uuid, run_id,
                    sufficiency, $6::text::uuid, $7::text::uuid, payload, receipt_id, 'retrieval',
                    disclosure_grant_id, disclosure_grant_scope
               FROM storyos.context_assembly_manifests
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid
                AND manifest_role IN ('decision', 'later_request') AND decision_position = 0
                AND (manifest_role = 'decision' OR NOT EXISTS (
                  SELECT 1 FROM storyos.active_context_compactions AS refused
                   WHERE (refused.owner_user_id, refused.project_id, refused.run_id) =
                         ($1::text::uuid, $2::text::uuid, $3::text::uuid)
                     AND refused.install_state = 'refused'))
              ORDER BY manifest_role = 'later_request' DESC
              LIMIT 1",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &Uuid::now_v7().to_string(),
                &requirement_id,
                &manifests.destination,
                &manifests.outbound,
            ],
        )
        .await
        .map_err(unavailable)?;
    if inserted != 1 || manifested != 1 {
        return Err(unavailable(std::io::Error::other(
            "The decision Context Assembly is missing",
        )));
    }
    Ok(manifests)
}

/// A late Retrieve reuses the Destination Context Manifest of the original retrieval.
async fn retrieval_manifests(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<RetrievalManifests, CompleteAgentRunError> {
    let row = client
        .query_one(
            "SELECT destination_context_manifest_id::text, outbound_disclosure_manifest_id::text
               FROM storyos.model_attempts
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
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
    Ok(RetrievalManifests {
        destination: row.get(0),
        outbound: row.get(1),
    })
}
