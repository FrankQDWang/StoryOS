use std::future::Future;

use crate::ProjectScope;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorCommandOutcomeUnknownBoundary {
    AdmissionCommitted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorCommandOutcomeUnknownReason {
    AcknowledgementMissing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReconciliationRequired;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppendAuthorCommandOutcomeUnknown {
    pub project_scope: ProjectScope,
    pub author_command_admission_id: String,
    pub observation_id: String,
    pub last_provable_boundary: AuthorCommandOutcomeUnknownBoundary,
    pub reason: AuthorCommandOutcomeUnknownReason,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorCommandOutcomeUnknownObservation {
    pub project_scope: ProjectScope,
    pub observation_id: String,
    pub observation_sequence: u64,
    pub author_command_admission_id: String,
    pub command_id: String,
    pub command_kind: String,
    pub canonical_command_digest: String,
    pub idempotency_key: String,
    pub last_provable_boundary: AuthorCommandOutcomeUnknownBoundary,
    pub reason: AuthorCommandOutcomeUnknownReason,
    pub reconciliation_required: ReconciliationRequired,
    pub observed_at: String,
}

#[derive(Debug)]
pub enum AuthorCommandOutcomeUnknownError {
    BindingConflict,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl AuthorCommandOutcomeUnknownError {
    pub fn unavailable(source: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Unavailable(Box::new(source))
    }
}

impl std::fmt::Display for AuthorCommandOutcomeUnknownError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::BindingConflict => "The Author Command observation conflicts",
            Self::Unavailable(_) => "The Author Command observation store is unavailable",
        })
    }
}

impl std::error::Error for AuthorCommandOutcomeUnknownError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BindingConflict => None,
            Self::Unavailable(source) => Some(source.as_ref()),
        }
    }
}

/// Appends or reads one immutable Admission outcome-unknown observation.
pub trait AuthorCommandOutcomeUnknownStore: Sync {
    fn append_author_command_outcome_unknown(
        &self,
        request: &AppendAuthorCommandOutcomeUnknown,
    ) -> impl Future<
        Output = Result<AuthorCommandOutcomeUnknownObservation, AuthorCommandOutcomeUnknownError>,
    > + Send;
}

pub async fn append_author_command_outcome_unknown(
    store: &impl AuthorCommandOutcomeUnknownStore,
    request: &AppendAuthorCommandOutcomeUnknown,
) -> Result<AuthorCommandOutcomeUnknownObservation, AuthorCommandOutcomeUnknownError> {
    store.append_author_command_outcome_unknown(request).await
}
