use axum::body::to_bytes;
use sha2::{Digest, Sha256};
use storyos_application::{
    AcceptProposalCommand, AcceptProposalError, AcceptProposalSettlementEffect,
    AuthorCommandAdmissionIds, EditorClientBinding, EditorSessionId,
    ProjectCommandChallengeBinding,
};

use super::editor_session::{exact_header, session_binding_ref};
use super::project_command_challenge::{
    hex_bytes, plain_digest, valid_uuid_v7, validate_json_content_type,
};
use super::*;

pub(super) async fn accept_proposal(
    State(state): State<Arc<ServerState>>,
    Path((project_id, proposal_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::AcceptProposalResponse>, ApiError> {
    let (parts, body_stream) = request.into_parts();
    let headers = parts.headers;
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::StateChanging,
    )?;
    validate_json_content_type(&headers)?;
    valid_uuid(&proposal_id)?;
    let bytes = to_bytes(body_stream, contracts::AUTHOR_EDIT_MAX_WIRE_BODY_BYTES)
        .await
        .map_err(|_| payload_too_large())?;
    let body = serde_json::from_slice::<contracts::AcceptProposalRequest>(&bytes)
        .map_err(|_| invalid_request_shape())?;
    let input = &body.accept_proposal_input;
    let session_handle = session_cookie(&headers).ok_or_else(authentication_required)?;
    let session = state
        .client_session_binding(session_handle)
        .ok_or_else(authentication_required)?;
    if body.command_schema != contracts::ACCEPT_PROPOSAL_REQUEST_SCHEMA_ID
        || input.client_contract_revision != session.client_contract_revision
        || input.security_policy_revision != session.security_policy_revision
    {
        return Err(invalid_request());
    }
    valid_uuid(&input.correlation_id)?;
    valid_uuid(&input.proposal_revision_id)?;
    valid_uuid(&input.validation_receipt_id)?;
    if input.selected_operation_ids.is_empty() {
        return Err(invalid_request());
    }
    for operation_id in &input.selected_operation_ids {
        valid_uuid(operation_id)?;
    }
    valid_uuid(&input.expected_authoritative_revision_id)?;
    valid_uuid(&input.editor_session_id)?;
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
    let canonical_command_bytes = canonical_body_bytes(&body)?;
    let digest_hex = hex_bytes(&Sha256::digest(&canonical_command_bytes));
    let canonical_command_digest = format!(
        "sha256:{}:{digest_hex}",
        contracts::ACCEPT_PROPOSAL_DIGEST_PROFILE
    );
    let store = project_reader(&state).await?;
    let command = AcceptProposalCommand {
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
            method: contracts::ACCEPT_PROPOSAL_METHOD.to_owned(),
            route_template: contracts::ACCEPT_PROPOSAL_PATH.to_owned(),
            command_schema: body.command_schema.clone(),
            command_kind: "acceptProposal".to_owned(),
            canonical_command_digest: canonical_command_digest.clone(),
            idempotency_key: idempotency_key.to_owned(),
        },
        nonce_digest: plain_digest(nonce.as_bytes()),
        canonical_command_bytes,
        correlation_id: input.correlation_id.clone(),
        ids: AuthorCommandAdmissionIds {
            command_id: Uuid::now_v7().to_string(),
            author_command_admission_id: Uuid::now_v7().to_string(),
            receipt_id: Uuid::now_v7().to_string(),
        },
        editor_session_id: EditorSessionId::new(input.editor_session_id.clone()),
        proposal_id,
        proposal_revision_id: input.proposal_revision_id.clone(),
        validation_receipt_id: input.validation_receipt_id.clone(),
        selected_operation_ids: input.selected_operation_ids.clone(),
        expected_authoritative_revision_id: input.expected_authoritative_revision_id.clone(),
    };
    let settlement = storyos_application::accept_proposal(&store, &command)
        .await
        .map_err(accept_error)?;
    super::acknowledgement_hold::hold_first_acknowledgement_if_requested(idempotency_key).await;
    accept_response(&command, &digest_hex, settlement)
}

fn accept_response(
    command: &AcceptProposalCommand,
    digest_hex: &str,
    settlement: storyos_application::AcceptProposalSettlement,
) -> Result<Json<contracts::AcceptProposalResponse>, ApiError> {
    let project = settlement.response_project;
    let contract_project_scope = contract_scope(&command.project_scope);
    let (result, effect, prior, resulting, commits) = match settlement.effect {
        AcceptProposalSettlementEffect::Applied {
            author_action_sequence,
            authoritative_commit_id,
            revision_id,
            body,
            blocks,
            project_activity_position,
        } => (
            contracts::AcceptanceReceiptResult::Applied,
            contracts::AcceptProposalEffect::Applied {
                author_action_sequence: author_action_sequence.to_string(),
                authoritative_commit_id: authoritative_commit_id.clone(),
                authoritative_revision: contract_chapter_revision(
                    revision_id.clone(),
                    body,
                    &blocks,
                ),
                project_activity_position: project_activity_position.to_string(),
            },
            vec![command.expected_authoritative_revision_id.clone()],
            vec![revision_id],
            vec![authoritative_commit_id],
        ),
        AcceptProposalSettlementEffect::Invalid { reason } => (
            contracts::AcceptanceReceiptResult::Invalid,
            contracts::AcceptProposalEffect::Invalid {
                reason: match reason {
                    storyos_core::AcceptProposalInvalid::InvalidValidation => {
                        contracts::AcceptProposalInvalidReason::InvalidValidation
                    }
                    storyos_core::AcceptProposalInvalid::AlteredCandidate => {
                        contracts::AcceptProposalInvalidReason::AlteredCandidate
                    }
                },
            },
            vec![command.expected_authoritative_revision_id.clone()],
            vec![command.expected_authoritative_revision_id.clone()],
            Vec::new(),
        ),
        AcceptProposalSettlementEffect::Conflicted { reason } => (
            contracts::AcceptanceReceiptResult::Conflicted,
            contracts::AcceptProposalEffect::Conflicted {
                reason: match reason {
                    storyos_core::AcceptProposalConflict::ChangedHead => {
                        contracts::AcceptProposalConflictReason::ChangedHead
                    }
                },
            },
            vec![command.expected_authoritative_revision_id.clone()],
            vec![command.expected_authoritative_revision_id.clone()],
            Vec::new(),
        ),
        AcceptProposalSettlementEffect::Refused { reason } => (
            contracts::AcceptanceReceiptResult::Refused,
            contracts::AcceptProposalEffect::Refused {
                reason: match reason {
                    storyos_core::AcceptProposalRefusal::WrongScope => {
                        contracts::AcceptProposalRefusalReason::WrongScope
                    }
                    storyos_core::AcceptProposalRefusal::WrongAdmission => {
                        contracts::AcceptProposalRefusalReason::WrongAdmission
                    }
                    storyos_core::AcceptProposalRefusal::StaleProposalRevision => {
                        contracts::AcceptProposalRefusalReason::StaleProposalRevision
                    }
                    storyos_core::AcceptProposalRefusal::NotEligible => {
                        contracts::AcceptProposalRefusalReason::NotEligible
                    }
                    storyos_core::AcceptProposalRefusal::OperationNotPending => {
                        contracts::AcceptProposalRefusalReason::OperationNotPending
                    }
                    storyos_core::AcceptProposalRefusal::DuplicateIdentities => {
                        contracts::AcceptProposalRefusalReason::DuplicateIdentities
                    }
                    storyos_core::AcceptProposalRefusal::MissingRequiredDependencies => {
                        contracts::AcceptProposalRefusalReason::MissingRequiredDependencies
                    }
                    storyos_core::AcceptProposalRefusal::IncompleteBundleClosure => {
                        contracts::AcceptProposalRefusalReason::IncompleteBundleClosure
                    }
                },
            },
            vec![command.expected_authoritative_revision_id.clone()],
            vec![command.expected_authoritative_revision_id.clone()],
            Vec::new(),
        ),
    };
    Ok(Json(contracts::AcceptProposalResponse {
        schema_id: contracts::ACCEPT_PROPOSAL_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: command.correlation_id.clone(),
        project_scope: contract_project_scope.clone(),
        command_id: settlement.ids.command_id,
        author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
        receipt: contracts::AcceptanceReceipt {
            receipt_id: settlement.ids.receipt_id,
            project_scope: contract_project_scope,
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: contracts::ACCEPT_PROPOSAL_DIGEST_PROFILE.to_owned(),
                value_hex_lowercase: digest_hex.to_owned(),
            },
            idempotency_key: command.challenge_binding.idempotency_key.clone(),
            author_command_admission_id: settlement.ids.author_command_admission_id,
            proposal_id: command.proposal_id.clone(),
            proposal_revision_id: command.proposal_revision_id.clone(),
            validation_receipt_id: command.validation_receipt_id.clone(),
            selected_operation_ids: command.selected_operation_ids.clone(),
            prior_authoritative_revision_ids: prior,
            resulting_authoritative_revision_ids: resulting,
            authoritative_commit_ids: commits,
            condition_refs: settlement.condition_refs,
            result,
            created_at: settlement.receipt_created_at,
        },
        project: contracts::ControlledProject {
            project_id: project.project_id.as_ref().to_owned(),
            title: project.title,
            open: match project.current_chapter_id {
                Some(chapter_id) => contracts::ProjectOpenState::CurrentChapter {
                    current_chapter_id: chapter_id.as_ref().to_owned(),
                },
                None => contracts::ProjectOpenState::Empty,
            },
        },
        effect,
    }))
}

