use axum::body::to_bytes;
use sha2::{Digest, Sha256};
use storyos_application::{
    AgentRunControlCommand, AgentRunControlEffect, AgentRunControlError, AgentRunControlIntent,
    AgentRunControlSettlement, AuthorCommandAdmissionIds, EditorClientBinding,
    ProjectCommandChallengeBinding, control_agent_run,
};

use super::editor_session::{exact_header, session_binding_ref};
use super::project_command_challenge::{
    hex_bytes, plain_digest, valid_uuid_v7, validate_json_content_type,
};
use super::*;

pub(super) async fn steer_agent_run(
    State(state): State<Arc<ServerState>>,
    Path((project_id, run_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::SteerAgentRunResponse>, ApiError> {
    let settled = settle_control(
        &state,
        &project_id,
        &run_id,
        request,
        AgentRunControlIntent::Steer,
    )
    .await?;
    let (effect, result) = match &settled.settlement.effect {
        AgentRunControlEffect::Retained {
            run_id,
            steering_input_id,
            input_position,
        } => (
            contracts::SteerAgentRunEffect::Retained {
                run_id: run_id.clone(),
                steering_input_id: steering_input_id.clone(),
                input_position: input_position.to_string(),
            },
            contracts::DomainReceiptResult::NoEffect,
        ),
        AgentRunControlEffect::Conflicted { .. } => (
            contracts::SteerAgentRunEffect::Conflicted {
                reason: contracts::PauseAgentRunConflictReason::TerminalRun,
            },
            contracts::DomainReceiptResult::Conflicted,
        ),
    };
    Ok(Json(contracts::SteerAgentRunResponse {
        schema_id: contracts::STEER_AGENT_RUN_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: settled.correlation_id,
        project_scope: contract_scope(&settled.scope),
        command_id: settled.settlement.ids.command_id.clone(),
        author_command_admission_id: settled.settlement.ids.author_command_admission_id.clone(),
        receipt: control_receipt(
            &settled.scope,
            contracts::DomainReceiptCommandKind::SteerAgentRun,
            contracts::STEER_AGENT_RUN_DIGEST_PROFILE,
            &settled.digest_hex,
            &settled.idempotency_key,
            result,
            &settled.settlement,
        ),
        project: contract_project(&settled.settlement.response_project),
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
        steering_input,
    ) = match intent {
        AgentRunControlIntent::Steer => {
            let body = serde_json::from_slice::<contracts::SteerAgentRunRequest>(&bytes)
                .map_err(|_| invalid_request_shape())?;
            let canonical = canonical_body_bytes(&body)?;
            valid_uuid(&body.steer_agent_run_input.conversation_id)?;
            let input = body.steer_agent_run_input;
            (
                body.command_schema,
                input.correlation_id,
                input.client_contract_revision,
                input.security_policy_revision,
                canonical,
                Some(storyos_application::AgentRunSteeringInput {
                    conversation_id: input.conversation_id,
                    author_message: input.author_message.text,
                }),
            )
        }
    };
    let session_handle = session_cookie(&headers).ok_or_else(authentication_required)?;
    let session = state
        .client_session_binding(session_handle)
        .ok_or_else(authentication_required)?;
    let expected_schema = match intent {
        AgentRunControlIntent::Steer => contracts::STEER_AGENT_RUN_REQUEST_SCHEMA_ID,
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
        AgentRunControlIntent::Steer => (
            "steerAgentRun",
            "POST",
            contracts::STEER_AGENT_RUN_PATH,
            contracts::STEER_AGENT_RUN_DIGEST_PROFILE,
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
            steering_input,
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
        AgentRunControlError::InputLimit => problem(
            StatusCode::PAYLOAD_TOO_LARGE,
            "steering_input_limit",
            "The correction exceeds the admitted Context input limit.",
        ),
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
