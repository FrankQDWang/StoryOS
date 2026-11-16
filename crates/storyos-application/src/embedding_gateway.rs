//! The Model Gateway function of one embedding request (ADR 0039, ADR 0095 section 3): prepare,
//! commit the dispatch claim, exchange with no open transaction, and record. An embedding
//! request is not a Run. No retry and no fallback: a stopped process leaves the Embedding
//! Attempt in OutcomeUnknown, and the caller makes a new request with a new attempt.

use std::future::Future;

use storyos_core::{EmbeddingInput, EmbeddingRefusal, EmbeddingRegistration, EmbeddingSettlement};

use crate::{DiagnosticField, ProjectReadError, ProjectScope};

/// The non-secret Wire Payload Projection of one embedding request. It holds no copy of the text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmbeddingProjection {
    pub mapping_revision: &'static str,
    pub digest: String,
}

/// One preparation: the projection to commit, and the single-use value for the exchange.
pub struct PreparedEmbeddingRequest<P> {
    pub projection: EmbeddingProjection,
    pub prepared: P,
}

/// The one refusal of a preparation: the adapter cannot resolve the Credential Reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CredentialUnavailable;

/// The usage of one exchange. Unknown usage is never zero usage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbeddingUsage {
    Reported { input_tokens: u64 },
    Unknown,
}

/// The complete result of one embedding exchange. It has no error channel.
#[derive(Clone, Debug, PartialEq)]
pub enum EmbeddingObservation {
    Vector {
        values: Vec<f32>,
        usage: EmbeddingUsage,
    },
    Rejected {
        reason: String,
    },
    NotSubmitted,
    OutcomeUnknown,
}

/// The committed dispatch claim of one Embedding Attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmbeddingClaim {
    pub embedding_attempt_id: String,
}

/// What the record step commits for one exchange.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmbeddingRecord<'a> {
    pub settlement: EmbeddingSettlement,
    pub usage: EmbeddingUsage,
    pub rejection_reason: Option<&'a str>,
}

/// The result that the caller gets. Only an admitted exchange with a vector of the dimension of
/// the Registration gives a vector.
#[derive(Clone, Debug, PartialEq)]
pub enum EmbeddingResult {
    Vector(Vec<f32>),
    Refused(EmbeddingRefusal),
    Settled(EmbeddingSettlement),
}

/// Persistence of the embedding sequence. Each operation is one committed transaction.
pub trait EmbeddingDispatchStore: Sync {
    /// Loads the admission facts, decides the admission with the Core function, and commits the
    /// Embedding Attempt with the Outbound Disclosure Event in OutcomeUnknown, before I/O. A
    /// refusal writes nothing.
    fn commit_embedding_claim(
        &self,
        scope: &ProjectScope,
        registration: Option<&'static EmbeddingRegistration>,
        input: &EmbeddingInput,
        projection: &EmbeddingProjection,
    ) -> impl Future<Output = Result<Result<EmbeddingClaim, EmbeddingRefusal>, ProjectReadError>> + Send;

    /// Settles the Embedding Attempt with the observation and the usage.
    fn record_embedding(
        &self,
        scope: &ProjectScope,
        claim: &EmbeddingClaim,
        record: &EmbeddingRecord<'_>,
    ) -> impl Future<Output = Result<(), ProjectReadError>> + Send;
}

/// The protocol projection of one embedding destination. It cannot decide retry or authority.
pub trait EmbeddingProviderAdapter: Sync {
    /// The single-use value of one preparation. It may hold a resolved credential.
    type Prepared: Send;

    /// Maps the one text to its wire form and resolves the credential, or refuses before the
    /// dispatch claim.
    fn prepare(
        &self,
        registration: &'static EmbeddingRegistration,
        input: &EmbeddingInput,
    ) -> impl Future<
        Output = Result<PreparedEmbeddingRequest<Self::Prepared>, CredentialUnavailable>,
    > + Send;

    /// Consumes the prepared value and reports one observation.
    fn exchange(
        &self,
        prepared: Self::Prepared,
    ) -> impl Future<Output = EmbeddingObservation> + Send;
}

/// The embedding destination of a process with no embedding adapter. Each request refuses before
/// the claim.
pub struct NoEmbeddingDestination;

