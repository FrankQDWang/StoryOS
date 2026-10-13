//! The summary request of an active compaction through the Model Gateway (ADR 0039).

use storyos_application::{
    ClaimedAgentRun, CompleteAgentRunError, CreatePurpose, CreateRequest, DestinationRequest,
    DispatchClaim, ModelUsage, Observation, RequestAttempt, WirePayloadProjection,
};
use storyos_core::{
    AssistanceAvailability, SummaryObservation, compaction_assembly_record, encode_assembly_record,
    model_registration, stage_compaction, summary_text,
};
use uuid::Uuid;

use super::{load_chain_source, unavailable, work_error, write};
use crate::agent_run_work::{RunPhaseRow, WorkPhase, route_facts};
use crate::update_project_assistance::read_assistance_record;

/// The summary Attempt of the claimed AgentRun, and the prior response reference that it sent.
pub(crate) struct SummaryAttempt {
    pub model_attempt_id: String,
    pub prior_reference: Option<String>,
}

pub(crate) async fn load_summary_attempt(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<Option<SummaryAttempt>, CompleteAgentRunError> {
    let row = client
        .query_opt(
            "SELECT model_attempt_id::text, payload->'wire'->>'prior_response_reference'
               FROM storyos.model_attempts
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid AND attempt_role = 'compaction'",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map_err(work_error)?;
    Ok(row.map(|row| SummaryAttempt {
        model_attempt_id: row.get(/*idx*/ 0),
        prior_reference: row.get(/*idx*/ 1),
    }))
}

/// The response chain that the ordinary decision request continues: the prior decision Attempt
/// of the Conversation and its response reference.
pub(crate) struct PriorChain {
    pub model_attempt_id: String,
    pub reference: Option<String>,
    /// The reported input and output tokens of the prior response, or `None` when unreported.
    pub reported_tokens: Option<u64>,
}

/// The prior binding that the current continuation decision selects, or `None` when the decision
/// request continues no prior response. Then nothing is to be compacted.
pub(crate) async fn prior_chain(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
) -> Result<Option<PriorChain>, CompleteAgentRunError> {
    let Some(assistance) = read_assistance_record(client, &claim.project_scope)
        .await
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?
    else {
        return Ok(None);
    };
    let continuation = crate::agent_run_continuation::decide_continuation(
        client,
        claim,
        &run.conversation_id,
        &run.author_message,
        &assistance,
    )
    .await?;
    let Some(binding) = continuation.prior_binding_id else {
        return Ok(None);
    };
    let row = client
        .query_opt(
            "SELECT model_attempt_id::text, payload->>'response_reference',
                    CASE WHEN payload->'usage'->>'kind' = 'reported'
                         THEN ((payload->'usage'->>'input_tokens')::bigint
                               + (payload->'usage'->>'output_tokens')::bigint)::text END
               FROM storyos.model_attempts
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND continuation_binding_id = $3::text::uuid",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &binding,
            ],
        )
        .await
        .map_err(work_error)?;
    Ok(row.map(|row| PriorChain {
        model_attempt_id: row.get(/*idx*/ 0),
        reference: row.get(/*idx*/ 1),
        reported_tokens: row
            .get::<_, Option<String>>(/*idx*/ 2)
            .and_then(|tokens| tokens.parse().ok()),
    }))
}

/// The summary request: no project content, and the response reference of the prior chain.
pub(crate) async fn summary_request(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    attempt: RequestAttempt,
    previous_response_reference: Option<String>,
) -> Result<DestinationRequest, CompleteAgentRunError> {
    Ok(DestinationRequest::Create(CreateRequest {
        attempt,
        purpose: CreatePurpose::CompactionSummary,
        route: crate::model_registration::request_route(client, claim).await?,
        author_message: run.author_message.clone(),
        chapter_id: run.chapter_id.clone(),
        passage_resolution: None,
        passage_input: None,
        context: Vec::new(),
        declared_targets: Vec::new(),
        candidate_revision: None,
        previous_response_reference,
        successor_of: None,
    }))
}

/// Commits the dispatch claim of the summary request: its Context Assembly, manifests, and Model
/// Attempt. `None` means that the work changed: the current admission refuses the request, or the
/// prior chain is not the chain of the prepared request.
pub(crate) async fn commit_claim(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    request: &CreateRequest,
    projection: &WirePayloadProjection,
) -> Result<Option<DispatchClaim>, CompleteAgentRunError> {
    if run.settled()
        || run.attempt_id.is_some()
        || load_summary_attempt(client, claim).await?.is_some()
    {
        return Ok(None);
    }
    let Some(chain) = prior_chain(client, claim, run)
        .await?
        .filter(|chain| chain.reference == request.previous_response_reference)
    else {
        return Ok(None);
    };
    let decision_record = run
        .disclosure_request
        .as_ref()
        .ok_or_else(|| unavailable("Compaction requires the decision Context Assembly"))?;
    let record = compaction_assembly_record(decision_record);
    let assistance = read_assistance_record(client, &claim.project_scope)
        .await
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    // The next phase decides a refusal durably, so a refusal here only means changed work.
    if admit(run, assistance.as_ref(), &record).is_err() {
        return Ok(None);
    }
    let [destination_manifest_id, outbound_manifest_id] =
        [(); 2].map(|()| Uuid::now_v7().to_string());
    // The Context Assembly Manifest committed in the next work phase (ADR 0039). The claim adds
    // the destination and disclosure manifests and the grant that admitted it.
    let Some(manifest) = client
        .query_opt(
            "UPDATE storyos.context_assembly_manifests
                SET destination_context_manifest_id = $4::text::uuid,
                    outbound_disclosure_manifest_id = $5::text::uuid,
                    disclosure_grant_id = $6::text::uuid, disclosure_grant_scope = $7
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid AND manifest_role = 'compaction'
                AND destination_context_manifest_id IS NULL
          RETURNING context_assembly_manifest_id::text",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &destination_manifest_id,
                &outbound_manifest_id,
                &run.grant_id,
                &run.grant_scope,
            ],
        )
        .await
        .map_err(work_error)?
    else {
        return Ok(None);
    };
    let manifest_id: String = manifest.get(/*idx*/ 0);
    let scope = [
        &claim.project_scope.owner_user_id.as_ref(),
        &claim.project_scope.project_id.as_ref(),
    ];
    let model_attempt_id = Uuid::now_v7().to_string();
    let mut payload = serde_json::json!({
        "execution_profile": {
            "profile_revision": projection.execution_profile,
            "mapping_revision": projection.mapping_revision,
            "network_io": assistance.as_ref().map(|record| record.destination)
                != Some(storyos_core::DestinationKind::HostFake),
            "provider_bound": "unknown"
        },
        "purpose": "compaction_summary",
        "assembly_manifest_id": manifest_id,
        "wire": {
            "digest": projection.digest,
            "prior_response_reference": chain.reference,
            "prior_model_attempt_id": chain.model_attempt_id
        },
        "items": [],
        "usage": { "kind": "unknown" },
        "disclosure_grant": { "grant_id": run.grant_id, "grant_scope": run.grant_scope }
    });
    if let Some(bytes) = &projection.serialized_payload {
        payload["wire"]["serialized_payload"] = serde_json::json!(bytes);
    }
    if let Some(setting) = projection.cache_setting {
        payload["cache"] = crate::provider_evidence::encode_cache_setting(setting);
    }
    client
        .execute(
            "INSERT INTO storyos.model_attempts
               (owner_user_id, project_id, run_id, model_attempt_id, destination_attempt_id,
                outbound_disclosure_event_id, destination_context_manifest_id,
                outbound_disclosure_manifest_id, wire_payload_projection_id, model_invocation_id,
                conversation_id, dispatch_state, payload, attempt_role)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::uuid, $7::text::uuid, $8::text::uuid,
                     $9::text::uuid, $10::text::uuid, $11::text::uuid, 'uncertain',
                     $12::text::jsonb, 'compaction')",
            &[
                scope[0],
                scope[1],
                &claim.run_id,
                &model_attempt_id,
                &Uuid::now_v7().to_string(),
                &Uuid::now_v7().to_string(),
                &destination_manifest_id,
                &outbound_manifest_id,
                &Uuid::now_v7().to_string(),
                &Uuid::now_v7().to_string(),
                &run.conversation_id,
                &payload.to_string(),
            ],
        )
        .await
        .map_err(work_error)?;
    Ok(Some(DispatchClaim { model_attempt_id }))
}

