use axum::body::to_bytes;
use sha2::{Digest, Sha256};
use storyos_application::{
    AgentRunControlCommand, AgentRunControlConflict, AgentRunControlEffect, AgentRunControlError,
    AgentRunControlIntent, AgentRunControlNoEffect, AgentRunControlSettlement,
    AgentRunControlStatus, AuthorCommandAdmissionIds, EditorClientBinding,
    ProjectCommandChallengeBinding, control_agent_run,
};

use super::editor_session::{exact_header, session_binding_ref};
use super::project_command_challenge::{
    hex_bytes, plain_digest, valid_uuid_v7, validate_json_content_type,
};
use super::*;

pub(super) async fn pause_agent_run(
    State(state): State<Arc<ServerState>>,
    Path((project_id, run_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::PauseAgentRunResponse>, ApiError> {
    let settlement = settle_control(
        &state,
        &project_id,
        &run_id,
        request,
        AgentRunControlIntent::Pause,
    )
    .await?;
    let PauseWire {
        schema_id,
        command_kind,
        digest_profile,
        effect,
        receipt_result,
    } = pause_wire(&settlement)?;
    Ok(Json(contracts::PauseAgentRunResponse {
        schema_id: schema_id.to_owned(),
        correlation_id: settlement.correlation_id,
        project_scope: contract_scope(&settlement.scope),
        command_id: settlement.settlement.ids.command_id.clone(),
        author_command_admission_id: settlement
            .settlement
            .ids
            .author_command_admission_id
            .clone(),
        receipt: control_receipt(
            &settlement.scope,
            command_kind,
            digest_profile,
            &settlement.digest_hex,
            &settlement.idempotency_key,
            receipt_result,
            &settlement.settlement,
        ),
        project: contract_project(&settlement.settlement.response_project),
        effect,
    }))
}

pub(super) async fn cancel_agent_run(
    State(state): State<Arc<ServerState>>,
    Path((project_id, run_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::CancelAgentRunResponse>, ApiError> {
    let settlement = settle_control(
        &state,
        &project_id,
        &run_id,
        request,
        AgentRunControlIntent::Cancel,
    )
    .await?;
    let CancelWire {
        schema_id,
        command_kind,
        digest_profile,
        effect,
        receipt_result,
    } = cancel_wire(&settlement)?;
    Ok(Json(contracts::CancelAgentRunResponse {
        schema_id: schema_id.to_owned(),
        correlation_id: settlement.correlation_id,
        project_scope: contract_scope(&settlement.scope),
        command_id: settlement.settlement.ids.command_id.clone(),
        author_command_admission_id: settlement
            .settlement
            .ids
            .author_command_admission_id
            .clone(),
        receipt: control_receipt(
            &settlement.scope,
            command_kind,
            digest_profile,
            &settlement.digest_hex,
            &settlement.idempotency_key,
            receipt_result,
            &settlement.settlement,
        ),
        project: contract_project(&settlement.settlement.response_project),
        effect,
    }))
}

struct SettledControl {
    scope: ApplicationScope,
    correlation_id: String,
    digest_hex: String,
    idempotency_key: String,
    settlement: AgentRunControlSettlement,
}

struct PauseWire {
    schema_id: &'static str,
    command_kind: contracts::DomainReceiptCommandKind,
    digest_profile: &'static str,
    effect: contracts::PauseAgentRunEffect,
    receipt_result: contracts::DomainReceiptResult,
}

struct CancelWire {
    schema_id: &'static str,
    command_kind: contracts::DomainReceiptCommandKind,
    digest_profile: &'static str,
    effect: contracts::CancelAgentRunEffect,
    receipt_result: contracts::DomainReceiptResult,
}

async fn settle_control(
    state: &ServerState,
    project_id: &str,
    run_id: &str,
    request: Request,
    intent: AgentRunControlIntent,
) -> Result<SettledControl, ApiError> {
    let (parts, body_stream) = request.into_parts();
    let headers = parts.headers;
    let scope = authenticate_scope(
        state,
        &headers,
        project_id,
        RequestOriginPolicy::StateChanging,
    )?;
    validate_json_content_type(&headers)?;
    valid_uuid(run_id)?;
    let bytes = to_bytes(body_stream, contracts::AUTHOR_EDIT_MAX_WIRE_BODY_BYTES)
        .await
        .map_err(|_| payload_too_large())?;
    let (
        command_schema,
        correlation_id,
        client_contract_revision,
        security_policy_revision,
        canonical_command_bytes,
    ) = match intent {
        AgentRunControlIntent::Pause => {
            let body = serde_json::from_slice::<contracts::PauseAgentRunRequest>(&bytes)
                .map_err(|_| invalid_request_shape())?;
            let canonical_command_bytes = canonical_body_bytes(&body)?;
            (
                body.command_schema,
                body.pause_agent_run_input.correlation_id,
                body.pause_agent_run_input.client_contract_revision,
                body.pause_agent_run_input.security_policy_revision,
                canonical_command_bytes,
            )
        }
        AgentRunControlIntent::Cancel => {
            let body = serde_json::from_slice::<contracts::CancelAgentRunRequest>(&bytes)
                .map_err(|_| invalid_request_shape())?;
            let canonical_command_bytes = canonical_body_bytes(&body)?;
            (
                body.command_schema,
                body.cancel_agent_run_input.correlation_id,
                body.cancel_agent_run_input.client_contract_revision,
                body.cancel_agent_run_input.security_policy_revision,
                canonical_command_bytes,
            )
        }
    };
    let session_handle = session_cookie(&headers).ok_or_else(authentication_required)?;
    let session = state
        .client_session_binding(session_handle)
        .ok_or_else(authentication_required)?;
    let expected_schema = match intent {
        AgentRunControlIntent::Pause => contracts::PAUSE_AGENT_RUN_REQUEST_SCHEMA_ID,
        AgentRunControlIntent::Cancel => contracts::CANCEL_AGENT_RUN_REQUEST_SCHEMA_ID,
    };
    if command_schema != expected_schema
        || client_contract_revision != session.client_contract_revision
        || security_policy_revision != session.security_policy_revision
    {
        return Err(invalid_request());
    }
    valid_uuid(&correlation_id)?;
    let idempotency_key = exact_header(&headers, "idempotency-key")?;
    let nonce = exact_header(&headers, "x-storyos-anti-forgery")?;
    if !valid_uuid_v7(idempotency_key)
        || nonce.len() != 64
        || !nonce
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(invalid_request());
    }
    let secret = state
        .config
        .project_command_challenge_secret
        .as_deref()
        .filter(|secret| secret.len() >= 32)
        .ok_or_else(challenge_store_unavailable)?;
    let binding_ref = session_binding_ref(secret, session_handle);
    let digest_hex = hex_bytes(&Sha256::digest(&canonical_command_bytes));
    let (command_kind, method, route, digest_profile) = match intent {
        AgentRunControlIntent::Pause => (
            "pauseAgentRun",
            contracts::PAUSE_AGENT_RUN_METHOD,
            contracts::PAUSE_AGENT_RUN_PATH,
            contracts::PAUSE_AGENT_RUN_DIGEST_PROFILE,
        ),
        AgentRunControlIntent::Cancel => (
            "cancelAgentRun",
            contracts::CANCEL_AGENT_RUN_METHOD,
            contracts::CANCEL_AGENT_RUN_PATH,
            contracts::CANCEL_AGENT_RUN_DIGEST_PROFILE,
        ),
    };
    let store = project_reader(state).await?;
    let settlement = control_agent_run(
        &store,
        &AgentRunControlCommand {
            project_scope: scope.clone(),
            client_binding: EditorClientBinding {
                binding_ref: binding_ref.clone(),
                session_generation: session.session_generation,
                client_contract_revision: session.client_contract_revision.clone(),
                security_policy_revision: session.security_policy_revision.clone(),
            },
            challenge_binding: ProjectCommandChallengeBinding {
                project_scope: scope.clone(),
                client_session_binding_digest: binding_ref,
                client_session_generation: session.session_generation,
                client_contract_revision: session.client_contract_revision.clone(),
                security_policy_revision: session.security_policy_revision.clone(),
                limit_profile_revision: contracts::LIMIT_PROFILE_REVISION.to_owned(),
                challenge_rate_policy_revision:
                    storyos_application::PROJECT_COMMAND_CHALLENGE_RATE_POLICY_REVISION.to_owned(),
                method: method.to_owned(),
                route_template: route.to_owned(),
                command_schema,
                command_kind: command_kind.to_owned(),
                canonical_command_digest: format!("sha256:{digest_profile}:{digest_hex}"),
                idempotency_key: idempotency_key.to_owned(),
            },
            nonce_digest: plain_digest(nonce.as_bytes()),
            canonical_command_bytes,
            correlation_id: correlation_id.clone(),
            run_id: run_id.to_owned(),
            ids: AuthorCommandAdmissionIds {
                command_id: Uuid::now_v7().to_string(),
                author_command_admission_id: Uuid::now_v7().to_string(),
                receipt_id: Uuid::now_v7().to_string(),
            },
            intent,
        },
    )
    .await
    .map_err(control_error)?;
    Ok(SettledControl {
        scope,
        correlation_id,
        digest_hex,
        idempotency_key: idempotency_key.to_owned(),
        settlement,
    })
}

fn pause_wire(settled: &SettledControl) -> Result<PauseWire, ApiError> {
    let (receipt_result, effect) = match &settled.settlement.effect {
        AgentRunControlEffect::Applied {
            run_id,
            status,
            fence_generation,
        } => (
            contracts::DomainReceiptResult::AuthoritativeApplied,
            contracts::PauseAgentRunEffect::Applied {
                run_id: run_id.clone(),
                status: contract_status(*status)?,
                fence_generation: fence_generation.to_string(),
                project_activity_position: settled.settlement.project_activity_position.to_string(),
            },
        ),
        AgentRunControlEffect::NoEffect {
            reason: AgentRunControlNoEffect::AlreadyPaused,
        } => (
            contracts::DomainReceiptResult::NoEffect,
            contracts::PauseAgentRunEffect::NoEffect {
                reason: contracts::PauseAgentRunNoEffectReason::AlreadyPaused,
            },
        ),
        AgentRunControlEffect::Conflicted {
            reason: AgentRunControlConflict::TerminalRun,
        } => (
            contracts::DomainReceiptResult::Conflicted,
            contracts::PauseAgentRunEffect::Conflicted {
                reason: contracts::PauseAgentRunConflictReason::TerminalRun,
            },
        ),
        AgentRunControlEffect::NoEffect {
            reason: AgentRunControlNoEffect::AlreadyCancelled,
        } => {
            return Err(control_error(AgentRunControlError::BindingConflict));
        }
    };
    Ok(PauseWire {
        schema_id: contracts::PAUSE_AGENT_RUN_RESPONSE_SCHEMA_ID,
        command_kind: contracts::DomainReceiptCommandKind::PauseAgentRun,
        digest_profile: contracts::PAUSE_AGENT_RUN_DIGEST_PROFILE,
        effect,
        receipt_result,
    })
}

fn cancel_wire(settled: &SettledControl) -> Result<CancelWire, ApiError> {
    let (receipt_result, effect) = match &settled.settlement.effect {
        AgentRunControlEffect::Applied {
            run_id,
            status,
            fence_generation,
        } => (
            contracts::DomainReceiptResult::AuthoritativeApplied,
            contracts::CancelAgentRunEffect::Applied {
                run_id: run_id.clone(),
                status: contract_status(*status)?,
                fence_generation: fence_generation.to_string(),
                project_activity_position: settled.settlement.project_activity_position.to_string(),
            },
        ),
        AgentRunControlEffect::NoEffect {
            reason: AgentRunControlNoEffect::AlreadyCancelled,
        } => (
            contracts::DomainReceiptResult::NoEffect,
            contracts::CancelAgentRunEffect::NoEffect {
                reason: contracts::CancelAgentRunNoEffectReason::AlreadyCancelled,
            },
        ),
        AgentRunControlEffect::Conflicted {
            reason: AgentRunControlConflict::TerminalRun,
        } => (
            contracts::DomainReceiptResult::Conflicted,
            contracts::CancelAgentRunEffect::Conflicted {
                reason: contracts::CancelAgentRunConflictReason::TerminalRun,
            },
        ),
        AgentRunControlEffect::NoEffect {
            reason: AgentRunControlNoEffect::AlreadyPaused,
        } => {
            return Err(control_error(AgentRunControlError::BindingConflict));
        }
    };
    Ok(CancelWire {
        schema_id: contracts::CANCEL_AGENT_RUN_RESPONSE_SCHEMA_ID,
        command_kind: contracts::DomainReceiptCommandKind::CancelAgentRun,
        digest_profile: contracts::CANCEL_AGENT_RUN_DIGEST_PROFILE,
        effect,
        receipt_result,
    })
}

fn control_receipt(
    scope: &ApplicationScope,
    command_kind: contracts::DomainReceiptCommandKind,
    digest_profile: &str,
    digest_hex: &str,
    idempotency_key: &str,
    result: contracts::DomainReceiptResult,
    settlement: &AgentRunControlSettlement,
) -> contracts::DomainReceipt {
    contracts::DomainReceipt {
        receipt_id: settlement.ids.receipt_id.clone(),
        project_scope: contract_scope(scope),
        command_kind,
        command_digest: contracts::DigestValue {
            algorithm: contracts::DigestAlgorithm::Sha256,
            profile: digest_profile.to_owned(),
            value_hex_lowercase: digest_hex.to_owned(),
        },
        idempotency_key: idempotency_key.to_owned(),
        producer_cause: contracts::DomainReceiptProducerCause::AuthorCommandAdmission,
        author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
        expected_heads: Vec::new(),
        prior_heads: Vec::new(),
        resulting_heads: Vec::new(),
        authoritative_revision_ids: Vec::new(),
        proposal_revision_ids: Vec::new(),
        authoritative_commit_ids: Vec::new(),
        author_action_sequence: None,
        draft_artifact_refs: Vec::new(),
        artifact_lifecycle_event_refs: Vec::new(),
        condition_refs: Vec::new(),
        result,
        created_at: settlement.receipt_created_at.clone(),
    }
}

fn contract_project(project: &storyos_application::Project) -> contracts::ControlledProject {
    contracts::ControlledProject {
        project_id: project.project_id.as_ref().to_owned(),
        title: project.title.clone(),
        open: match &project.current_chapter_id {
            Some(chapter_id) => contracts::ProjectOpenState::CurrentChapter {
                current_chapter_id: chapter_id.as_ref().to_owned(),
            },
            None => contracts::ProjectOpenState::Empty,
        },
    }
}

fn contract_status(status: AgentRunControlStatus) -> Result<contracts::AgentRunStatus, ApiError> {
    match status {
        AgentRunControlStatus::Paused => Ok(contracts::AgentRunStatus::Paused),
        AgentRunControlStatus::Cancelled => Ok(contracts::AgentRunStatus::Cancelled),
    }
}

fn canonical_body_bytes<T: serde::Serialize>(body: &T) -> Result<Vec<u8>, ApiError> {
    let canonical =
        canonical_json(serde_json::to_value(body).map_err(|_| invalid_request_shape())?);
    serde_json::to_vec(&canonical).map_err(|_| invalid_request_shape())
}

fn canonical_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(canonical_json).collect())
        }
        serde_json::Value::Object(values) => serde_json::Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, canonical_json(value)))
                .collect(),
        ),
        scalar => scalar,
    }
}

fn control_error(error: AgentRunControlError) -> ApiError {
    match error {
        AgentRunControlError::BindingConflict => problem(
            StatusCode::CONFLICT,
            "idempotency_binding_conflict",
            "The AgentRun control binding conflicts.",
        ),
        AgentRunControlError::HistoricalAcknowledgementUnavailable => problem(
            StatusCode::CONFLICT,
            "historical_acknowledgement_unavailable",
            "The original AgentRun control acknowledgement cannot be recovered. Refresh to inspect the current Project.",
        ),
        AgentRunControlError::InvalidChallenge => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "challenge_invalid",
            "The AgentRun control challenge is invalid.",
        ),
        AgentRunControlError::MissingProject | AgentRunControlError::MissingRun => {
            resource_unavailable()
        }
        AgentRunControlError::Unavailable(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "project_store_unavailable",
            "The Project store is unavailable.",
        ),
    }
}
