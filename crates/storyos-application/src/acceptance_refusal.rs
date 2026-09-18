#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptanceRefusalBoundary {
    Challenge,
    WriterSession,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptanceRefusalReason {
    StaleWriter,
    SessionChanged,
    InvalidChallenge,
}

/// Safe historical evidence of an Acceptance request refused before Admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptanceRefusal {
    pub refusal_id: String,
    pub correlation_id: String,
    pub reason: AcceptanceRefusalReason,
    pub boundary: AcceptanceRefusalBoundary,
    pub command_schema: String,
    pub refusal_profile_revision: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub limit_profile_revision: String,
    pub challenge_rate_policy_revision: String,
    pub recorded_at: String,
}
