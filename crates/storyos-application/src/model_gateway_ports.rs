//! Ports and values of the Model Gateway dispatch sequence (ADR 0039).

use std::future::Future;
use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};

pub use storyos_core::RequestBounds;
use storyos_core::{
    ContextSourceClass, ModelAdapter, ModelOutput, NativeStreamItem, OrdinaryPassageResolution,
    RetrievalBounds, StreamItemState, ToolExposure,
};

use crate::{ClaimedAgentRun, CompleteAgentRun, CompleteAgentRunError};

/// The most provisional Model Stream Events that an adapter sends to the sink in one commit
/// (ADR 0039, ADR 0054). Each terminal item event commits at once.
pub const PROVISIONAL_STREAM_EVENT_BOUND: usize = 64;

/// The longest time between two commits of one streamed exchange, also when no event arrives.
/// Thus the exchange sees a stale fence or a cancellation within this time (ADR 0054).
pub const PROVISIONAL_STREAM_COMMIT_INTERVAL: std::time::Duration =
    std::time::Duration::from_millis(/*millis*/ 500);

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

/// The adapter and exact model of the Model Registration that the AgentRun pinned, and the
/// endpoint, Credential Reference, and request bounds of its use binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestRoute {
    pub adapter: ModelAdapter,
    pub provider_model_id: String,
    /// The wire mapping revision of the pinned Registration.
    pub mapping_revision: String,
    /// The endpoint of the Processing Destination Identity evidence. The fake has none.
    pub endpoint: Option<String>,
    pub credential_reference: Option<CredentialReference>,
    /// `None` is an unknown bound, which blocks a real request.
    pub bounds: Option<RequestBounds>,
}

/// Whether a request still needs its dispatch claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RequestAttempt {
    New,
    /// The exchange re-observes an Attempt that a lost claim committed. The adapter must not send
    /// the request again; an adapter that cannot re-observe reports OutcomeUnknown.
    Claimed(DispatchClaim),
}

/// What one Create request asks for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreatePurpose {
    /// The candidate of the active decision.
    Decision,
    /// The Host-managed summary of the Conversation for an active compaction. The request sends
    /// one Host instruction with the prior response reference, and no project content.
    CompactionSummary,
    /// One author-authorized qualification probe of a pending Tool entry (ADR 0049). The request
    /// sends the constant author message or a constant function output, and no project content.
    /// Its response makes no Agent Decision.
    ToolQualification(ToolProbeRequest),
}

/// The probe of one Tool qualification request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolProbeRequest {
    FunctionCall,
    /// The output of the function call `call_id` of the earlier probe response, which the
    /// request continues with its prior response reference. `None` when the Conversation has no
    /// function call probe: the adapter refuses that request before dispatch.
    FunctionCallOutput {
        call_id: Option<String>,
    },
}

impl ToolProbeRequest {
    pub fn probe(&self) -> storyos_core::ToolProbe {
        match self {
            Self::FunctionCall => storyos_core::ToolProbe::FunctionCall,
            Self::FunctionCallOutput { .. } => storyos_core::ToolProbe::FunctionCallOutput,
        }
    }
}

/// One provider-neutral Create request for the active decision of one claimed AgentRun.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateRequest {
    pub attempt: RequestAttempt,
    pub purpose: CreatePurpose,
    pub route: RequestRoute,
    pub author_message: String,
    pub chapter_id: String,
    pub passage_resolution: Option<OrdinaryPassageResolution>,
    pub passage_input: Option<serde_json::Value>,
    /// The selected items of the active decision Context Assembly, in order.
    pub context: Vec<RequestContextItem>,
    pub declared_targets: Vec<DeclaredTarget>,
    /// The exact text ranges that the author added to the message, in order.
    pub selection_references: Vec<storyos_core::AdmittedSelection>,
    /// The destination reference of the prior response that an incremental continuation sends.
    pub previous_response_reference: Option<String>,
    /// The Tool Exposure of this RunStep. The adapter declares each exposed Tool as one function
    /// and sends no Tool for an empty Exposure (ADR 0084).
    pub tool_exposure: ToolExposure,
    /// The unknown predecessor Model Attempt that this one bounded successor replaces.
    pub successor_of: Option<String>,
    /// The function outputs of a continuation RunStep, in item order: one for each resolved
    /// ToolCall of the Run, from its selected `tool_result` Context item.
    pub tool_outputs: Vec<ToolOutput>,
}

