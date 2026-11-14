//! Classify one conversation continuation input mapping without I/O.

const FULL_CONTINUATION_INPUT_PREFIX: &str = "Submit the complete current request.";

/// Whether the prior response of an incremental continuation read the current input of the
/// request: the selected Context text, the declared targets, the selection references, and the
/// passage input (ADR 0085).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PriorInput {
    /// The prior response read the current input, or the request sends no prior response.
    Current,
    /// The current input changed after the prior response read it. The incremental request sends
    /// the current input again before the Tool outputs, and the input says that it replaces the
    /// earlier input.
    Replaced,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContinuationInputMapping {
    None,
    Incremental,
    Full,
    NewTransport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContinuationIdentity {
    pub owner_user_id: String,
    pub project_id: String,
    pub conversation_id: String,
    pub destination_identity: String,
    pub evidence_revision: String,
    pub registration: String,
    pub adapter_mapping: String,
    pub use_binding: String,
    pub compatibility: String,
    pub covered_copy_restricted: bool,
    /// The digest of the Tool Exposure of the Attempt. A changed Exposure starts a new
    /// transport continuation with the full input (ADR 0033). It is not part of the processing
    /// boundary of `continuation_boundary_matches`.
    pub tool_exposure_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContinuationMappingInput {
    pub current: ContinuationIdentity,
    pub prior: Option<ContinuationIdentity>,
    pub mapping_can_represent: bool,
    /// The destination confirmed that the prior reference expired or is unusable.
    pub prior_reference_unusable: bool,
}

/// Ordinary author text can use a delta plus an eligible prior reference.
pub fn continuation_mapping_can_represent(author_message: &str) -> bool {
    !author_message.starts_with(FULL_CONTINUATION_INPUT_PREFIX)
}

/// Classify incremental, full, or new-transport input for one current admission.
pub fn map_continuation_input(input: &ContinuationMappingInput) -> ContinuationInputMapping {
    let Some(prior) = input.prior.as_ref() else {
        return ContinuationInputMapping::None;
    };
    // A changed Tool Exposure keeps the processing boundary and changes only the transport: the
    // request sends the full input with no prior reference (ADR 0033, ADR 0084).
    if prior.covered_copy_restricted
        || !continuation_boundary_matches(&input.current, prior)
        || input.current.tool_exposure_digest != prior.tool_exposure_digest
    {
        return ContinuationInputMapping::NewTransport;
    }
    // An incremental request sends only the new items, so it needs a usable prior reference.
    // Otherwise the request sends the full input (ADR 0033). A changed current input is a new
    // item of an incremental request (ADR 0085).
    if input.mapping_can_represent && !input.prior_reference_unusable {
        ContinuationInputMapping::Incremental
    } else {
        ContinuationInputMapping::Full
    }
}

/// Same Scope, conversation, destination, registration, mapping, and use boundary: the
/// processing boundary. The Tool Exposure is not part of it, so a changed Exposure permits the
/// full input under the current admission.
pub fn continuation_boundary_matches(
    current: &ContinuationIdentity,
    prior: &ContinuationIdentity,
) -> bool {
    current.owner_user_id == prior.owner_user_id
        && current.project_id == prior.project_id
        && current.conversation_id == prior.conversation_id
        && current.destination_identity == prior.destination_identity
        && current.evidence_revision == prior.evidence_revision
        && current.registration == prior.registration
        && current.adapter_mapping == prior.adapter_mapping
        && current.use_binding == prior.use_binding
        && current.compatibility == prior.compatibility
}

#[cfg(test)]
#[path = "continuation_input_tests.rs"]
mod tests;
