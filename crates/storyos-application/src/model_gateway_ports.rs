//! Ports and values of the Model Gateway dispatch sequence (ADR 0039).

use std::future::Future;

use storyos_core::{
    ModelAdapter, ModelOutput, NativeStreamItem, OrdinaryPassageResolution, RetrievalBounds,
};

use crate::{ClaimedAgentRun, CompleteAgentRun, CompleteAgentRunError};

/// One closed model destination request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DestinationRequest {
    Create(CreateRequest),
    Retrieve(RetrieveRequest),
    Abort(AbortRequest),
}

impl DestinationRequest {
    pub fn attempt(&self) -> &RequestAttempt {
        match self {
            Self::Create(request) => &request.attempt,
            Self::Retrieve(request) => &request.attempt,
            Self::Abort(request) => &request.attempt,
        }
    }

    pub fn route(&self) -> &RequestRoute {
        match self {
            Self::Create(request) => &request.route,
            Self::Retrieve(request) => &request.route,
            Self::Abort(request) => &request.route,
        }
    }
}

/// The adapter of the Model Registration that the AgentRun pinned, and the Credential Reference
/// of its use binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestRoute {
    pub adapter: ModelAdapter,
    pub credential_reference: Option<CredentialReference>,
}

/// Whether a request still needs its dispatch claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RequestAttempt {
    New,
    /// The exchange re-observes an Attempt that a lost claim committed. The adapter must not send
    /// the request again; an adapter that cannot re-observe reports OutcomeUnknown.
    Claimed(DispatchClaim),
}

/// One provider-neutral Create request for the active decision of one claimed AgentRun.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateRequest {
    pub attempt: RequestAttempt,
    pub route: RequestRoute,
    pub author_message: String,
    pub chapter_id: String,
    pub passage_resolution: Option<OrdinaryPassageResolution>,
    pub passage_input: Option<serde_json::Value>,
    pub declared_targets: Vec<DeclaredTarget>,
    pub candidate_revision: Option<String>,
    /// The destination reference of the prior response that an incremental continuation sends.
    pub previous_response_reference: Option<String>,
    /// The unknown predecessor Model Attempt that this one bounded successor replaces.
    pub successor_of: Option<String>,
}

/// One admitted Proposal target that the request declares to the destination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredTarget {
    pub chapter_id: String,
    pub block_id: String,
    pub base_revision_id: String,
    pub collection: bool,
}

impl DeclaredTarget {
    /// The (Chapter, Block, base revision) key that prose changes must match.
    pub fn location(&self) -> (String, String, String) {
        (
            self.chapter_id.clone(),
            self.block_id.clone(),
            self.base_revision_id.clone(),
        )
    }
}

/// One read of a retained response reference. It never sends the original request again.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetrieveRequest {
    pub attempt: RequestAttempt,
    pub route: RequestRoute,
    pub purpose: RetrievePurpose,
    pub original_model_attempt_id: String,
    pub response_reference: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetrievePurpose {
    /// Reconciles an unknown create with its original result.
    OriginalResult,
    /// Looks for the late result of a predecessor after its successor was dispatched.
    LateResult,
}

/// A best-effort provider abort of one cancelled Model Attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbortRequest {
    pub attempt: RequestAttempt,
    pub route: RequestRoute,
    pub ticket: AbortTicket,
    pub response_reference: Option<String>,
}

/// Permission to abort one Model Attempt. The Model Gateway makes it only from a
/// `CommittedCancellation` that the store returned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbortTicket {
    model_attempt_id: String,
}

impl AbortTicket {
    pub fn model_attempt_id(&self) -> &str {
        &self.model_attempt_id
    }
}

/// A durable Model Attempt Cancellation of an in-flight Attempt, as the store reports it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommittedCancellation {
    pub model_attempt_id: String,
    pub response_reference: Option<String>,
    /// The Abort Destination Attempt that a lost claim already committed, if any.
    pub abort_attempt: RequestAttempt,
    pub route: RequestRoute,
}

impl CommittedCancellation {
    pub(crate) fn into_abort(self) -> DestinationRequest {
        DestinationRequest::Abort(AbortRequest {
            attempt: self.abort_attempt,
            route: self.route,
            ticket: AbortTicket {
                model_attempt_id: self.model_attempt_id,
            },
            response_reference: self.response_reference,
        })
    }
}

/// The committed dispatch claim of one Destination Attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DispatchClaim {
    pub model_attempt_id: String,
}

/// The non-secret Wire Payload Projection that one preparation returns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WirePayloadProjection {
    pub execution_profile: String,
    pub mapping_revision: String,
    pub digest: String,
    pub serialized_payload: Option<String>,
}

/// One preparation: the projection to commit, and the single-use value for the exchange.
pub struct PreparedRequest<P> {
    pub projection: WirePayloadProjection,
    pub prepared: P,
}

/// A proven failure before the dispatch claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreDispatchRefusal {
    CredentialUnavailable,
    UnsupportedRequest,
}

/// The complete result of one exchange. It has no error channel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Observation {
    NotSubmitted,
    Rejected {
        reason: String,
    },
    Terminal(ModelResponse),
    OutcomeUnknown {
        response_reference: Option<ResponseReference>,
    },
}

