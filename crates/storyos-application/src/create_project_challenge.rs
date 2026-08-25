use std::future::Future;

use crate::{ProjectCommandChallengeError, ProjectId, UserId};

/// Exact User-level bindings for one prospective Create Project challenge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateProjectChallengeBinding {
    pub owner_user_id: UserId,
    pub prospective_project_id: ProjectId,
    pub client_session_binding_digest: String,
    pub client_session_generation: u64,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub limit_profile_revision: String,
    pub method: String,
    pub route_template: String,
    pub command_schema: String,
    pub command_kind: String,
    pub create_input_digest: String,
    pub canonical_command_digest: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateProjectChallenge {
    pub prospective_project_id: ProjectId,
    pub canonical_command_digest: String,
    pub expires_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueCreateProjectChallenge {
    pub binding: CreateProjectChallengeBinding,
    pub nonce_digest: String,
}

/// Owns User-level prospective Create Project challenge issuance.
pub trait CreateProjectChallengeStore: Sync {
    fn issue(
        &self,
        request: &IssueCreateProjectChallenge,
    ) -> impl Future<Output = Result<CreateProjectChallenge, ProjectCommandChallengeError>> + Send;
}

pub async fn issue_create_project_challenge(
    store: &impl CreateProjectChallengeStore,
    request: &IssueCreateProjectChallenge,
) -> Result<CreateProjectChallenge, ProjectCommandChallengeError> {
    store.issue(request).await
}
