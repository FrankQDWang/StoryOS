use axum::body::to_bytes;
use sha2::{Digest, Sha256};
use storyos_application::{
    ApplyAuthorEditCommand, AuthorCommandAdmissionIds, AuthorEditError, AuthorEditSettlementEffect,
    EditorClientBinding, EditorSessionId, ProjectCommandChallengeBinding,
};

use super::editor_session::{exact_header, session_binding_ref};
use super::project_command_challenge::{plain_digest, valid_uuid_v7, validate_json_content_type};
use super::*;

pub(super) async fn apply_author_edit(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    request: Request,
) -> Result<Json<contracts::ApplyAuthorEditResponse>, ApiError> {
    let (parts, body_stream) = request.into_parts();
    let headers = parts.headers;
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::StateChanging,
    )?;
    validate_json_content_type(&headers)?;
    let bytes = to_bytes(body_stream, contracts::AUTHOR_EDIT_MAX_WIRE_BODY_BYTES)
        .await
        .map_err(|_| payload_too_large())?;
    let body = serde_json::from_slice::<contracts::ApplyAuthorEditRequest>(&bytes)
        .map_err(|_| invalid_request_shape())?;
    validate_request(&body)?;
    let session_handle = session_cookie(&headers).ok_or_else(authentication_required)?;
    let session_binding = state
        .config
        .session_bindings
        .get(session_handle)
        .ok_or_else(authentication_required)?;
    if body.client_contract_revision != session_binding.client_contract_revision
        || body.security_policy_revision != session_binding.security_policy_revision
    {
        return Err(authentication_required());
    }
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
    let digest_hex = Sha256::digest(&canonical_command_bytes).iter().fold(
        String::with_capacity(64),
        |mut value, byte| {
            use std::fmt::Write as _;
            write!(value, "{byte:02x}").expect("writing to String cannot fail");
            value
        },
    );
    let challenge_binding = ProjectCommandChallengeBinding {
        project_scope: scope.clone(),
        client_session_binding_digest: binding_ref.clone(),
        client_session_generation: session_binding.session_generation,
        client_contract_revision: session_binding.client_contract_revision.clone(),
        security_policy_revision: session_binding.security_policy_revision.clone(),
        limit_profile_revision: contracts::LIMIT_PROFILE_REVISION.to_owned(),
        challenge_rate_policy_revision:
            storyos_application::PROJECT_COMMAND_CHALLENGE_RATE_POLICY_REVISION.to_owned(),
        method: contracts::APPLY_AUTHOR_EDIT_METHOD.to_owned(),
        route_template: contracts::APPLY_AUTHOR_EDIT_PATH.to_owned(),
        command_schema: contracts::APPLY_AUTHOR_EDIT_REQUEST_SCHEMA_ID.to_owned(),
        command_kind: "applyAuthorEdit".to_owned(),
        canonical_command_digest: format!(
            "sha256:storyos.command.applyAuthorEdit.jcs.v1:{digest_hex}"
        ),
        idempotency_key: idempotency_key.to_owned(),
    };
    let ids = AuthorCommandAdmissionIds {
        command_id: Uuid::now_v7().to_string(),
        author_command_admission_id: Uuid::now_v7().to_string(),
        receipt_id: Uuid::now_v7().to_string(),
    };
    let author_edit_units = body
        .author_edit_units
        .iter()
        .map(|unit| storyos_core::AuthorEditUnit {
            normalized_primitives: unit
                .normalized_primitives
                .iter()
                .map(|primitive| match primitive {
                    contracts::AuthorEditPrimitive::ReplaceSelection { from, to, text } => {
                        storyos_core::AuthorEditPrimitive::ReplaceSelection {
                            from: *from,
                            to: *to,
                            text: text.clone(),
                        }
                    }
                })
                .collect(),
            selection_snapshot: storyos_core::SelectionSnapshot {
                coordinate_profile: unit.selection_snapshot.coordinate_profile.clone(),
                from: unit.selection_snapshot.from,
                to: unit.selection_snapshot.to,
            },
        })
        .collect();
    let store = project_reader(&state)?;
    let command = ApplyAuthorEditCommand {
        project_scope: scope.clone(),
        client_binding: EditorClientBinding {
            binding_ref,
            session_generation: session_binding.session_generation,
            client_contract_revision: session_binding.client_contract_revision.clone(),
            security_policy_revision: session_binding.security_policy_revision.clone(),
        },
        challenge_binding,
        nonce_digest: plain_digest(nonce.as_bytes()),
        canonical_command_bytes,
        correlation_id: body.correlation_id.clone(),
        ids,
        editor_session_id: EditorSessionId::new(&body.editor_session_id),
        writer_generation: body
            .writer_generation
            .parse()
            .map_err(|_| invalid_request())?,
        chapter_id: body.chapter_id.clone(),
        expected_authoritative_revision_id: body.expected_authoritative_revision_id.clone(),
        expected_proposal_head_revision_ids: body.expected_proposal_head_revision_ids.clone(),
        target_refs: body.target_refs.clone(),
        observed_ownership_partition: body.observed_ownership_partition.clone(),
        editor_contract_revision: body.editor_contract_revision.clone(),
        undo_group_id: body.undo_group_id.clone(),
        completed_intent_record_id: body.completed_intent_record_id.clone(),
        local_intent_sequence: body
            .local_intent_sequence
            .parse()
            .map_err(|_| invalid_request())?,
        author_edit_units,
    };
    let settlement = storyos_application::apply_author_edit(&store, &command)
        .await
        .map_err(author_edit_error)?;
    let contract_project_scope = contract_scope(&scope);
    let (
        receipt_result,
        prior_heads,
        resulting_heads,
        authoritative_revision_ids,
        authoritative_commit_ids,
        author_action_sequence,
        effect,
    ) = match settlement.effect {
        AuthorEditSettlementEffect::AuthoritativeApplied {
            ids,
            body,
            author_action_sequence,
            project_activity_position,
        } => (
            contracts::DomainReceiptResult::AuthoritativeApplied,
            vec![command.expected_authoritative_revision_id.clone()],
            vec![ids.revision_id.clone()],
            vec![ids.revision_id.clone()],
            vec![ids.authoritative_commit_id.clone()],
            Some(author_action_sequence.to_string()),
            contracts::ApplyAuthorEditEffect::AuthoritativeApplied {
                authoritative_revision: contracts::AuthoritativeChapterRevision {
                    revision_id: ids.revision_id.clone(),
                    body,
                },
                authoritative_commit_id: ids.authoritative_commit_id,
                author_action_sequence: author_action_sequence.to_string(),
                project_activity_position: project_activity_position.to_string(),
            },
        ),
        AuthorEditSettlementEffect::NoEffect { reason } => (
            contracts::DomainReceiptResult::NoEffect,
            vec![command.expected_authoritative_revision_id.clone()],
            vec![command.expected_authoritative_revision_id.clone()],
            Vec::new(),
            Vec::new(),
            None,
            contracts::ApplyAuthorEditEffect::NoEffect {
                reason: match reason {
                    storyos_core::AuthorEditNoEffect::ContentUnchanged => {
                        contracts::NoEffectReason::ContentUnchanged
                    }
                },
            },
        ),
        AuthorEditSettlementEffect::Conflicted {
            reason,
            current_authoritative_revision_id,
        } => (
            contracts::DomainReceiptResult::Conflicted,
            vec![current_authoritative_revision_id.clone()],
            vec![current_authoritative_revision_id.clone()],
            Vec::new(),
            Vec::new(),
            None,
            contracts::ApplyAuthorEditEffect::Conflicted {
                reason: match reason {
                    storyos_core::AuthorEditConflict::StaleAuthoritativeHead => {
                        contracts::AuthorEditConflictReason::StaleAuthoritativeHead
                    }
                    storyos_core::AuthorEditConflict::ProposalHeadPresent => {
                        contracts::AuthorEditConflictReason::ProposalHeadPresent
                    }
                    storyos_core::AuthorEditConflict::OwnershipChanged => {
                        contracts::AuthorEditConflictReason::OwnershipChanged
                    }
                },
                current_authoritative_revision_id,
            },
        ),
        AuthorEditSettlementEffect::Refused { reason } => (
            contracts::DomainReceiptResult::Refused,
            vec![command.expected_authoritative_revision_id.clone()],
            vec![command.expected_authoritative_revision_id.clone()],
            Vec::new(),
            Vec::new(),
            None,
            contracts::ApplyAuthorEditEffect::Refused {
                reason: match reason {
                    storyos_core::AuthorEditRefusal::UnsupportedIntentShape => {
                        contracts::AuthorEditRefusalReason::UnsupportedIntentShape
                    }
                    storyos_core::AuthorEditRefusal::InvalidSelection => {
                        contracts::AuthorEditRefusalReason::InvalidSelection
                    }
                    storyos_core::AuthorEditRefusal::TargetMismatch => {
                        contracts::AuthorEditRefusalReason::TargetMismatch
                    }
                },
            },
        ),
    };
    Ok(Json(contracts::ApplyAuthorEditResponse {
        schema_id: contracts::APPLY_AUTHOR_EDIT_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: body.correlation_id,
        project_scope: contract_project_scope.clone(),
        command_id: settlement.ids.command_id,
        author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
        receipt: contracts::DomainReceipt {
            receipt_id: settlement.ids.receipt_id.clone(),
            project_scope: contract_project_scope,
            command_kind: contracts::DomainReceiptCommandKind::ApplyAuthorEdit,
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: "storyos.command.applyAuthorEdit.jcs.v1".to_owned(),
                value_hex_lowercase: digest_hex,
            },
            idempotency_key: command.challenge_binding.idempotency_key.clone(),
            producer_cause: contracts::DomainReceiptProducerCause::AuthorCommandAdmission,
            author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
            expected_heads: vec![command.expected_authoritative_revision_id.clone()],
            prior_heads,
            resulting_heads,
            authoritative_revision_ids,
            proposal_revision_ids: Vec::new(),
            authoritative_commit_ids,
            author_action_sequence,
            draft_artifact_refs: Vec::new(),
            artifact_lifecycle_event_refs: Vec::new(),
            condition_refs: Vec::new(),
            result: receipt_result,
            created_at: settlement.receipt_created_at,
        },
        effect,
        completed_intent_record_id: settlement.completed_intent_record_id,
        local_intent_sequence: settlement.local_intent_sequence.to_string(),
    }))
}

