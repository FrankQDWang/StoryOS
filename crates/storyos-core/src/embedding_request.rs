//! The embedding request of the manuscript search (ADR 0095 section 3): its Registration, its
//! one text, and its admission. An embedding request is not a Run. It has no AgentRun, no
//! RunStep, and no Model Attempt.

use std::time::Duration;

use crate::assemble_context::count_context_item_tokens;
use crate::{DestinationKind, GrantScope};

/// The input limit of one text, in tokens of the Token Counting Profile (ADR 0052).
pub const EMBEDDING_TEXT_TOKEN_LIMIT: u64 = 8_192;
/// The timeout of one embedding request.
pub const EMBEDDING_REQUEST_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 30);
/// The length of each vector. A vector of another length is an invalid result.
pub const EMBEDDING_DIMENSIONS: usize = 1_024;
/// The name of the request bounds of each Embedding Registration: one text of at most
/// `EMBEDDING_TEXT_TOKEN_LIMIT` tokens, and `EMBEDDING_REQUEST_TIMEOUT`.
pub const EMBEDDING_REQUEST_BOUNDS: &str = "embedding_request_v1";
/// The Purpose of each embedding request. It is not a Context Purpose.
pub const MANUSCRIPT_EMBEDDING_PURPOSE: &str = "manuscript_embedding";

/// The adapter of an Embedding Registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbeddingAdapter {
    HostFake,
    VolcengineAgentPlanEmbeddings,
}

/// The one global Embedding Registration of a destination. It holds no Project data and no
/// Credential Reference.
#[derive(Debug, Eq, PartialEq)]
pub struct EmbeddingRegistration {
    pub revision: &'static str,
    pub destination: DestinationKind,
    pub adapter: EmbeddingAdapter,
    /// The exact dated model id (ADR 0059).
    pub provider_model_id: &'static str,
    pub mapping_revision: &'static str,
    pub dimensions: usize,
    /// The one input modality. The Registration accepts text only.
    pub input_modality: &'static str,
    pub request_bounds: &'static str,
}

/// The Embedding Registration of the Contract-Faithful Fake Destination.
pub const HOST_FAKE_EMBEDDING_REGISTRATION: EmbeddingRegistration = EmbeddingRegistration {
    revision: "018f0000-0000-7001-8000-0000000eb001",
    destination: DestinationKind::HostFake,
    adapter: EmbeddingAdapter::HostFake,
    provider_model_id: "host-fake-embedding",
    mapping_revision: "storyos.host-fake.embedding.v1",
    dimensions: EMBEDDING_DIMENSIONS,
    input_modality: "text",
    request_bounds: EMBEDDING_REQUEST_BOUNDS,
};

/// The Embedding Registration of the multimodal embedding path of the Agent Plan route
/// (`docs/research/embedding-and-pgvector-probe-2026-11-16.md`).
pub const AGENT_PLAN_EMBEDDING_REGISTRATION: EmbeddingRegistration = EmbeddingRegistration {
    revision: "018f0000-0000-7001-8000-0000000eb002",
    destination: DestinationKind::VolcengineAgentPlan,
    adapter: EmbeddingAdapter::VolcengineAgentPlanEmbeddings,
    provider_model_id: "doubao-embedding-vision-251215",
    mapping_revision: "storyos.volcengine-agent-plan.embeddings.multimodal.v1",
    dimensions: EMBEDDING_DIMENSIONS,
    input_modality: "text",
    request_bounds: EMBEDDING_REQUEST_BOUNDS,
};

impl EmbeddingRegistration {
    /// The Registration of the configured embedding destination.
    pub fn of(destination: DestinationKind) -> &'static Self {
        match destination {
            DestinationKind::HostFake => &HOST_FAKE_EMBEDDING_REGISTRATION,
            DestinationKind::VolcengineAgentPlan => &AGENT_PLAN_EMBEDDING_REGISTRATION,
        }
    }
}

/// The typed cause of one embedding request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbeddingCause {
    /// A passage of a Chapter goes into the Retrieval Index.
    PassageIndex,
    /// A question of a search ToolCall.
    SearchQuestion,
}