/// One native function call of the Run and its projected output (ADR 0033: the request keeps
/// the native tool-call and result correlation).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolOutput {
    pub call_id: String,
    pub name: String,
    /// The validated arguments of the call as canonical JSON text.
    pub arguments: String,
    /// The bounded `tool_result` projection of the stored ToolCall.
    pub output: String,
    pub origin: ToolOutputOrigin,
}

/// Where the native function call of one output is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolOutputOrigin {
    /// The response that the prior response reference names: an incremental request sends only
    /// the output.
    PriorResponse,
    /// An earlier RunStep of the Run: only a full input sends it again, with its call.
    EarlierStep,
}

/// One selected Context Assembly item that the request sends.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestContextItem {
    pub source_class: ContextSourceClass,
    pub content: String,
}

/// One admitted Proposal target that the request declares to the destination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredTarget {
    pub chapter_id: String,
    pub block_id: String,
    pub base_revision_id: String,
    pub collection: bool,
    pub kind: storyos_core::DeclaredTargetKind,
    /// The text of the block at its base revision, or the text of the pending candidate.
    pub block_text: String,
}

impl CreateRequest {
    /// The input of this request that the model reads, other than the Tool outputs and the Host
    /// instructions.
    pub fn current_input(&self) -> CurrentInput<'_> {
        CurrentInput {
            author_message: &self.author_message,
            chapter_id: &self.chapter_id,
            passage_resolution: self.passage_resolution,
            passage_input: self.passage_input.as_ref(),
            context: &self.context,
            declared_targets: &self.declared_targets,
            selection_references: &self.selection_references,
        }
    }
}

/// The input of a decision request that the model reads, other than the Tool outputs and the
/// Host instructions: the author message, the Chapter, the passage input, each selected Context
/// item that the message sends, each declared target, and each selection reference.
#[derive(Clone, Copy, Debug)]
pub struct CurrentInput<'a> {
    pub author_message: &'a str,
    pub chapter_id: &'a str,
    pub passage_resolution: Option<OrdinaryPassageResolution>,
    pub passage_input: Option<&'a serde_json::Value>,
    pub context: &'a [RequestContextItem],
    pub declared_targets: &'a [DeclaredTarget],
    pub selection_references: &'a [storyos_core::AdmittedSelection],
}

impl CurrentInput<'_> {
    /// One digest over the input. A continuation whose digest differs from that of its prior
    /// request sends a full input (ADR 0085), because the prior response read the earlier input.
    pub fn digest(&self) -> String {
        let context: Vec<serde_json::Value> = self
            .context
            .iter()
            .filter(|item| {
                !matches!(
                    item.source_class,
                    ContextSourceClass::HostControl | ContextSourceClass::ToolResult
                )
            })
            .map(|item| serde_json::json!([item.source_class.name(), item.content]))
            .collect();
        let targets: Vec<serde_json::Value> = self
            .declared_targets
            .iter()
            .map(|target| {
                let kind = match target.kind {
                    storyos_core::DeclaredTargetKind::ChapterBlock => {
                        serde_json::json!("chapter_block")
                    }
                    storyos_core::DeclaredTargetKind::PendingCandidate => {
                        serde_json::json!("pending_candidate")
                    }
                    storyos_core::DeclaredTargetKind::JoinableBlock { distance } => {
                        serde_json::json!({ "joinable_block": distance })
                    }
                };
                serde_json::json!([
                    target.chapter_id,
                    target.block_id,
                    target.base_revision_id,
                    target.collection,
                    kind,
                    target.block_text,
                ])
            })
            .collect();
        // The passage input repeats the selected items with their source versions, which are new
        // in each RunStep. The Context items above carry their text.
        let passage_input = self.passage_input.cloned().map(|mut input| {
            if let Some(object) = input.as_object_mut() {
                object.remove("selected");
            }
            input
        });
        let text = storyos_core::canonical_json(&serde_json::json!({
            "author_message": self.author_message,
            "chapter_id": self.chapter_id,
            "passage_resolution": self.passage_resolution,
            "passage_input": passage_input,
            "context": context,
            "declared_targets": targets,
            "selection_references": self.selection_references,
        }));
        format!("sha256:{}", storyos_core::hex_sha256(text.as_bytes()))
    }
}

