use axum::body::to_bytes;
use sha2::{Digest, Sha256};
use storyos_application::{
    CreateProjectChallengeBinding, IssueCreateProjectChallenge, ProjectId,
    issue_create_project_challenge,
};

use super::project_command_challenge::{
    SESSION_BINDING_HMAC_PROFILE, hex_bytes, plain_digest, secret_digest, valid_uuid_v7,
    validate_json_content_type,
};
use super::*;

const CREATE_PROJECT_ROUTE: &str = "/api/v1/projects";
const CREATE_PROJECT_METHOD: &str = "POST";
const CREATE_PROJECT_KIND: &str = "createProject";
const NONCE_HMAC_PROFILE: &str = "storyos.project-command-challenge.nonce.hmac-sha256.v1";

pub(super) async fn create_project_challenge(
    State(state): State<Arc<ServerState>>,
    request: Request,
) -> Result<Json<contracts::CreateProjectChallengeResponse>, ApiError> {
    let (parts, body) = request.into_parts();
    let headers = parts.headers;
    let request_origin =
        validate_request_site(&state, &headers, RequestOriginPolicy::StateChanging)?;
    let session_handle = session_cookie(&headers).ok_or_else(authentication_required)?;
    let session = state
        .config
        .session_bindings
        .get(session_handle)
        .ok_or_else(authentication_required)?;
    validate_session_binding(&state, session_handle, session, &headers, &request_origin)?;
    validate_json_content_type(&headers)?;
    let bytes = to_bytes(body, 1024 * 1024)
        .await
        .map_err(|_| payload_too_large())?;
    let request = serde_json::from_slice::<contracts::CreateProjectChallengeRequest>(&bytes)
        .map_err(|_| invalid_request_shape())?;
    if request.command_schema != contracts::CREATE_PROJECT_REQUEST_SCHEMA_ID
        || request.create_project_input.client_contract_revision != session.client_contract_revision
        || request.create_project_input.security_policy_revision != session.security_policy_revision
        || request.create_project_input.title.is_empty()
        || request.create_project_input.title.len() > 1024
        || !valid_uuid_v7(&request.idempotency_key)
    {
        return Err(invalid_request());
    }
    valid_uuid(&request.create_project_input.correlation_id)?;
    let secret = state
        .config
        .project_command_challenge_secret
        .as_deref()
        .ok_or_else(challenge_store_unavailable)?;
    if secret.len() < 32 {
        return Err(challenge_store_unavailable());
    }
    let prospective_project_id = Uuid::now_v7().to_string();
    let canonical_command_digest =
        create_project_command_digest(&prospective_project_id, &request.create_project_input);
    let session_digest = secret_digest(
        secret,
        SESSION_BINDING_HMAC_PROFILE,
        &[session_handle.as_bytes().to_vec()],
    );
    let binding = CreateProjectChallengeBinding {
        owner_user_id: session.owner_user_id.clone(),
        prospective_project_id: ProjectId::new(prospective_project_id),
        client_session_binding_digest: session_digest,
        client_session_generation: session.session_generation,
        client_contract_revision: session.client_contract_revision.clone(),
        security_policy_revision: session.security_policy_revision.clone(),
        limit_profile_revision: contracts::LIMIT_PROFILE_REVISION.to_owned(),
        method: CREATE_PROJECT_METHOD.to_owned(),
        route_template: CREATE_PROJECT_ROUTE.to_owned(),
        command_schema: request.command_schema,
        command_kind: CREATE_PROJECT_KIND.to_owned(),
        create_input_digest: create_input_digest(&request.create_project_input)?,
        canonical_command_digest,
        idempotency_key: request.idempotency_key,
    };
    let nonce_digest = plain_digest(
        secret_digest(secret, NONCE_HMAC_PROFILE, &binding_parts(&binding)).as_bytes(),
    );
    let store = project_reader(&state)?;
    let issued = issue_create_project_challenge(
        &store,
        &IssueCreateProjectChallenge {
            binding: binding.clone(),
            nonce_digest,
        },
    )
    .await
    .map_err(challenge_error)?;
    let mut returned = binding;
    returned.prospective_project_id = issued.prospective_project_id.clone();
    returned.canonical_command_digest = issued.canonical_command_digest.clone();
    let nonce = secret_digest(secret, NONCE_HMAC_PROFILE, &binding_parts(&returned));
    Ok(Json(contracts::CreateProjectChallengeResponse {
        prospective_project_id: issued.prospective_project_id.as_ref().to_owned(),
        canonical_command_digest: digest_value(&issued.canonical_command_digest)?,
        nonce,
        expires_at: issued.expires_at,
        limit_profile_revision: contracts::LIMIT_PROFILE_REVISION.to_owned(),
    }))
}

fn create_project_command_digest(
    project_id: &str,
    input: &contracts::CreateProjectInput,
) -> String {
    let canonical = canonical_json(serde_json::json!({
        "client_contract_revision": input.client_contract_revision,
        "correlation_id": input.correlation_id,
        "project_id": project_id,
        "security_policy_revision": input.security_policy_revision,
        "title": input.title,
    }));
    let digest = Sha256::digest(
        serde_json::to_vec(&canonical).expect("Create Project digest input must serialize"),
    );
    format!(
        "sha256:{}:{}",
        contracts::CREATE_PROJECT_DIGEST_PROFILE,
        hex_bytes(&digest)
    )
}

fn create_input_digest(input: &contracts::CreateProjectInput) -> Result<String, ApiError> {
    let canonical =
        canonical_json(serde_json::to_value(input).map_err(|_| invalid_request_shape())?);
    Ok(plain_digest(
        &serde_json::to_vec(&canonical).map_err(|_| invalid_request_shape())?,
    ))
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

fn binding_parts(binding: &CreateProjectChallengeBinding) -> Vec<Vec<u8>> {
    vec![
        binding.owner_user_id.as_ref().as_bytes().to_vec(),
        binding.prospective_project_id.as_ref().as_bytes().to_vec(),
        binding.client_session_binding_digest.as_bytes().to_vec(),
        binding.client_session_generation.to_be_bytes().to_vec(),
        binding.client_contract_revision.as_bytes().to_vec(),
        binding.security_policy_revision.as_bytes().to_vec(),
        binding.limit_profile_revision.as_bytes().to_vec(),
        binding.method.as_bytes().to_vec(),
        binding.route_template.as_bytes().to_vec(),
        binding.command_schema.as_bytes().to_vec(),
        binding.command_kind.as_bytes().to_vec(),
        binding.canonical_command_digest.as_bytes().to_vec(),
        binding.idempotency_key.as_bytes().to_vec(),
    ]
}

fn digest_value(stored: &str) -> Result<contracts::DigestValue, ApiError> {
    let prefix = format!("sha256:{}:", contracts::CREATE_PROJECT_DIGEST_PROFILE);
    stored
        .strip_prefix(&prefix)
        .filter(|value| value.len() == 64)
        .map(|value| contracts::DigestValue {
            algorithm: contracts::DigestAlgorithm::Sha256,
            profile: contracts::CREATE_PROJECT_DIGEST_PROFILE.to_owned(),
            value_hex_lowercase: value.to_owned(),
        })
        .ok_or_else(challenge_store_unavailable)
}
