use axum::body::to_bytes;
use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};
use storyos_application::{
    IssueProjectCommandChallenge, ProjectCommandChallengeBinding, issue_project_command_challenge,
};

use super::*;

const SESSION_BINDING_HMAC_PROFILE: &str =
    "storyos.project-command-challenge.session-binding.hmac-sha256.v1";
const NONCE_HMAC_PROFILE: &str = "storyos.project-command-challenge.nonce.hmac-sha256.v1";

pub(super) async fn create_project_command_challenge(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    request: Request,
) -> Result<Json<contracts::CreateProjectCommandChallengeResponse>, ApiError> {
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
    let request = serde_json::from_slice::<contracts::CreateProjectCommandChallengeRequest>(&bytes)
        .map_err(|_| invalid_request_shape())?;
    let session_handle = session_cookie(&headers).ok_or_else(authentication_required)?;
    let binding = state
        .config
        .session_bindings
        .get(session_handle)
        .ok_or_else(authentication_required)?;
    let command_kind = contracts::project_command_kind(
        &request.method,
        &request.route_template,
        &request.command_schema,
    )
    .ok_or_else(command_target_refused)?;
    if !valid_command_digest(&request.canonical_command_digest, command_kind)
        || !valid_uuid_v7(&request.idempotency_key)
    {
        return Err(invalid_request());
    }
    let store = project_reader(&state)?;
    if open_project(&store, &scope)
        .await
        .map_err(service_unavailable)?
        .is_none()
    {
        return Err(resource_unavailable());
    }
    let secret = state
        .config
        .project_command_challenge_secret
        .as_deref()
        .ok_or_else(challenge_store_unavailable)?;
    if secret.len() < 32 {
        return Err(challenge_store_unavailable());
    }
    let session_digest = secret_digest(
        secret,
        SESSION_BINDING_HMAC_PROFILE,
        &[session_handle.as_bytes().to_vec()],
    );
    let challenge_binding = ProjectCommandChallengeBinding {
        project_scope: scope,
        client_session_binding_digest: session_digest,
        client_session_generation: binding.session_generation,
        client_contract_revision: binding.client_contract_revision.clone(),
        security_policy_revision: binding.security_policy_revision.clone(),
        limit_profile_revision: contracts::LIMIT_PROFILE_REVISION.to_owned(),
        challenge_rate_policy_revision:
            storyos_application::PROJECT_COMMAND_CHALLENGE_RATE_POLICY_REVISION.to_owned(),
        method: request.method,
        route_template: request.route_template,
        command_schema: request.command_schema,
        command_kind: command_kind.to_owned(),
        canonical_command_digest: canonical_digest(&request.canonical_command_digest),
        idempotency_key: request.idempotency_key,
    };
    let nonce = secret_digest(
        secret,
        NONCE_HMAC_PROFILE,
        &binding_parts(&challenge_binding),
    );
    let nonce_digest = plain_digest(nonce.as_bytes());
    let challenge = issue_project_command_challenge(
        &store,
        &IssueProjectCommandChallenge {
            binding: challenge_binding,
            nonce,
            nonce_digest,
        },
    )
    .await
    .map_err(challenge_error)?;
    Ok(Json(contracts::CreateProjectCommandChallengeResponse {
        nonce: challenge.nonce,
        expires_at: challenge.expires_at,
        limit_profile_revision: contracts::LIMIT_PROFILE_REVISION.to_owned(),
    }))
}

fn binding_parts(binding: &ProjectCommandChallengeBinding) -> Vec<Vec<u8>> {
    vec![
        binding
            .project_scope
            .owner_user_id
            .as_ref()
            .as_bytes()
            .to_vec(),
        binding
            .project_scope
            .project_id
            .as_ref()
            .as_bytes()
            .to_vec(),
        binding.client_session_binding_digest.as_bytes().to_vec(),
        binding.client_session_generation.to_be_bytes().to_vec(),
        binding.client_contract_revision.as_bytes().to_vec(),
        binding.security_policy_revision.as_bytes().to_vec(),
        binding.limit_profile_revision.as_bytes().to_vec(),
        binding.challenge_rate_policy_revision.as_bytes().to_vec(),
        binding.method.as_bytes().to_vec(),
        binding.route_template.as_bytes().to_vec(),
        binding.command_schema.as_bytes().to_vec(),
        binding.command_kind.as_bytes().to_vec(),
        binding.canonical_command_digest.as_bytes().to_vec(),
        binding.idempotency_key.as_bytes().to_vec(),
    ]
}

pub(super) fn secret_digest(secret: &[u8], profile: &str, parts: &[Vec<u8>]) -> String {
    let mut digest =
        Hmac::<Sha256>::new_from_slice(secret).expect("HMAC-SHA256 accepts a secret of any length");
    for part in std::iter::once(profile.as_bytes()).chain(parts.iter().map(Vec::as_slice)) {
        digest.update(&part.len().to_be_bytes());
        digest.update(part);
    }
    hex_bytes(&digest.finalize().into_bytes())
}

fn plain_digest(value: &[u8]) -> String {
    format!("sha256:{}", hex_bytes(&Sha256::digest(value)))
}

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn valid_command_digest(value: &contracts::DigestValue, command_kind: &str) -> bool {
    matches!(value.algorithm, contracts::DigestAlgorithm::Sha256)
        && value.profile == format!("storyos.command.{command_kind}.jcs.v1")
        && value.value_hex_lowercase.len() == 64
        && value
            .value_hex_lowercase
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn canonical_digest(value: &contracts::DigestValue) -> String {
    format!(
        "{}:{}:{}",
        "sha256", value.profile, value.value_hex_lowercase
    )
}

fn valid_uuid_v7(value: &str) -> bool {
    Uuid::parse_str(value)
        .is_ok_and(|uuid| uuid.get_version_num() == 7 && uuid.to_string() == value)
}
