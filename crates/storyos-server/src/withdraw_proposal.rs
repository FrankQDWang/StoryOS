use axum::body::to_bytes;
use sha2::{Digest, Sha256};
use storyos_application::{
    AuthorCommandAdmissionIds, EditorClientBinding, EditorSessionId,
    ProjectCommandChallengeBinding, ResolvedWithdrawal, WithdrawProposalCommand,
    WithdrawProposalError, WithdrawProposalSettlementEffect, WithdrawalActor, WithdrawalNote,
};

use super::editor_session::{exact_header, session_binding_ref};
use super::project_command_challenge::{
    hex_bytes, plain_digest, valid_uuid_v7, validate_json_content_type,
};
use super::*;

pub(super) async fn withdraw_proposal(
    State(state): State<Arc<ServerState>>,
    Path((project_id, proposal_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::WithdrawProposalResponse>, ApiError> {
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
    let body = serde_json::from_slice::<contracts::WithdrawProposalRequest>(&bytes)
        .map_err(|_| invalid_request_shape())?;
    let session_handle = session_cookie(&headers).ok_or_else(authentication_required)?;
    let session = state
        .client_session_binding(session_handle)
        .ok_or_else(authentication_required)?;
    let (
        proposal_revision_id,
        expected_closure,
        expected_target_revisions,
        correlation_id,
        client_contract_revision,
        security_policy_revision,
        withdrawal_note,
        editor_session_id,
        actor,
        require_anti_forgery,
    ) = match &body.withdraw_proposal_input {
        contracts::WithdrawProposalInput::Author {
            proposal_revision_id,
            expected_closure,
            expected_target_revisions,
            withdrawal_reason,
            editor_session_id,
            client_contract_revision,
            security_policy_revision,
            correlation_id,
        } => {
            let withdrawal_note = match withdrawal_reason {
                contracts::AuthorWithdrawalReason::AuthorWithdrew { note } => match note {
                    contracts::BoundedAuthorNote::Omitted => WithdrawalNote::Omitted,
                    contracts::BoundedAuthorNote::Present { text } => {
                        if text.is_empty() {
                            return Err(invalid_request());
                        }
                        WithdrawalNote::Present { text: text.clone() }
                    }
                },
            };
            valid_uuid(editor_session_id)?;
            (
                proposal_revision_id.clone(),
                expected_closure.clone(),
                expected_target_revisions.clone(),
                correlation_id.clone(),
                client_contract_revision.clone(),
                security_policy_revision.clone(),
                withdrawal_note,
                EditorSessionId::new(editor_session_id.clone()),
                WithdrawalActor::Author,
                true,
            )
        }
        contracts::WithdrawProposalInput::CurrentProducer {
            producer:
                contracts::AgentRunDecisionProducer {
                    kind: contracts::AgentRunDecisionKind::AgentRunDecision,
                    run_id,
                    decision_id,
                },
            proposal_revision_id,
            expected_closure,
            expected_target_revisions,
            withdrawal_reason: contracts::CurrentProducerWithdrawalReason::CurrentProducerWithdrew,
            client_contract_revision,
            security_policy_revision,
            correlation_id,
        } => {
            valid_uuid(run_id)?;
            valid_uuid(decision_id)?;
            (
                proposal_revision_id.clone(),
                expected_closure.clone(),
                expected_target_revisions.clone(),
                correlation_id.clone(),
                client_contract_revision.clone(),
                security_policy_revision.clone(),
                WithdrawalNote::Omitted,
                EditorSessionId::new(String::new()),
                WithdrawalActor::CurrentProducer {
                    run_id: run_id.clone(),
                    decision_id: decision_id.clone(),
                },
                false,
            )
        }
    };
    if body.command_schema != contracts::WITHDRAW_PROPOSAL_REQUEST_SCHEMA_ID
        || client_contract_revision != session.client_contract_revision
        || security_policy_revision != session.security_policy_revision
        || expected_closure != "open"
        || expected_target_revisions.len() != 1
    {
        return Err(invalid_request());
    }
    let expected_authoritative_revision_id = expected_target_revisions[0].clone();
    valid_uuid(&correlation_id)?;
    valid_uuid(&proposal_revision_id)?;
    valid_uuid(&expected_authoritative_revision_id)?;
    let idempotency_key = exact_header(&headers, "idempotency-key")?;
    let nonce = if require_anti_forgery {
        exact_header(&headers, "x-storyos-anti-forgery")?.to_owned()
    } else {
        String::new()
    };
    if !valid_uuid_v7(idempotency_key)
        || (require_anti_forgery
            && (nonce.len() != 64
                || !nonce
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())))
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
        contracts::WITHDRAW_PROPOSAL_DIGEST_PROFILE
    );
    let store = project_reader(&state).await?;
    let command = WithdrawProposalCommand {
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
            method: contracts::WITHDRAW_PROPOSAL_METHOD.to_owned(),
            route_template: contracts::WITHDRAW_PROPOSAL_PATH.to_owned(),
            command_schema: body.command_schema.clone(),
            command_kind: "withdrawProposal".to_owned(),
            canonical_command_digest: canonical_command_digest.clone(),
            idempotency_key: idempotency_key.to_owned(),
        },
        nonce_digest: plain_digest(nonce.as_bytes()),
        canonical_command_bytes,
        correlation_id,
        ids: AuthorCommandAdmissionIds {
            command_id: Uuid::now_v7().to_string(),
            author_command_admission_id: Uuid::now_v7().to_string(),
            receipt_id: Uuid::now_v7().to_string(),
        },
        editor_session_id,
        proposal_id,
        proposal_revision_id,
        expected_closure,
        expected_authoritative_revision_id,
        withdrawal_note,
        actor,
    };
    let settlement = storyos_application::withdraw_proposal(&store, &command)
        .await
        .map_err(withdraw_error)?;
    super::acknowledgement_hold::hold_first_acknowledgement_if_requested(idempotency_key).await;
    withdraw_response(&command, &digest_hex, settlement)
}

