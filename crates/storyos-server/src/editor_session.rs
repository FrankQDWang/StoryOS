use std::collections::BTreeMap;

use axum::body::to_bytes;
use sha2::{Digest, Sha256};
use storyos_application::{
    EditorClientBinding, EditorSession, EditorSessionError, EditorSessionId, EditorSessionLookup,
    EditorWriterState, OpenEditorSession, ProjectCommandChallengeBinding,
};

use super::project_command_challenge::{
    SESSION_BINDING_HMAC_PROFILE, plain_digest, secret_digest, valid_uuid_v7,
    validate_json_content_type,
};
use super::*;

pub(super) fn session_binding_ref(secret: &[u8], session_handle: &str) -> String {
    secret_digest(
        secret,
        SESSION_BINDING_HMAC_PROFILE,
        &[session_handle.as_bytes().to_vec()],
    )
}

pub(super) async fn create_editor_session(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    request: Request,
) -> Result<Json<contracts::CreateEditorSessionResponse>, ApiError> {
    let (parts, body) = request.into_parts();
    let headers = parts.headers;
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::StateChanging,
    )?;
    validate_json_content_type(&headers)?;
    let bytes = to_bytes(body, 1024 * 1024)
        .await
        .map_err(|_| payload_too_large())?;
    let body = serde_json::from_slice::<contracts::CreateEditorSessionRequest>(&bytes)
        .map_err(|_| invalid_request_shape())?;
    valid_uuid(&body.correlation_id)?;
    if body.command_schema != contracts::CREATE_EDITOR_SESSION_REQUEST_SCHEMA_ID {
        return Err(command_target_refused());
    }
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
    let canonical_body = BTreeMap::from([
        (
            "client_contract_revision",
            body.client_contract_revision.as_str(),
        ),
        ("command_schema", body.command_schema.as_str()),
        ("correlation_id", body.correlation_id.as_str()),
        (
            "security_policy_revision",
            body.security_policy_revision.as_str(),
        ),
    ]);
    let digest = Sha256::digest(
        serde_json::to_vec(&canonical_body).expect("CreateEditorSession request must serialize"),
    );
    let digest_hex = digest
        .iter()
        .fold(String::with_capacity(64), |mut value, byte| {
            use std::fmt::Write as _;
            write!(value, "{byte:02x}").expect("writing to String cannot fail");
            value
        });
    let challenge_binding = ProjectCommandChallengeBinding {
        project_scope: scope.clone(),
        client_session_binding_digest: binding_ref.clone(),
        client_session_generation: session_binding.session_generation,
        client_contract_revision: session_binding.client_contract_revision.clone(),
        security_policy_revision: session_binding.security_policy_revision.clone(),
        limit_profile_revision: contracts::LIMIT_PROFILE_REVISION.to_owned(),
        challenge_rate_policy_revision:
            storyos_application::PROJECT_COMMAND_CHALLENGE_RATE_POLICY_REVISION.to_owned(),
        method: contracts::CREATE_EDITOR_SESSION_METHOD.to_owned(),
        route_template: contracts::CREATE_EDITOR_SESSION_PATH.to_owned(),
        command_schema: contracts::CREATE_EDITOR_SESSION_REQUEST_SCHEMA_ID.to_owned(),
        command_kind: "createEditorSession".to_owned(),
        canonical_command_digest: format!(
            "sha256:storyos.command.createEditorSession.jcs.v1:{digest_hex}"
        ),
        idempotency_key: idempotency_key.to_owned(),
    };
    let store = project_reader(&state)?;
    let session = storyos_application::create_editor_session(
        &store,
        &OpenEditorSession {
            project_scope: scope.clone(),
            editor_session_id: EditorSessionId::new(Uuid::now_v7().to_string()),
            snapshot_id: Uuid::now_v7().to_string(),
            client_binding: EditorClientBinding {
                binding_ref,
                session_generation: session_binding.session_generation,
                client_contract_revision: session_binding.client_contract_revision.clone(),
                security_policy_revision: session_binding.security_policy_revision.clone(),
            },
            challenge_binding,
            nonce_digest: plain_digest(nonce.as_bytes()),
        },
    )
    .await
    .map_err(editor_session_error)?;
    Ok(Json(create_response(&scope, body.correlation_id, session)))
}