/// A response reference that the destination reported before the result was known.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResponseReference {
    pub reference_id: String,
    pub retrieval: ReferenceRetrieval,
    pub reported_binding: ReportedBinding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceRetrieval {
    Unsupported,
    Supported { bounds: RetrievalBounds },
}

/// The binding that the destination reports for a reference. An absent value reports nothing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReportedBinding {
    pub owner_user_id: Option<String>,
    pub conversation_id: Option<String>,
    pub destination_identity: Option<String>,
    pub mapping_revision: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelResponse {
    pub items: Vec<NativeStreamItem>,
    pub output: Option<ModelOutput>,
    pub usage: ModelUsage,
    pub response_reference: Option<String>,
}

/// Unknown usage is never zero usage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelUsage {
    Reported {
        input_tokens: u64,
        output_tokens: u64,
    },
    Estimated {
        input_tokens: u64,
        output_tokens: u64,
    },
    Unknown,
}

/// What the store returns for one claimed AgentRun.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NextDispatchWork {
    Settled(CompleteAgentRun),
    Request(DestinationRequest),
    Abort(CommittedCancellation),
}

/// Why an exchange must stop at its next durable boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamStop {
    StaleFence,
    Cancelled(CommittedCancellation),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamControl {
    Continue,
    Stop,
}

/// One result that the store records after an exchange or a refusal.
pub enum DispatchRecord<'a> {
    Refusal(PreDispatchRefusal),
    Exchange {
        claim: &'a DispatchClaim,
        request: &'a DestinationRequest,
        observation: Observation,
    },
}

/// Persistence for the Model Gateway sequence. Each operation is one committed transaction
/// with a check of the Run Lease fence.
pub trait ModelDispatchStore: Sync {
    /// Settles the phases that need no destination I/O, then returns the next request.
    fn next_dispatch_work(
        &self,
        claim: &ClaimedAgentRun,
    ) -> impl Future<Output = Result<NextDispatchWork, CompleteAgentRunError>> + Send;

    /// Commits the Destination Attempt, its Wire Payload Projection, and its Outbound
    /// Disclosure Event before I/O. `None` means the work changed; get the next work again.
    fn commit_dispatch_claim(
        &self,
        claim: &ClaimedAgentRun,
        request: &DestinationRequest,
        projection: &WirePayloadProjection,
    ) -> impl Future<Output = Result<Option<DispatchClaim>, CompleteAgentRunError>> + Send;

    /// Commits one ordered batch of Model Stream Events, or tells the exchange to stop.
    fn append_model_stream_events(
        &self,
        claim: &ClaimedAgentRun,
        dispatch: &DispatchClaim,
        events: &[NativeStreamItem],
    ) -> impl Future<Output = Result<Option<StreamStop>, CompleteAgentRunError>> + Send;

    /// Commits one observation or one pre-dispatch refusal.
    fn record(
        &self,
        claim: &ClaimedAgentRun,
        record: DispatchRecord<'_>,
    ) -> impl Future<Output = Result<(), CompleteAgentRunError>> + Send;
}

/// The store-supplied receiver of the Model Stream Events of one exchange.
pub trait ModelStreamSink: Send {
    /// Commits the events; `Stop` tells the adapter to end the exchange.
    fn append(&mut self, events: &[NativeStreamItem])
    -> impl Future<Output = StreamControl> + Send;
}

/// The protocol projection of one destination. It cannot decide retry, fallback, or selection.
pub trait ModelProviderAdapter: Sync {
    /// The adapters of the Model Registrations whose AgentRuns this value can serve.
    const ADAPTERS: &'static [ModelAdapter];

    /// The single-use value of one preparation. It may hold a resolved credential.
    type Prepared: Send;

    /// Maps the request to its wire form, or refuses before the dispatch claim.
    fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> impl Future<Output = Result<PreparedRequest<Self::Prepared>, PreDispatchRefusal>> + Send;

    /// Consumes the prepared value and reports one observation.
    fn exchange(
        &self,
        prepared: Self::Prepared,
        sink: &mut impl ModelStreamSink,
    ) -> impl Future<Output = Observation> + Send;
}

/// The opaque name of one Provider credential.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialReference(pub String);

/// A resolved credential value. It cannot be copied, printed, or serialized.
pub struct ResolvedCredential(String);

impl ResolvedCredential {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

/// Resolves a Credential Reference during preparation, before the dispatch claim.
pub trait CredentialResolver: Sync {
    /// `None` makes the preparation a pre-dispatch refusal.
    fn resolve(
        &self,
        reference: &CredentialReference,
    ) -> impl Future<Output = Option<ResolvedCredential>> + Send;
}

/// One Contract Fault Point that the Model Gateway reaches after a commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContractFaultPoint {
    DispatchClaimed,
    StreamCommitted,
}

/// Receives each reached Contract Fault Point. Production uses `NoContractFaults`.
pub trait ContractFaultObserver: Sync {
    fn reached(&self, point: ContractFaultPoint) -> impl Future<Output = ()> + Send;
}

/// The production observer. It does nothing.
pub struct NoContractFaults;

impl ContractFaultObserver for NoContractFaults {
    async fn reached(&self, _point: ContractFaultPoint) {}
}