impl DeclaredTarget {
    /// The location that prose changes must match.
    pub fn location(&self) -> storyos_core::DeclaredLocation {
        storyos_core::DeclaredLocation {
            chapter_id: self.chapter_id.clone(),
            block_id: self.block_id.clone(),
            base_revision_id: self.base_revision_id.clone(),
            kind: self.kind,
        }
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
    /// The Proposal targets that the original Create declared, so that its result maps the same.
    pub declared_targets: Vec<DeclaredTarget>,
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
    /// The cache setting of a real request. The fake destination has no cache.
    pub cache_setting: Option<CacheSetting>,
}

/// The cache that a request asks for. A setting proves no cache hit and no saving.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheSetting {
    /// The destination applies its default implicit cache. The request asks for no explicit cache.
    DestinationDefault,
}

/// The cache use that the destination reports for one response. Unknown is never zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheUse {
    Reported { cached_input_tokens: u64 },
    Unknown,
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
    /// The terminal state of the whole response: `Complete`, `Incomplete`, `Failed`, or
    /// `Cancelled`. An item keeps its own state.
    pub state: StreamItemState,
    pub items: Vec<NativeStreamItem>,
    pub output: Option<ModelOutput>,
    pub usage: ModelUsage,
    pub cache_use: CacheUse,
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
    Request(Box<DestinationRequest>),
    Abort(CommittedCancellation),
    /// The durable Decision has pending ToolCalls: the ToolDispatch phase dispatches them.
    ToolCalls,
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
    Refusal {
        refusal: PreDispatchRefusal,
        request: &'a DestinationRequest,
    },
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

    /// Commits one ordered batch of Model Stream Events, or tells the exchange to stop. A
    /// `reference` is the response reference that the destination reported for the Attempt.
    fn append_model_stream_events(
        &self,
        claim: &ClaimedAgentRun,
        dispatch: &DispatchClaim,
        events: &[NativeStreamItem],
        reference: Option<&ResponseReference>,
    ) -> impl Future<Output = Result<Option<StreamStop>, CompleteAgentRunError>> + Send;

    /// Extends the Run Lease of the fenced claim. Returns the delay until the next renewal, or
    /// `None` when the claim has lost its lease.
    fn renew_agent_run_lease(
        &self,
        claim: &ClaimedAgentRun,
    ) -> impl Future<Output = Result<Option<Duration>, CompleteAgentRunError>> + Send;

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

    /// Commits the response reference of the exchange as soon as the destination reports it, so
    /// that a restart can still retrieve the original result.
    fn retain_reference(
        &mut self,
        reference: &ResponseReference,
    ) -> impl Future<Output = StreamControl> + Send;
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
pub struct ResolvedCredential(SecretString);

impl ResolvedCredential {
    pub fn new(value: impl Into<SecretString>) -> Self {
        Self(value.into())
    }

    /// The value, at the place that sends it.
    pub fn expose(&self) -> &str {
        self.0.expose_secret()
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
    /// The store recorded the result of a compaction summary request.
    CompactionStaged,
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

/// The time source of the Model Gateway. The Worker waits on the runtime timer. A test supplies
/// a Deterministic Test Scheduler, so that a retry delay or a lease renewal is a test step.
pub trait GatewayClock: Sync {
    /// Completes when `delay` has elapsed.
    fn wait(&self, delay: Duration) -> impl Future<Output = ()> + Send;
}