fn withdraw_response(
    command: &WithdrawProposalCommand,
    digest_hex: &str,
    settlement: storyos_application::WithdrawProposalSettlement,
) -> Result<Json<contracts::WithdrawProposalResponse>, ApiError> {
    let project = settlement.response_project;
    let contract_project_scope = contract_scope(&command.project_scope);
    let author_command_admission_id = match &command.actor {
        WithdrawalActor::Author => Some(settlement.ids.author_command_admission_id.clone()),
        WithdrawalActor::CurrentProducer { .. } => None,
    };
    let (result, effect) = match settlement.effect {
        WithdrawProposalSettlementEffect::Resolved {
            ownership,
            preserved_generation,
            preserved_validation,
            withdrawal_event_id,
        } => {
            let (author_action_sequence, undo_disposition, withdrawal_reason) = match ownership {
                ResolvedWithdrawal::Author {
                    author_action_sequence,
                    withdrawal_note,
                } => (
                    Some(author_action_sequence.to_string()),
                    Some(contracts::AuthorUndoDisposition::Forward),
                    contracts::ProposalWithdrawalReason::AuthorWithdrew {
                        note: match withdrawal_note {
                            WithdrawalNote::Omitted => contracts::BoundedAuthorNote::Omitted,
                            WithdrawalNote::Present { text } => {
                                contracts::BoundedAuthorNote::Present { text }
                            }
                        },
                    },
                ),
                ResolvedWithdrawal::CurrentProducer => (
                    None,
                    None,
                    contracts::ProposalWithdrawalReason::CurrentProducerWithdrew,
                ),
            };
            (
                contracts::WithdrawalReceiptResult::Resolved,
                contracts::WithdrawProposalEffect::Resolved {
                    author_action_sequence,
                    undo_disposition,
                    preserved_generation,
                    preserved_validation,
                    prior_closure: "open".to_owned(),
                    resulting_closure: "withdrawn".to_owned(),
                    withdrawal_reason,
                    closure_event_refs: vec![withdrawal_event_id],
                },
            )
        }
        WithdrawProposalSettlementEffect::Conflicted { reason } => (
            contracts::WithdrawalReceiptResult::Conflicted,
            contracts::WithdrawProposalEffect::Conflicted {
                reason: match reason {
                    storyos_core::WithdrawProposalConflict::ChangedHead => {
                        contracts::WithdrawProposalConflictReason::ChangedHead
                    }
                },
            },
        ),
        WithdrawProposalSettlementEffect::Refused { reason } => (
            contracts::WithdrawalReceiptResult::Refused,
            contracts::WithdrawProposalEffect::Refused {
                reason: match reason {
                    storyos_core::WithdrawProposalRefusal::WrongScope => {
                        contracts::WithdrawProposalRefusalReason::WrongScope
                    }
                    storyos_core::WithdrawProposalRefusal::WrongAdmission => {
                        contracts::WithdrawProposalRefusalReason::WrongAdmission
                    }
                    storyos_core::WithdrawProposalRefusal::StaleProposalRevision => {
                        contracts::WithdrawProposalRefusalReason::StaleProposalRevision
                    }
                },
            },
        ),
        WithdrawProposalSettlementEffect::NoEffect { reason } => (
            contracts::WithdrawalReceiptResult::NoEffect,
            contracts::WithdrawProposalEffect::NoEffect {
                reason: match reason {
                    storyos_core::WithdrawProposalNoEffect::UnsupportedCause => {
                        contracts::WithdrawProposalNoEffectReason::UnsupportedCause
                    }
                    storyos_core::WithdrawProposalNoEffect::TerminalSupersession => {
                        contracts::WithdrawProposalNoEffectReason::TerminalSupersession
                    }
                    storyos_core::WithdrawProposalNoEffect::ClosureNotOpen => {
                        contracts::WithdrawProposalNoEffectReason::ClosureNotOpen
                    }
                },
            },
        ),
    };
    Ok(Json(contracts::WithdrawProposalResponse {
        schema_id: contracts::WITHDRAW_PROPOSAL_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: command.correlation_id.clone(),
        project_scope: contract_project_scope.clone(),
        command_id: settlement.ids.command_id,
        author_command_admission_id: author_command_admission_id.clone(),
        receipt: contracts::WithdrawalReceipt {
            receipt_id: settlement.ids.receipt_id,
            project_scope: contract_project_scope,
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: contracts::WITHDRAW_PROPOSAL_DIGEST_PROFILE.to_owned(),
                value_hex_lowercase: digest_hex.to_owned(),
            },
            idempotency_key: command.challenge_binding.idempotency_key.clone(),
            author_command_admission_id,
            proposal_id: command.proposal_id.clone(),
            proposal_revision_id: command.proposal_revision_id.clone(),
            expected_target_revisions: vec![command.expected_authoritative_revision_id.clone()],
            prior_authoritative_revision_ids: vec![
                command.expected_authoritative_revision_id.clone(),
            ],
            resulting_authoritative_revision_ids: vec![
                command.expected_authoritative_revision_id.clone(),
            ],
            authoritative_commit_ids: Vec::new(),
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

fn canonical_body_bytes(body: &contracts::WithdrawProposalRequest) -> Result<Vec<u8>, ApiError> {
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

fn withdraw_error(error: WithdrawProposalError) -> ApiError {
    match error {
        WithdrawProposalError::BindingConflict => problem(
            StatusCode::CONFLICT,
            "idempotency_binding_conflict",
            "The Withdrawal binding conflicts.",
        ),
        WithdrawProposalError::HistoricalAcknowledgementUnavailable => problem(
            StatusCode::CONFLICT,
            "historical_acknowledgement_unavailable",
            "The original Withdrawal acknowledgement cannot be recovered. Refresh to inspect the current Project.",
        ),
        WithdrawProposalError::InvalidChallenge => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "challenge_invalid",
            "The Withdrawal challenge is invalid.",
        ),
        WithdrawProposalError::MissingProject => resource_unavailable(),
        WithdrawProposalError::Unavailable(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "project_store_unavailable",
            "The Project store is unavailable.",
        ),
    }
}
