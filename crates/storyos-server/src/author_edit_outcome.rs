use axum::http::HeaderValue;
use storyos_application::{
    ApplyAuthorEditOutcome as ApplicationOutcome,
    ApplyAuthorEditRejectionReason as ApplicationRejectionReason,
    ApplyAuthorEditUnknownObservation as ApplicationUnknownObservation, EditorClientBinding,
    ReadApplyAuthorEditOutcome,
};

use super::author_edit::{AuthorEditResponseIdentity, author_edit_response};
use super::editor_session::{exact_header, session_binding_ref};
use super::project_command_challenge::{plain_digest, valid_uuid_v7};
use super::*;

pub(super) async fn get_apply_author_edit_outcome(
    State(state): State<Arc<ServerState>>,
    Path((project_id, idempotency_key)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<
    (
        HeaderMap,
        Json<contracts::GetApplyAuthorEditOutcomeResponse>,
    ),
    ApiError,
> {
    match read_outcome_response(&state, &headers, &project_id, &idempotency_key).await {
        Ok(response) => {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            Ok((response_headers, Json(response)))
        }
        Err(error) => Err(error.with_no_store()),
    }
}

pub(super) async fn apply_author_edit_outcome_method_not_allowed() -> ApiError {
    problem(
        StatusCode::METHOD_NOT_ALLOWED,
        "method_not_allowed",
        "The request method is not allowed.",
    )
    .with_no_store()
}

async fn read_outcome_response(
    state: &ServerState,
    headers: &HeaderMap,
    project_id: &str,
    idempotency_key: &str,
) -> Result<contracts::GetApplyAuthorEditOutcomeResponse, ApiError> {
    let scope = authenticate_scope(
        state,
        headers,
        project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    let nonce = exact_header(headers, "x-storyos-anti-forgery")?;
    if !valid_uuid_v7(idempotency_key)
        || nonce.len() != 64
        || !nonce
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid_request());
    }
    let session_handle = session_cookie(headers).ok_or_else(authentication_required)?;
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
    let store = project_reader(state)?;
    let outcome = storyos_application::get_apply_author_edit_outcome(
        &store,
        &ReadApplyAuthorEditOutcome {
            project_scope: scope.clone(),
            client_binding: EditorClientBinding {
                binding_ref: session_binding_ref(secret, session_handle),
                session_generation: session_binding.session_generation,
                client_contract_revision: session_binding.client_contract_revision.clone(),
                security_policy_revision: session_binding.security_policy_revision.clone(),
            },
            limit_profile_revision: contracts::LIMIT_PROFILE_REVISION.to_owned(),
            idempotency_key: idempotency_key.to_owned(),
            nonce_digest: plain_digest(nonce.as_bytes()),
        },
    )
    .await
    .map_err(|_| resource_unavailable())?;
    let outcome = contract_outcome(&scope, outcome)?;
    Ok(contracts::GetApplyAuthorEditOutcomeResponse {
        schema_id: contracts::GET_APPLY_AUTHOR_EDIT_OUTCOME_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: Uuid::now_v7().to_string(),
        project_scope: contract_scope(&scope),
        outcome,
    })
}

fn contract_outcome(
    scope: &ApplicationScope,
    outcome: ApplicationOutcome,
) -> Result<contracts::ApplyAuthorEditOutcome, ApiError> {
    Ok(match outcome {
        ApplicationOutcome::Committed(committed) => contracts::ApplyAuthorEditOutcome::Committed {
            response: Box::new(author_edit_response(
                scope,
                AuthorEditResponseIdentity {
                    correlation_id: committed.correlation_id,
                    canonical_command_digest: committed.canonical_command_digest,
                    idempotency_key: committed.idempotency_key,
                    expected_authoritative_revision_id: committed
                        .expected_authoritative_revision_id,
                },
                committed.settlement,
            )?),
        },
        ApplicationOutcome::Rejected { reason } => contracts::ApplyAuthorEditOutcome::Rejected {
            reason: match reason {
                ApplicationRejectionReason::ChallengeExpiredUnconsumed => {
                    contracts::ApplyAuthorEditRejectionReason::ChallengeExpiredUnconsumed
                }
            },
        },
        ApplicationOutcome::StillUnknown { observation } => {
            contracts::ApplyAuthorEditOutcome::StillUnknown {
                observation: match observation {
                    ApplicationUnknownObservation::ChallengeIssued { expires_at } => {
                        contracts::ApplyAuthorEditUnknownObservation::ChallengeIssued { expires_at }
                    }
                    ApplicationUnknownObservation::AdmissionCommitted {
                        command_id,
                        author_command_admission_id,
                    } => contracts::ApplyAuthorEditUnknownObservation::AdmissionCommitted {
                        command_id,
                        author_command_admission_id,
                        reconciliation_required: contracts::ReconciliationRequired,
                    },
                },
            }
        }
    })
}

#[cfg(test)]
#[path = "author_edit_outcome_tests.rs"]
mod tests;