impl EmbeddingCause {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PassageIndex => "passage_index",
            Self::SearchQuestion => "search_question",
        }
    }

    /// The source class of the text. It is not a Context source class.
    pub fn source_class(self) -> &'static str {
        match self {
            Self::PassageIndex => "chapter_passage",
            Self::SearchQuestion => "search_question",
        }
    }

    /// The `instructions` of the request. A passage and a question get different text.
    pub fn instructions(self) -> &'static str {
        match self {
            Self::PassageIndex => "Instruction:Compress the text into one word.\nQuery:",
            Self::SearchQuestion => {
                "Target_modality: text.\nInstruction:Find the passage that answers this question\nQuery:"
            }
        }
    }
}

/// The one text of one embedding request and the references of its source. A request holds the
/// text of one Project, the Project of its scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EmbeddingInput {
    Passage {
        chapter_id: String,
        revision_id: String,
        passage_digest: String,
        text: String,
    },
    Question {
        tool_call_id: String,
        text: String,
    },
}

impl EmbeddingInput {
    pub fn cause(&self) -> EmbeddingCause {
        match self {
            Self::Passage { .. } => EmbeddingCause::PassageIndex,
            Self::Question { .. } => EmbeddingCause::SearchQuestion,
        }
    }

    pub fn text(&self) -> &str {
        match self {
            Self::Passage { text, .. } | Self::Question { text, .. } => text,
        }
    }
}

/// A typed refusal of an embedding request. A refusal writes no Destination Attempt and no
/// Outbound Disclosure Event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbeddingRefusal {
    NotConfigured,
    BindingAbsent,
    DisclosureOutsideGrant,
    InputOutOfBounds,
    CredentialUnavailable,
}

impl EmbeddingRefusal {
    pub fn code(self) -> &'static str {
        match self {
            Self::NotConfigured => "embedding_not_configured",
            Self::BindingAbsent => "embedding_binding_absent",
            Self::DisclosureOutsideGrant => "disclosure_outside_grant",
            Self::InputOutOfBounds => "embedding_input_out_of_bounds",
            Self::CredentialUnavailable => "destination_credential_unavailable",
        }
    }
}

/// The current embedding use binding of the Project, as the store loads it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddingBindingFacts<'a> {
    pub registration_revision: &'a str,
    pub grant_scope: Option<&'a str>,
    pub destination_kind: &'a str,
    pub credential_reference: Option<&'a str>,
}

/// The Registration that `createAgentRun` binds in the Project, or `None`. It is `None` when the
/// deployment has no embedding configuration, and when the current binding of the Project has the
/// configured Registration, the scope `manuscript_embedding_v1`, the destination, and the
/// Credential Reference of that destination.
pub fn embedding_binding_to_make(
    configured: Option<&'static EmbeddingRegistration>,
    current: Option<EmbeddingBindingFacts<'_>>,
) -> Option<&'static EmbeddingRegistration> {
    let registration = configured?;
    let wanted = EmbeddingBindingFacts {
        registration_revision: registration.revision,
        grant_scope: Some(GrantScope::ManuscriptEmbeddingV1.as_str()),
        destination_kind: registration.destination.as_str(),
        credential_reference: registration.destination.credential_reference(),
    };
    (current != Some(wanted)).then_some(registration)
}

/// The facts that the store loads in the dispatch-claim transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddingAdmissionFacts<'a> {
    /// The Registration of the embedding configuration of the deployment.
    pub registration: Option<&'static EmbeddingRegistration>,
    pub binding: Option<EmbeddingBindingFacts<'a>>,
    /// Whether each reference of the input is in the Project Scope of the request.
    pub source_in_scope: bool,
    pub input: &'a EmbeddingInput,
}

/// Admits one embedding request, or gives its refusal. The text is never shortened.
pub fn admit_embedding(facts: &EmbeddingAdmissionFacts<'_>) -> Result<(), EmbeddingRefusal> {
    let registration = facts.registration.ok_or(EmbeddingRefusal::NotConfigured)?;
    let binding = facts
        .binding
        .filter(|binding| binding.registration_revision == registration.revision)
        .ok_or(EmbeddingRefusal::BindingAbsent)?;
    let covered = binding
        .grant_scope
        .and_then(GrantScope::parse)
        .is_some_and(|scope| scope.names_embedding(facts.input.cause()));
    if !covered || !facts.source_in_scope {
        return Err(EmbeddingRefusal::DisclosureOutsideGrant);
    }
    let text = facts.input.text();
    if text.is_empty() || count_context_item_tokens(text) > EMBEDDING_TEXT_TOKEN_LIMIT {
        return Err(EmbeddingRefusal::InputOutOfBounds);
    }
    Ok(())
}

#[cfg(test)]
#[path = "embedding_request_tests.rs"]
mod tests;