/// Commits the operation requirement and the manifest of the summary request, with no project
/// content. This occurs before the store returns the request (ADR 0005 gate six, ADR 0039). The
/// dispatch claim adds the destination and disclosure manifests later.
pub(crate) async fn commit_assembly(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
) -> Result<(), CompleteAgentRunError> {
    let decision_record = run
        .disclosure_request
        .as_ref()
        .ok_or_else(|| unavailable("Compaction requires the decision Context Assembly"))?;
    let scope = [
        &claim.project_scope.owner_user_id.as_ref(),
        &claim.project_scope.project_id.as_ref(),
    ];
    let committed = client
        .query_opt(
            "SELECT 1 FROM storyos.context_assembly_manifests
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid AND manifest_role = 'compaction'",
            &[scope[0], scope[1], &claim.run_id],
        )
        .await
        .map_err(work_error)?;
    if committed.is_some() {
        return Ok(());
    }
    let [requirement_id, snapshot_id, manifest_id] = [(); 3].map(|()| Uuid::now_v7().to_string());
    let mut manifest_payload = encode_assembly_record(&compaction_assembly_record(decision_record));
    manifest_payload["operation_requirement"]["operation_requirement_id"] =
        serde_json::json!(requirement_id);
    manifest_payload["operation_requirement"]["input_snapshot_id"] = serde_json::json!(snapshot_id);
    let inserted = client
        .execute(
            "INSERT INTO storyos.operation_requirements
               (owner_user_id, project_id, operation_requirement_id, run_id, input_snapshot_id,
                receipt_id, payload, requirement_role)
             SELECT owner_user_id, project_id, $4::text::uuid, run_id, $5::text::uuid,
                    receipt_id, $6::text::jsonb, 'compaction'
               FROM storyos.operation_requirements
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND operation_requirement_id = $3::text::uuid",
            &[
                scope[0],
                scope[1],
                &decision_record
                    .operation_requirement
                    .operation_requirement_id,
                &requirement_id,
                &snapshot_id,
                &manifest_payload["operation_requirement"].to_string(),
            ],
        )
        .await
        .map_err(work_error)?;
    let manifest = client
        .execute(
            "INSERT INTO storyos.context_assembly_manifests
               (owner_user_id, project_id, context_assembly_manifest_id,
                operation_requirement_id, run_id, sufficiency, payload, receipt_id, manifest_role)
             SELECT owner_user_id, project_id, $4::text::uuid, $5::text::uuid, run_id,
                    sufficiency, $6::text::jsonb, receipt_id, 'compaction'
               FROM storyos.context_assembly_manifests
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND context_assembly_manifest_id = $3::text::uuid",
            &[
                scope[0],
                scope[1],
                &run.assembly_manifest_id,
                &manifest_id,
                &requirement_id,
                &manifest_payload.to_string(),
            ],
        )
        .await
        .map_err(work_error)?;
    if inserted != 1 || manifest != 1 {
        return Err(unavailable("The decision Context Assembly is missing"));
    }
    Ok(())
}

