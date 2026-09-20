//! Classify one conversation continuation input mapping without I/O.

const FULL_CONTINUATION_INPUT_PREFIX: &str = "Submit the complete current request.";

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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContinuationMappingInput {
    pub current: ContinuationIdentity,
    pub prior: Option<ContinuationIdentity>,
    pub mapping_can_represent: bool,
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
    if prior.covered_copy_restricted || !identities_match(&input.current, prior) {
        return ContinuationInputMapping::NewTransport;
    }
    if input.mapping_can_represent {
        ContinuationInputMapping::Incremental
    } else {
        ContinuationInputMapping::Full
    }
}

fn identities_match(current: &ContinuationIdentity, prior: &ContinuationIdentity) -> bool {
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
