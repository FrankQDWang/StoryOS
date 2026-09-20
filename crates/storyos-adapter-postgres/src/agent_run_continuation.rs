use storyos_application::{
    AgentRunContinuationAdmission, AgentRunInputMapping, ClaimedAgentRun, CompleteAgentRunError,
    ProjectAssistanceRecord,
};
use storyos_core::{
    ContinuationIdentity, ContinuationInputMapping, ContinuationMappingInput,
    HOST_FAKE_MAPPING_REVISION, continuation_mapping_can_represent, map_continuation_input,
};

pub(crate) struct ContinuationWire {
    pub mapping: ContinuationInputMapping,
    pub prior_binding_id: Option<String>,
    pub known_prior_binding_id: Option<String>,
    pub admission: ContinuationIdentity,
}

pub(crate) async fn decide_continuation(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    conversation_id: &str,
    author_message: &str,
    assistance: &ProjectAssistanceRecord,
) -> Result<ContinuationWire, CompleteAgentRunError> {
    let admission = current_identity(claim, conversation_id, assistance);
    let prior = load_prior_binding(client, claim, conversation_id).await?;
    let known_prior_binding_id = prior.as_ref().map(|loaded| loaded.binding_id.clone());
    let mapping = map_continuation_input(&ContinuationMappingInput {
        current: admission.clone(),
        prior: prior.as_ref().map(|loaded| loaded.identity.clone()),
        mapping_can_represent: continuation_mapping_can_represent(author_message),
    });
    let prior_binding_id = match mapping {
        ContinuationInputMapping::Incremental => known_prior_binding_id.clone(),
        ContinuationInputMapping::None
        | ContinuationInputMapping::Full
        | ContinuationInputMapping::NewTransport => None,
    };
    Ok(ContinuationWire {
        mapping,
        prior_binding_id,
        known_prior_binding_id,
        admission,
    })
}

pub(crate) fn encode_wire(wire: &ContinuationWire) -> serde_json::Value {
    serde_json::json!({
        "input_mapping": mapping_name(wire.mapping),
        "prior_continuation_binding_id": wire.prior_binding_id,
        "known_prior_continuation_binding_id": wire.known_prior_binding_id,
        "admission": identity_value(&wire.admission)
    })
}

pub(crate) fn parse_wire(payload: &serde_json::Value) -> Option<ContinuationWire> {
    let continuation = payload.get("continuation")?;
    let admission = parse_identity(continuation.get("admission")?)?;
    Some(ContinuationWire {
        mapping: parse_mapping(
            continuation
                .get("input_mapping")
                .and_then(serde_json::Value::as_str),
        ),
        prior_binding_id: string_field(continuation, "prior_continuation_binding_id"),
        known_prior_binding_id: string_field(continuation, "known_prior_continuation_binding_id"),
        admission,
    })
}

pub(crate) fn encode_produced_binding(
    binding_id: &str,
    attempt_id: &str,
    identity: &ContinuationIdentity,
) -> serde_json::Value {
    let mut value = identity_value(identity);
    value["continuation_binding_id"] = serde_json::json!(binding_id);
    value["original_attempt_id"] = serde_json::json!(attempt_id);
    value
}

pub(crate) fn inspect_mapping(mapping: ContinuationInputMapping) -> AgentRunInputMapping {
    match mapping {
        ContinuationInputMapping::None => AgentRunInputMapping::None,
        ContinuationInputMapping::Incremental => AgentRunInputMapping::Incremental,
        ContinuationInputMapping::Full => AgentRunInputMapping::Full,
        ContinuationInputMapping::NewTransport => AgentRunInputMapping::NewTransport,
    }
}

pub(crate) fn inspect_admission(identity: &ContinuationIdentity) -> AgentRunContinuationAdmission {
    AgentRunContinuationAdmission {
        processing_destination_identity: identity.destination_identity.clone(),
        evidence_revision: identity.evidence_revision.clone(),
        model_registration_revision: identity.registration.clone(),
        adapter_mapping: identity.adapter_mapping.clone(),
        project_model_use_binding_revision: identity.use_binding.clone(),
        external_compatibility_decision: identity.compatibility.clone(),
    }
}

pub(crate) fn default_inspect_admission() -> AgentRunContinuationAdmission {
    AgentRunContinuationAdmission {
        processing_destination_identity: String::new(),
        evidence_revision: String::new(),
        model_registration_revision: String::new(),
        adapter_mapping: String::new(),
        project_model_use_binding_revision: String::new(),
        external_compatibility_decision: String::new(),
    }
}

