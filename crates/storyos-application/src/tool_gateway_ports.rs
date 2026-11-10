//! Ports and values of the Tool Gateway dispatch sequence: the store port pattern of ADR 0039 for
//! the Tool destination (ADR 0083, ADR 0084).

use std::future::Future;

use serde_json::{Map, Value};
use storyos_core::{RegisteredTool, ToolObservation, ToolRegistration};

use crate::{
    ClaimedAgentRun, CompleteAgentRunError, CredentialReference, PreDispatchRefusal,
    PreparedRequest, WirePayloadProjection,
};

/// One pending ToolCall of the durable Decision, in item order, with its pinned contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolCallRequest {
    pub tool_call_id: String,
    pub registration: &'static ToolRegistration,
    pub tool: &'static RegisteredTool,
    /// The endpoint of the evidence of the admitted Processing Destination Identity.
    pub endpoint: String,
    /// The validated typed arguments, mapped to the argument names of the server tool.
    pub server_arguments: Map<String, Value>,
    /// The Credential Reference of the current use binding. `None` refuses before the claim.
    pub credential_reference: Option<CredentialReference>,
    /// The Context Assembly Manifest that the store committed for this preparation (ADR 0005
    /// gate six). The dispatch claim refers to it.
    pub context_assembly_manifest_id: String,
}

/// The committed dispatch claim of one Tool Destination Attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolDispatchClaim {
    pub destination_attempt_id: String,
}

/// One result that the store records after a Tool exchange or a refusal before the claim.
pub enum ToolDispatchRecord<'a> {
    Refusal {
        request: &'a ToolCallRequest,
        refusal: PreDispatchRefusal,
    },
    Exchange {
        claim: &'a ToolDispatchClaim,
        request: &'a ToolCallRequest,
        observation: ToolObservation,
    },
}

/// Persistence for the Tool Gateway sequence. Each operation is one committed transaction with a
/// check of the Run Lease fence.
pub trait ToolDispatchStore: Sync {
    /// The next pending ToolCall of the durable Decision of the AgentRun, or `None` when each
    /// ToolCall is resolved.
    fn next_tool_call(
        &self,
        claim: &ClaimedAgentRun,
    ) -> impl Future<Output = Result<Option<ToolCallRequest>, CompleteAgentRunError>> + Send;

    /// Checks the current authority again, then commits the Destination Attempt Admission
    /// Decision, the Wire Payload Projection, the Destination Attempt, and the Outbound
    /// Disclosure Event in OutcomeUnknown before I/O. `None` means a refusal that the store
    /// recorded, or changed work; get the next ToolCall again.
    fn commit_tool_dispatch_claim(
        &self,
        claim: &ClaimedAgentRun,
        request: &ToolCallRequest,
        projection: &WirePayloadProjection,
    ) -> impl Future<Output = Result<Option<ToolDispatchClaim>, CompleteAgentRunError>> + Send;

    /// Commits one observation or one refusal before the claim.
    fn record_tool_call(
        &self,
        claim: &ClaimedAgentRun,
        record: ToolDispatchRecord<'_>,
    ) -> impl Future<Output = Result<(), CompleteAgentRunError>> + Send;
}

/// The protocol projection of one Tool destination. It cannot decide retry or authority.
pub trait ToolDestinationAdapter: Sync {
    /// The single-use value of one preparation. It may hold a resolved credential.
    type Prepared: Send;

    /// Maps the request to its wire form and resolves its credential, or refuses before the
    /// dispatch claim.
    fn prepare(
        &self,
        request: &ToolCallRequest,
    ) -> impl Future<Output = Result<PreparedRequest<Self::Prepared>, PreDispatchRefusal>> + Send;

    /// Consumes the prepared value and reports one observation.
    fn exchange(&self, prepared: Self::Prepared) -> impl Future<Output = ToolObservation> + Send;
}

/// The Tool destination of a process with no Tool adapter, such as the in-process Worker of the
/// Server. Each request refuses before the claim.
pub struct NoToolDestination;

impl ToolDestinationAdapter for NoToolDestination {
    type Prepared = std::convert::Infallible;

    async fn prepare(
        &self,
        _request: &ToolCallRequest,
    ) -> Result<PreparedRequest<Self::Prepared>, PreDispatchRefusal> {
        Err(PreDispatchRefusal::UnsupportedRequest)
    }

    async fn exchange(&self, prepared: Self::Prepared) -> ToolObservation {
        match prepared {}
    }
}

/// Implements `ToolDispatchStore` for a test store whose Runs have no ToolCall.
#[cfg(test)]
macro_rules! no_tool_calls {
    ($store:ty) => {
        impl $crate::ToolDispatchStore for $store {
            async fn next_tool_call(
                &self,
                _claim: &$crate::ClaimedAgentRun,
            ) -> Result<Option<$crate::ToolCallRequest>, $crate::CompleteAgentRunError> {
                Ok(None)
            }

            async fn commit_tool_dispatch_claim(
                &self,
                _claim: &$crate::ClaimedAgentRun,
                _request: &$crate::ToolCallRequest,
                _projection: &$crate::WirePayloadProjection,
            ) -> Result<Option<$crate::ToolDispatchClaim>, $crate::CompleteAgentRunError> {
                unreachable!("the test Run has no ToolCall")
            }

            async fn record_tool_call(
                &self,
                _claim: &$crate::ClaimedAgentRun,
                _record: $crate::ToolDispatchRecord<'_>,
            ) -> Result<(), $crate::CompleteAgentRunError> {
                unreachable!("the test Run has no ToolCall")
            }
        }
    };
}

#[cfg(test)]
pub(crate) use no_tool_calls;
