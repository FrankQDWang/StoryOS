use std::fmt::Write as _;

use axum::body::to_bytes;
use sha2::{Digest, Sha256};
use storyos_application::{
    AuthorCommandAdmissionIds, EditorClientBinding, EditorSessionId,
    ProjectCommandChallengeBinding, TakeOverProjectWriterCommand, TakeOverProjectWriterEffect,
    TakeOverProjectWriterError, TakeOverProjectWriterSettlement,
};

use super::editor_session::{exact_header, session_binding_ref};
use super::project_command_challenge::{plain_digest, valid_uuid_v7, validate_json_content_type};
use super::*;

pub(super) async fn take_over_project_writer(
    State(state): State<Arc<ServerState>>,
    Path((project_id, editor_session_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::TakeOverProjectWriterResponse>, ApiError> {
    let (parts, body_stream) = request.into_parts();
    let headers = parts.headers;
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::StateChanging,
    )?;
    valid_uuid(&editor_session_id)?;
    validate_json_content_type(&headers)?;
    let bytes = to_bytes(body_stream, contracts::AUTHOR_EDIT_MAX_WIRE_BODY_BYTES)
        .await
        .map_err(|_| payload_too_large())?;
    let body = serde_json::from_slice::<contracts::TakeOverProjectWriterRequest>(&bytes)
        .map_err(|_| invalid_request_shape())?;
    validate_request(&body)?;
    if body.editor_session_id != editor_session_id {
        return Err(invalid_request());
    }
    let session_handle = session_cookie(&headers).ok_or_else(authentication_required)?;
    let session_binding = state
        .client_session_binding(session_handle)
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
        method: contracts::TAKE_OVER_PROJECT_WRITER_METHOD.to_owned(),
        route_template: contracts::TAKE_OVER_PROJECT_WRITER_PATH.to_owned(),
        command_schema: contracts::TAKE_OVER_PROJECT_WRITER_REQUEST_SCHEMA_ID.to_owned(),
        command_kind: "takeOverProjectWriter".to_owned(),
        canonical_command_digest: format!(
            "sha256:storyos.command.takeOverProjectWriter.jcs.v1:{digest_hex}"
        ),
        idempotency_key: idempotency_key.to_owned(),
    };
    let store = project_reader(&state)?;
    let command = TakeOverProjectWriterCommand {
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
        ids: AuthorCommandAdmissionIds {
            command_id: Uuid::now_v7().to_string(),
            author_command_admission_id: Uuid::now_v7().to_string(),
            receipt_id: Uuid::now_v7().to_string(),
        },
        editor_session_id: EditorSessionId::new(&body.editor_session_id),
        observed_writer_generation: body
            .observed_writer_generation
            .parse()
            .map_err(|_| invalid_request())?,
        editor_contract_revision: body.editor_contract_revision.clone(),
    };
    let settlement = storyos_application::take_over_project_writer(&store, &command)
        .await
        .map_err(takeover_error)?;
    Ok(Json(takeover_response(
        &scope,
        command.correlation_id.clone(),
        command.challenge_binding.canonical_command_digest.clone(),
        command.challenge_binding.idempotency_key.clone(),
        settlement,
    )?))
}

fn takeover_response(
    scope: &ApplicationScope,
    correlation_id: String,
    canonical_command_digest: String,
    idempotency_key: String,
    settlement: TakeOverProjectWriterSettlement,
) -> Result<contracts::TakeOverProjectWriterResponse, ApiError> {
    let digest_hex = canonical_command_digest
        .strip_prefix("sha256:storyos.command.takeOverProjectWriter.jcs.v1:")
        .filter(|value| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .ok_or_else(resource_unavailable)?
        .to_owned();
    let contract_project_scope = contract_scope(scope);
    let result = match settlement.effect {
        TakeOverProjectWriterEffect::TakeoverApplied {
            prior_editor_session_id,
            prior_writer_generation,
            resulting_editor_session_id,
            resulting_writer_generation,
            resulting_snapshot_id,
            resulting_snapshot_activity_position,
        } => contracts::TakeOverProjectWriterResult::TakeoverApplied {
            prior_editor_session_id,
            prior_writer_generation: prior_writer_generation.to_string(),
            resulting_editor_session_id,
            resulting_writer_generation: resulting_writer_generation.to_string(),
            resulting_snapshot_id,
            resulting_snapshot_activity_position: resulting_snapshot_activity_position.to_string(),
            resulting_heads: settlement.resulting_heads.clone(),
        },
    };
    Ok(contracts::TakeOverProjectWriterResponse {
        schema_id: contracts::TAKE_OVER_PROJECT_WRITER_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id,
        project_scope: contract_project_scope.clone(),
        command_id: settlement.ids.command_id,
        author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
        receipt: contracts::DomainReceipt {
            receipt_id: settlement.ids.receipt_id.clone(),
            project_scope: contract_project_scope,
            command_kind: contracts::DomainReceiptCommandKind::TakeOverProjectWriter,
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: "storyos.command.takeOverProjectWriter.jcs.v1".to_owned(),
                value_hex_lowercase: digest_hex,
            },
            idempotency_key,
            producer_cause: contracts::DomainReceiptProducerCause::AuthorCommandAdmission,
            author_command_admission_id: settlement.ids.author_command_admission_id,
            expected_heads: settlement.expected_heads,
            prior_heads: settlement.prior_heads,
            resulting_heads: settlement.resulting_heads,
            authoritative_revision_ids: Vec::new(),
            proposal_revision_ids: Vec::new(),
            authoritative_commit_ids: Vec::new(),
            author_action_sequence: None,
            draft_artifact_refs: Vec::new(),
            artifact_lifecycle_event_refs: Vec::new(),
            condition_refs: Vec::new(),
            result: contracts::DomainReceiptResult::NoEffect,
            created_at: settlement.receipt_created_at,
        },
        result,
    })
}

fn validate_request(body: &contracts::TakeOverProjectWriterRequest) -> Result<(), ApiError> {
    if body.command_schema != contracts::TAKE_OVER_PROJECT_WRITER_REQUEST_SCHEMA_ID
        || body.editor_contract_revision != contracts::EDITOR_CONTRACT_REVISION
    {
        return Err(command_target_refused());
    }
    valid_uuid(&body.correlation_id)?;
    valid_uuid(&body.editor_session_id)?;
    let observed_generation = body
        .observed_writer_generation
        .parse::<u64>()
        .map_err(|_| invalid_request())?;
    if observed_generation < 1 {
        return Err(invalid_request());
    }
    Ok(())
}

fn canonical_body_bytes(
    body: &contracts::TakeOverProjectWriterRequest,
) -> Result<Vec<u8>, ApiError> {
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

fn takeover_error(error: TakeOverProjectWriterError) -> ApiError {
    match error {
        TakeOverProjectWriterError::BindingConflict => problem(
            StatusCode::CONFLICT,
            "idempotency_binding_conflict",
            "The writer takeover binding conflicts.",
        ),
        TakeOverProjectWriterError::InvalidChallenge => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "challenge_invalid",
            "The command challenge is invalid or expired.",
        ),
        TakeOverProjectWriterError::Unavailable(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "author_edit_store_unavailable",
            "The writer takeover store is unavailable.",
        ),
    }
}