fn validate_request(body: &contracts::ApplyAuthorEditRequest) -> Result<(), ApiError> {
    let normalized_primitive_count = body
        .author_edit_units
        .iter()
        .try_fold(0_usize, |count, unit| {
            count.checked_add(unit.normalized_primitives.len())
        })
        .ok_or_else(command_target_refused)?;
    if body.command_schema != contracts::APPLY_AUTHOR_EDIT_REQUEST_SCHEMA_ID
        || body.observed_ownership_partition != "authoritative"
        || body.editor_contract_revision != contracts::EDITOR_CONTRACT_REVISION
        || !body.expected_proposal_head_revision_ids.is_empty()
        || body.author_edit_units.is_empty()
        || body.author_edit_units.len() > contracts::AUTHOR_EDIT_MAX_UNITS
        || normalized_primitive_count > contracts::AUTHOR_EDIT_MAX_NORMALIZED_PRIMITIVES
    {
        return Err(command_target_refused());
    }
    for value in [
        &body.correlation_id,
        &body.editor_session_id,
        &body.chapter_id,
        &body.expected_authoritative_revision_id,
        &body.undo_group_id,
        &body.completed_intent_record_id,
    ] {
        valid_uuid(value)?;
    }
    if body.target_refs != [format!("manuscript:{}", body.chapter_id)] {
        return Err(command_target_refused());
    }
    Ok(())
}

fn canonical_body_bytes(body: &contracts::ApplyAuthorEditRequest) -> Result<Vec<u8>, ApiError> {
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

fn author_edit_error(error: AuthorEditError) -> ApiError {
    match error {
        AuthorEditError::BindingConflict => problem(
            StatusCode::CONFLICT,
            "idempotency_binding_conflict",
            "The Author Edit binding conflicts.",
        ),
        AuthorEditError::InvalidChallenge => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "challenge_invalid",
            "The command challenge is invalid or expired.",
        ),
        AuthorEditError::StaleWriter => problem(
            StatusCode::PRECONDITION_FAILED,
            "editor_writer_stale",
            "The Editor Session is not the current writer.",
        ),
        AuthorEditError::AdmissionExpired => problem(
            StatusCode::CONFLICT,
            "author_command_admission_expired",
            "The Author Command Admission expired before Core settlement.",
        ),
        AuthorEditError::Unavailable(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "author_edit_store_unavailable",
            "The Author Edit store is unavailable.",
        ),
    }
}

#[cfg(test)]
#[path = "author_edit_tests.rs"]
mod tests;