fn canonical_body_bytes(body: &contracts::AcceptProposalRequest) -> Result<Vec<u8>, ApiError> {
    let canonical =
        canonical_json(serde_json::to_value(body).map_err(|_| invalid_request_shape())?);
    serde_json::to_vec(&canonical).map_err(|_| invalid_request_shape())
}

fn canonical_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(key, nested)| (key, canonical_json(nested)))
                .collect(),
        ),
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(canonical_json).collect())
        }
        scalar => scalar,
    }
}

fn accept_error(error: AcceptProposalError) -> ApiError {
    match error {
        AcceptProposalError::PreAdmissionRefused { reason } => match reason {
            storyos_application::AcceptanceRefusalReason::InvalidChallenge => problem(
                StatusCode::UNPROCESSABLE_ENTITY,
                "challenge_invalid",
                "The Acceptance challenge is invalid.",
            ),
            storyos_application::AcceptanceRefusalReason::StaleWriter
            | storyos_application::AcceptanceRefusalReason::SessionChanged => problem(
                StatusCode::CONFLICT,
                "acceptance_session_ineligible",
                "This Acceptance session is no longer eligible. Inspect the Proposal and restore writer access before a new attempt.",
            ),
        },
        AcceptProposalError::BindingConflict => problem(
            StatusCode::CONFLICT,
            "idempotency_binding_conflict",
            "The Acceptance binding conflicts.",
        ),
        AcceptProposalError::HistoricalAcknowledgementUnavailable => problem(
            StatusCode::CONFLICT,
            "historical_acknowledgement_unavailable",
            "The original Acceptance acknowledgement cannot be recovered. Refresh to inspect the current Project.",
        ),
        AcceptProposalError::InvalidChallenge => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "challenge_invalid",
            "The Acceptance challenge is invalid.",
        ),
        AcceptProposalError::MissingProject => resource_unavailable(),
        AcceptProposalError::Unavailable(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "project_store_unavailable",
            "The Project store is unavailable.",
        ),
    }
}