/// The current admission of the summary request: the pinned route and the grant scope over its
/// Context Assembly.
fn admit(
    run: &RunPhaseRow,
    assistance: Option<&storyos_application::ProjectAssistanceRecord>,
    record: &storyos_core::CurrentPassageAssemblyRecord,
) -> Result<(), &'static str> {
    if assistance.map(|record| record.availability) != Some(AssistanceAvailability::Available) {
        return Err("assistance_unavailable");
    }
    let mut facts = route_facts(run, assistance);
    facts.grant.request = Some(record);
    storyos_core::admit_route(&facts)
}

/// Whether the current admission admits the summary request before its dispatch claim. A
/// refused summary request leaves the decision request with the ordinary continuation.
pub(crate) async fn admitted(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
) -> Result<bool, CompleteAgentRunError> {
    let decision_record = run
        .disclosure_request
        .as_ref()
        .ok_or_else(|| unavailable("Compaction requires the decision Context Assembly"))?;
    let assistance = read_assistance_record(client, &claim.project_scope)
        .await
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    Ok(admit(
        run,
        assistance.as_ref(),
        &compaction_assembly_record(decision_record),
    )
    .is_ok())
}

/// Settles the summary Attempt with its observation, and stages its result: the summary, or a
/// refusal when the request supplied no complete summary.
pub(crate) async fn record(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    dispatch: &DispatchClaim,
    observation: Observation,
) -> Result<WorkPhase, CompleteAgentRunError> {
    let (items, usage, cache_use, reference, summary) = match &observation {
        Observation::Terminal(response) => (
            response.items.clone(),
            response.usage,
            Some(response.cache_use),
            response.response_reference.clone(),
            summary_text(&response.items),
        ),
        Observation::OutcomeUnknown { response_reference } => (
            Vec::new(),
            ModelUsage::Unknown,
            None,
            response_reference
                .as_ref()
                .map(|reference| reference.reference_id.clone()),
            None,
        ),
        Observation::NotSubmitted | Observation::Rejected { .. } => {
            (Vec::new(), ModelUsage::Unknown, None, None, None)
        }
    };
    let stored: String = client
        .query_one(
            "SELECT (payload->'items')::text FROM storyos.model_attempts
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND model_attempt_id = $3::text::uuid AND attempt_role = 'compaction'",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &dispatch.model_attempt_id,
            ],
        )
        .await
        .map_err(work_error)?
        .get::<_, Option<String>>(/*idx*/ 0)
        .unwrap_or_else(|| "[]".to_owned());
    let stored: serde_json::Value = serde_json::from_str(&stored)
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    let row = client
        .query_one(
            "UPDATE storyos.model_attempts
                SET dispatch_state = 'settled',
                    payload = payload
                      || jsonb_build_object('items', $4::text::jsonb, 'usage', $5::text::jsonb,
                                            'observation', $6::text)
                      || CASE WHEN $7::text IS NULL THEN '{}'::jsonb
                              ELSE jsonb_build_object('response_reference', $7::text) END
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND model_attempt_id = $3::text::uuid AND attempt_role = 'compaction'
          RETURNING payload::text, model_invocation_id::text",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &dispatch.model_attempt_id,
                &crate::agent_run_observation::merge_items(&stored, &items).to_string(),
                &crate::agent_run_observation::encode_usage(usage).to_string(),
                &observation_kind(&observation),
                &reference,
            ],
        )
        .await
        .map_err(work_error)?;
    let mut payload: serde_json::Value = serde_json::from_str(&row.get::<_, String>(/*idx*/ 0))
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    if let (Some(cache), Some(cache_use)) = (payload.get_mut("cache"), cache_use) {
        cache["observed"] = crate::provider_evidence::encode_cache_use(cache_use);
        client
            .execute(
                "UPDATE storyos.model_attempts SET payload = $4::text::jsonb
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND model_attempt_id = $3::text::uuid",
                &[
                    &claim.project_scope.owner_user_id.as_ref(),
                    &claim.project_scope.project_id.as_ref(),
                    &dispatch.model_attempt_id,
                    &payload.to_string(),
                ],
            )
            .await
            .map_err(work_error)?;
    }
    let registration = model_registration(&run.model_registration_revision)
        .ok_or_else(|| unavailable("The Model Registration revision is unknown"))?;
    let prior_attempt = payload
        .pointer("/wire/prior_model_attempt_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| unavailable("The summary source is missing"))?;
    let source = load_chain_source(client, claim, prior_attempt).await?;
    let staged = stage_compaction(
        &write::compaction_source(&source),
        SummaryObservation {
            adapter: registration.adapter,
            mapping_revision: payload
                .pointer("/execution_profile/mapping_revision")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(registration.mapping_revision)
                .to_owned(),
            summary,
        },
    );
    write::insert_staged(
        client,
        claim,
        run,
        &source,
        &staged,
        &write::Producer {
            model_attempt_id: dispatch.model_attempt_id.clone(),
            manifest_id: payload
                .get("assembly_manifest_id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| unavailable("The summary manifest is missing"))?
                .to_owned(),
            invocation_id: row.get(/*idx*/ 1),
            usage_kind: match usage {
                ModelUsage::Reported { .. } => "reported",
                ModelUsage::Estimated { .. } => "estimated",
                ModelUsage::Unknown => "unknown",
            },
        },
    )
    .await?;
    Ok(WorkPhase::Hold("compaction"))
}

fn observation_kind(observation: &Observation) -> &'static str {
    match observation {
        Observation::NotSubmitted => "not_submitted",
        Observation::Rejected { .. } => "rejected",
        Observation::Terminal(_) => "terminal",
        Observation::OutcomeUnknown { .. } => "outcome_unknown",
    }
}

#[cfg(test)]
#[path = "agent_run_compaction_summary_tests.rs"]
mod tests;