pub(super) async fn get_editor_session(
    State(state): State<Arc<ServerState>>,
    Path((project_id, editor_session_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<contracts::GetEditorSessionResponse>, ApiError> {
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    valid_uuid(&editor_session_id)?;
    let session_handle = session_cookie(&headers).ok_or_else(authentication_required)?;
    let session_binding = state
        .config
        .session_bindings
        .get(session_handle)
        .ok_or_else(authentication_required)?;
    let secret = state
        .config
        .project_command_challenge_secret
        .as_deref()
        .filter(|secret| secret.len() >= 32)
        .ok_or_else(challenge_store_unavailable)?;
    let binding_ref = session_binding_ref(secret, session_handle);
    let store = project_reader(&state)?;
    let session = storyos_application::get_editor_session(
        &store,
        &EditorSessionLookup {
            project_scope: scope.clone(),
            editor_session_id: EditorSessionId::new(editor_session_id),
            client_binding: EditorClientBinding {
                binding_ref,
                session_generation: session_binding.session_generation,
                client_contract_revision: session_binding.client_contract_revision.clone(),
                security_policy_revision: session_binding.security_policy_revision.clone(),
            },
        },
    )
    .await
    .map_err(editor_session_error)?
    .ok_or_else(resource_unavailable)?;
    let response = create_response(&scope, Uuid::now_v7().to_string(), session);
    Ok(Json(contracts::GetEditorSessionResponse {
        schema_id: "storyos.query.editor-session.response.v1".to_owned(),
        correlation_id: response.correlation_id,
        project_scope: response.project_scope,
        editor_session: response.editor_session,
        writer: response.writer,
        base_snapshot: response.base_snapshot,
    }))
}

pub(super) fn exact_header<'a>(headers: &'a HeaderMap, name: &str) -> Result<&'a str, ApiError> {
    let mut values = headers.get_all(name).iter();
    let value = values
        .next()
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty());
    if values.next().is_some() {
        return Err(invalid_request());
    }
    value.ok_or_else(invalid_request)
}

fn create_response(
    scope: &ApplicationScope,
    correlation_id: String,
    session: EditorSession,
) -> contracts::CreateEditorSessionResponse {
    let writer = match session.writer {
        EditorWriterState::CurrentWriter { writer_generation } => {
            contracts::EditorWriterProjection::CurrentWriter {
                writer_generation: writer_generation.to_string(),
            }
        }
        EditorWriterState::ReadOnly {
            observed_writer_generation,
        } => contracts::EditorWriterProjection::ReadOnly {
            observed_writer_generation: observed_writer_generation.to_string(),
            reason: contracts::EditorReadOnlyReason::SecondarySession,
        },
    };
    let chapter_id = session.base_snapshot.chapter_id.clone();
    contracts::CreateEditorSessionResponse {
        schema_id: "storyos.command.create-editor-session.response.v1".to_owned(),
        correlation_id,
        project_scope: contract_scope(scope),
        editor_session: contracts::EditorSessionBinding {
            editor_session_id: session.editor_session_id.as_ref().to_owned(),
            client_session_binding_ref: session.client_binding.binding_ref,
            client_session_generation: session.client_binding.session_generation.to_string(),
            client_contract_revision: session.client_binding.client_contract_revision,
            security_policy_revision: session.client_binding.security_policy_revision,
            opened_at: session.opened_at,
            disposition: "open".to_owned(),
        },
        writer,
        base_snapshot: contracts::EditorBaseSnapshot {
            snapshot_id: session.base_snapshot.snapshot_id,
            chapter_id: chapter_id.clone(),
            project_activity_position: session.base_snapshot.project_activity_position.to_string(),
            authoritative_head_revision_id: session.base_snapshot.authoritative_revision_id.clone(),
            proposal_head_revision_ids: Vec::new(),
            target_refs: vec![format!("manuscript:{chapter_id}")],
            observed_ownership_partition: "authoritative".to_owned(),
            materialized_revision: contracts::AuthoritativeChapterRevision {
                revision_id: session.base_snapshot.authoritative_revision_id,
                body: session.base_snapshot.body,
            },
            materialized_payload_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: "storyos.canonical-payload.sha256.v1".to_owned(),
                value_hex_lowercase: session.base_snapshot.payload_digest_hex,
            },
            created_at: session.base_snapshot.created_at,
        },
    }
}

fn editor_session_error(error: EditorSessionError) -> ApiError {
    match error {
        EditorSessionError::BindingConflict => problem(
            StatusCode::CONFLICT,
            "idempotency_binding_conflict",
            "The Editor Session binding conflicts.",
        ),
        EditorSessionError::InvalidChallenge => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "challenge_invalid",
            "The command challenge is invalid or expired.",
        ),
        EditorSessionError::Unavailable(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "editor_session_store_unavailable",
            "The Editor Session store is unavailable.",
        ),
    }
}