fn current_identity(
    claim: &ClaimedAgentRun,
    conversation_id: &str,
    assistance: &ProjectAssistanceRecord,
) -> ContinuationIdentity {
    ContinuationIdentity {
        owner_user_id: claim.project_scope.owner_user_id.as_ref().to_owned(),
        project_id: claim.project_scope.project_id.as_ref().to_owned(),
        conversation_id: conversation_id.to_owned(),
        destination_identity: assistance.processing_destination_identity.clone(),
        evidence_revision: assistance
            .processing_destination_identity_evidence_revision
            .to_string(),
        registration: assistance.model_registration_revision.clone(),
        adapter_mapping: HOST_FAKE_MAPPING_REVISION.to_owned(),
        use_binding: assistance.project_model_use_binding_revision.clone(),
        compatibility: assistance.external_compatibility_decision.clone(),
        covered_copy_restricted: false,
    }
}

struct LoadedPrior {
    binding_id: String,
    identity: ContinuationIdentity,
}

async fn load_prior_binding(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    conversation_id: &str,
) -> Result<Option<LoadedPrior>, CompleteAgentRunError> {
    let row = client
        .query_opt(
            "SELECT attempt.continuation_binding_id::text, attempt.payload::text
               FROM storyos.model_attempts AS attempt
               JOIN storyos.agent_runs AS run
                 ON (run.owner_user_id, run.project_id, run.run_id) =
                    (attempt.owner_user_id, attempt.project_id, attempt.run_id)
              WHERE attempt.owner_user_id = $1::text::uuid
                AND attempt.project_id = $2::text::uuid
                AND attempt.conversation_id = $3::text::uuid
                AND attempt.continuation_binding_id IS NOT NULL
                AND attempt.dispatch_state = 'settled'
                AND run.status = 'completed' AND run.run_id <> $4::text::uuid
              ORDER BY run.run_id DESC LIMIT 1",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &conversation_id,
                &claim.run_id,
            ],
        )
        .await
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    let Some(row) = row else {
        return Ok(None);
    };
    let binding_id: String = row.get(0);
    let payload: serde_json::Value = serde_json::from_str(&row.get::<_, String>(1))
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    let Some(identity) = payload.get("produced_binding").and_then(parse_identity) else {
        return Ok(None);
    };
    Ok(Some(LoadedPrior {
        binding_id,
        identity,
    }))
}

fn identity_value(identity: &ContinuationIdentity) -> serde_json::Value {
    serde_json::json!({
        "owner_user_id": identity.owner_user_id,
        "project_id": identity.project_id,
        "conversation_id": identity.conversation_id,
        "processing_destination_identity": identity.destination_identity,
        "evidence_revision": identity.evidence_revision,
        "model_registration_revision": identity.registration,
        "adapter_mapping": identity.adapter_mapping,
        "project_model_use_binding_revision": identity.use_binding,
        "external_compatibility_decision": identity.compatibility,
        "covered_copy_restricted": identity.covered_copy_restricted
    })
}

fn parse_identity(value: &serde_json::Value) -> Option<ContinuationIdentity> {
    Some(ContinuationIdentity {
        owner_user_id: string_field(value, "owner_user_id")?,
        project_id: string_field(value, "project_id")?,
        conversation_id: string_field(value, "conversation_id")?,
        destination_identity: string_field(value, "processing_destination_identity")?,
        evidence_revision: string_field(value, "evidence_revision")?,
        registration: string_field(value, "model_registration_revision")?,
        adapter_mapping: string_field(value, "adapter_mapping")?,
        use_binding: string_field(value, "project_model_use_binding_revision")?,
        compatibility: string_field(value, "external_compatibility_decision")?,
        covered_copy_restricted: value
            .get("covered_copy_restricted")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    })
}

fn parse_mapping(name: Option<&str>) -> ContinuationInputMapping {
    match name {
        Some("incremental") => ContinuationInputMapping::Incremental,
        Some("full") => ContinuationInputMapping::Full,
        Some("new_transport") => ContinuationInputMapping::NewTransport,
        _ => ContinuationInputMapping::None,
    }
}

fn mapping_name(mapping: ContinuationInputMapping) -> &'static str {
    match mapping {
        ContinuationInputMapping::None => "none",
        ContinuationInputMapping::Incremental => "incremental",
        ContinuationInputMapping::Full => "full",
        ContinuationInputMapping::NewTransport => "new_transport",
    }
}

fn string_field(value: &serde_json::Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}