impl EmbeddingProviderAdapter for NoEmbeddingDestination {
    type Prepared = std::convert::Infallible;

    async fn prepare(
        &self,
        _registration: &'static EmbeddingRegistration,
        _input: &EmbeddingInput,
    ) -> Result<PreparedEmbeddingRequest<Self::Prepared>, CredentialUnavailable> {
        Err(CredentialUnavailable)
    }

    async fn exchange(&self, prepared: Self::Prepared) -> EmbeddingObservation {
        match prepared {}
    }
}

/// Sends one embedding request of the Project of `scope` in the order of ADR 0039. `registration`
/// is the embedding configuration of the deployment; `None` refuses with
/// `embedding_not_configured`.
#[tracing::instrument(skip_all, fields(
    project_id = scope.project_id.diagnostic(),
    request_kind = input.cause().as_str().diagnostic(),
    adapter = registration.map_or("none", |registration| registration.adapter.as_str())
        .diagnostic(),
    observation = tracing::field::Empty,
    usage = tracing::field::Empty,
    input_tokens = tracing::field::Empty,
))]
pub async fn dispatch_embedding(
    store: &impl EmbeddingDispatchStore,
    adapter: &impl EmbeddingProviderAdapter,
    registration: Option<&'static EmbeddingRegistration>,
    scope: &ProjectScope,
    input: &EmbeddingInput,
) -> Result<EmbeddingResult, ProjectReadError> {
    let refused = |refusal: EmbeddingRefusal| {
        tracing::Span::current().record("observation", refusal.code());
        Ok(EmbeddingResult::Refused(refusal))
    };
    let Some(configured) = registration else {
        return refused(EmbeddingRefusal::NotConfigured);
    };
    let prepared = match adapter.prepare(configured, input).await {
        Ok(prepared) => prepared,
        Err(CredentialUnavailable) => return refused(EmbeddingRefusal::CredentialUnavailable),
    };
    let claim = match store
        .commit_embedding_claim(scope, registration, input, &prepared.projection)
        .await?
    {
        Ok(claim) => claim,
        Err(refusal) => return refused(refusal),
    };
    let observation = adapter.exchange(prepared.prepared).await;
    let (record, vector) = settle(configured, &observation);
    let span = tracing::Span::current();
    span.record("observation", record.settlement.as_str());
    match record.usage {
        EmbeddingUsage::Reported { input_tokens } => {
            span.record("usage", "reported");
            span.record("input_tokens", input_tokens.diagnostic());
        }
        EmbeddingUsage::Unknown => {
            span.record("usage", "unknown");
        }
    }
    store.record_embedding(scope, &claim, &record).await?;
    Ok(vector.map_or(
        EmbeddingResult::Settled(record.settlement),
        EmbeddingResult::Vector,
    ))
}

/// The record of one observation, and the vector that the caller gets.
fn settle<'a>(
    registration: &EmbeddingRegistration,
    observation: &'a EmbeddingObservation,
) -> (EmbeddingRecord<'a>, Option<Vec<f32>>) {
    let record = |settlement, usage, rejection_reason| EmbeddingRecord {
        settlement,
        usage,
        rejection_reason,
    };
    match observation {
        EmbeddingObservation::Vector { values, usage }
            if values.len() == registration.dimensions =>
        {
            (
                record(EmbeddingSettlement::Vector, *usage, None),
                Some(values.clone()),
            )
        }
        EmbeddingObservation::Vector { usage, .. } => (
            record(EmbeddingSettlement::InvalidResult, *usage, None),
            None,
        ),
        EmbeddingObservation::Rejected { reason } => (
            record(
                EmbeddingSettlement::Rejected,
                EmbeddingUsage::Unknown,
                Some(reason.as_str()),
            ),
            None,
        ),
        EmbeddingObservation::NotSubmitted => (
            record(
                EmbeddingSettlement::NotSubmitted,
                EmbeddingUsage::Unknown,
                None,
            ),
            None,
        ),
        EmbeddingObservation::OutcomeUnknown => (
            record(
                EmbeddingSettlement::OutcomeUnknown,
                EmbeddingUsage::Unknown,
                None,
            ),
            None,
        ),
    }
}
