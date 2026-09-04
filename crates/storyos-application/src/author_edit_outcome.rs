use std::future::Future;

use crate::{AuthorEditSettlement, EditorClientBinding, ProjectScope};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolveApplyAuthorEditOutcome {
    pub project_scope: ProjectScope,
    pub client_binding: EditorClientBinding,
    pub limit_profile_revision: String,
    pub idempotency_key: String,
    pub nonce_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommittedApplyAuthorEdit {
    pub correlation_id: String,
    pub canonical_command_digest: String,
    pub idempotency_key: String,
    pub expected_authoritative_revision_id: String,
    pub settlement: AuthorEditSettlement,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplyAuthorEditRejectionReason {
    ChallengeExpiredUnconsumed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplyAuthorEditUnknownObservation {
    ChallengeIssued {
        expires_at: String,
    },
    AdmissionCommitted {
        command_id: String,
        author_command_admission_id: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplyAuthorEditReconfirmationReason {
    AdmissionExpired,
    BindingChanged,
    DirectEditIntentUnrecoverable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequiresReconfirmationApplyAuthorEdit {
    pub command_id: String,
    pub author_command_admission_id: String,
    pub reconfirmation_reason: ApplyAuthorEditReconfirmationReason,
    pub recovery_draft_ref: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplyAuthorEditOutcome {
    Committed(Box<CommittedApplyAuthorEdit>),
    Rejected {
        reason: ApplyAuthorEditRejectionReason,
    },
    RequiresReconfirmation(RequiresReconfirmationApplyAuthorEdit),
    StillUnknown {
        observation: ApplyAuthorEditUnknownObservation,
    },
}

#[derive(Debug)]
pub struct ApplyAuthorEditOutcomeResolveError {
    source: Box<dyn std::error::Error + Send + Sync>,
}

impl ApplyAuthorEditOutcomeResolveError {
    pub fn unavailable(source: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self {
            source: Box::new(source),
        }
    }
}

impl std::fmt::Display for ApplyAuthorEditOutcomeResolveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Apply Author Edit outcome resolution is unavailable")
    }
}

impl std::error::Error for ApplyAuthorEditOutcomeResolveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

/// Resolves one durable Apply Author Edit outcome and may complete same-Admission recovery.
pub trait ApplyAuthorEditOutcomeResolver: Sync {
    fn resolve_apply_author_edit_outcome(
        &self,
        query: &ResolveApplyAuthorEditOutcome,
    ) -> impl Future<Output = Result<ApplyAuthorEditOutcome, ApplyAuthorEditOutcomeResolveError>> + Send;
}

pub async fn get_apply_author_edit_outcome(
    resolver: &impl ApplyAuthorEditOutcomeResolver,
    query: &ResolveApplyAuthorEditOutcome,
) -> Result<ApplyAuthorEditOutcome, ApplyAuthorEditOutcomeResolveError> {
    resolver.resolve_apply_author_edit_outcome(query).await
}

#[cfg(test)]
#[path = "author_edit_outcome_tests.rs"]
mod tests;
