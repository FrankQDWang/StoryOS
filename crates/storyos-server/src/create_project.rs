use axum::body::to_bytes;
use storyos_application::{
    AuthorCommandAdmissionIds, CreateProjectChallengeBinding, CreateProjectCommand,
    CreateProjectError, EditorClientBinding,
};

use super::create_project_challenge::{create_input_digest, create_project_command_digest};
use super::editor_session::{exact_header, session_binding_ref};
use super::project_command_challenge::{plain_digest, valid_uuid_v7, validate_json_content_type};
use super::*;

pub(super) async fn create_project(
    State(state): State<Arc<ServerState>>,
    request: Request,
) -> Result<Json<contracts::CreateProjectResponse>, ApiError> {
    let (parts, body_stream) = request.into_parts();
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
    let bytes = to_bytes(body_stream, contracts::AUTHOR_EDIT_MAX_WIRE_BODY_BYTES)
        .await
        .map_err(|_| payload_too_large())?;
    let body = serde_json::from_slice::<contracts::CreateProjectRequest>(&bytes)
        .map_err(|_| invalid_request_shape())?;
    if body.command_schema != contracts::CREATE_PROJECT_REQUEST_SCHEMA_ID
        || body.create_project_input.client_contract_revision != session.client_contract_revision
        || body.create_project_input.security_policy_revision != session.security_policy_revision
        || body.create_project_input.title.is_empty()
        || body.create_project_input.title.len() > 1024
    {
        return Err(invalid_request());
    }
    valid_uuid(&body.prospective_project_id)?;
    valid_uuid(&body.create_project_input.correlation_id)?;
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
    let canonical_command_digest =
        create_project_command_digest(&body.prospective_project_id, &body.create_project_input);
    let challenge_binding = CreateProjectChallengeBinding {
        owner_user_id: session.owner_user_id.clone(),
        prospective_project_id: ProjectId::new(body.prospective_project_id.clone()),
        client_session_binding_digest: binding_ref.clone(),
        client_session_generation: session.session_generation,
        client_contract_revision: session.client_contract_revision.clone(),
        security_policy_revision: session.security_policy_revision.clone(),
        limit_profile_revision: contracts::LIMIT_PROFILE_REVISION.to_owned(),
        method: contracts::CREATE_PROJECT_METHOD.to_owned(),
        route_template: contracts::CREATE_PROJECT_PATH.to_owned(),
        command_schema: body.command_schema.clone(),
        command_kind: "createProject".to_owned(),
        create_input_digest: create_input_digest(&body.create_project_input)?,
        canonical_command_digest: canonical_command_digest.clone(),
        idempotency_key: idempotency_key.to_owned(),
    };
    let scope = ApplicationScope::new(
        session.owner_user_id.clone(),
        ProjectId::new(body.prospective_project_id.clone()),
    );
    let store = project_reader(&state)?;
    let settlement = storyos_application::create_project(
        &store,
        &CreateProjectCommand {
            project_scope: scope.clone(),
            client_binding: EditorClientBinding {
                binding_ref,
                session_generation: session.session_generation,
                client_contract_revision: session.client_contract_revision.clone(),
                security_policy_revision: session.security_policy_revision.clone(),
            },
            challenge_binding,
            nonce_digest: plain_digest(nonce.as_bytes()),
            canonical_command_bytes: bytes.to_vec(),
            correlation_id: body.create_project_input.correlation_id.clone(),
            title: body.create_project_input.title.clone(),
            ids: AuthorCommandAdmissionIds {
                command_id: Uuid::now_v7().to_string(),
                author_command_admission_id: Uuid::now_v7().to_string(),
                receipt_id: Uuid::now_v7().to_string(),
            },
        },
    )
    .await
    .map_err(create_project_error)?;
    create_project_response(
        &scope,
        &body.create_project_input.correlation_id,
        &canonical_command_digest,
        idempotency_key,
        settlement,
    )
}

fn create_project_response(
    scope: &ApplicationScope,
    correlation_id: &str,
    canonical_command_digest: &str,
    idempotency_key: &str,
    settlement: storyos_application::CreateProjectSettlement,
) -> Result<Json<contracts::CreateProjectResponse>, ApiError> {
    let digest_hex = canonical_command_digest
        .strip_prefix("sha256:storyos.command.createProject.jcs.v1:")
        .filter(|value| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .ok_or_else(resource_unavailable)?
        .to_owned();
    let contract_project_scope = contract_scope(scope);
    Ok(Json(contracts::CreateProjectResponse {
        schema_id: contracts::CREATE_PROJECT_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: correlation_id.to_owned(),
        project_scope: contract_project_scope.clone(),
        command_id: settlement.ids.command_id.clone(),
        author_command_admission_id: settlement.ids.author_command_admission_id.clone(),
        receipt: contracts::DomainReceipt {
            receipt_id: settlement.ids.receipt_id.clone(),
            project_scope: contract_project_scope,
            command_kind: contracts::DomainReceiptCommandKind::CreateProject,
            command_digest: contracts::DigestValue {
                algorithm: contracts::DigestAlgorithm::Sha256,
                profile: contracts::CREATE_PROJECT_DIGEST_PROFILE.to_owned(),
                value_hex_lowercase: digest_hex,
            },
            idempotency_key: idempotency_key.to_owned(),
            producer_cause: contracts::DomainReceiptProducerCause::AuthorCommandAdmission,
            author_command_admission_id: settlement.ids.author_command_admission_id,
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
            result: contracts::DomainReceiptResult::AuthoritativeApplied,
            created_at: settlement.receipt_created_at,
        },
        project: contracts::ControlledProject {
            project_id: scope.project_id.as_ref().to_owned(),
            title: settlement.title,
            open: contracts::ProjectOpenState::Empty,
        },
    }))
}

fn create_project_error(error: CreateProjectError) -> ApiError {
    match error {
        CreateProjectError::BindingConflict => problem(
            StatusCode::CONFLICT,
            "idempotency_binding_conflict",
            "The Create Project binding conflicts.",
        ),
        CreateProjectError::InvalidChallenge => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "challenge_invalid",
            "The Create Project challenge is invalid.",
        ),
        CreateProjectError::ExistingProject => problem(
            StatusCode::CONFLICT,
            "project_exists",
            "The prospective Project already exists.",
        ),
        CreateProjectError::Unavailable(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "project_store_unavailable",
            "The Project store is unavailable.",
        ),
    }
}
