//! Scripted unknown creates and the results that their references return later.

use storyos_application::{
    ModelResponse, ModelUsage, Observation, ReferenceRetrieval, ReportedBinding, ResponseReference,
    RetrievePurpose, RetrieveRequest,
};
use storyos_core::{
    ADVISORY_TEXT, DecisionCandidate, HOST_FAKE_MAPPING_REVISION, ModelOutput, OutputPhase,
    RetrievalBounds, StreamItemState, UnknownCreateScript, unknown_create_script,
};

use crate::create_plan::assistant_item;

const FOREIGN_SCOPE: &str = "018f0000-0000-7000-8000-00000000f033";
const FOREIGN_CONVERSATION: &str = "018f0000-0000-7000-8000-00000000f011";
const FOREIGN_DESTINATION: &str = "018f0000-0000-7000-8000-00000000f022";
const FOREIGN_MAPPING: &str = "storyos.host-fake.mapping.rejected";
const EXPIRED_CONTINUATION: u8 = 5;
const UNUSABLE_CONTINUATION: u8 = 6;

/// The later result that a fake reference encodes in its last group.
#[derive(Clone, Copy)]
enum ReferenceResult {
    Complete = 1,
    Incomplete = 2,
    Unknown = 3,
    LateComplete = 4,
}

/// `Some` when the create is a scripted recovery subject: the reference it reports, if any.
pub(crate) fn unknown_create(
    author_message: &str,
    wire_digest: &str,
) -> Option<Option<ResponseReference>> {
    let mut binding = ReportedBinding {
        owner_user_id: None,
        conversation_id: None,
        destination_identity: None,
        mapping_revision: HOST_FAKE_MAPPING_REVISION.to_owned(),
    };
    let supported = ReferenceRetrieval::Supported {
        bounds: RetrievalBounds::DeclaredReadOnly,
    };
    let (retrieval, result) = match original_result_script(author_message) {
        OriginalResultScript::MissingReference => return Some(None),
        OriginalResultScript::Unsupported => {
            (ReferenceRetrieval::Unsupported, ReferenceResult::Complete)
        }
        OriginalResultScript::UnknownBounds => (
            ReferenceRetrieval::Supported {
                bounds: RetrievalBounds::Unknown,
            },
            ReferenceResult::Complete,
        ),
        OriginalResultScript::ForeignScope => {
            binding.owner_user_id = Some(FOREIGN_SCOPE.to_owned());
            (supported, ReferenceResult::Complete)
        }
        OriginalResultScript::ForeignConversation => {
            binding.conversation_id = Some(FOREIGN_CONVERSATION.to_owned());
            (supported, ReferenceResult::Complete)
        }
        OriginalResultScript::ForeignDestination => {
            binding.destination_identity = Some(FOREIGN_DESTINATION.to_owned());
            (supported, ReferenceResult::Complete)
        }
        OriginalResultScript::ForeignMapping => {
            binding.mapping_revision = FOREIGN_MAPPING.to_owned();
            (supported, ReferenceResult::Complete)
        }
        OriginalResultScript::Incomplete => (supported, ReferenceResult::Incomplete),
        OriginalResultScript::UnknownResult => (supported, ReferenceResult::Unknown),
        OriginalResultScript::CompleteSelected => (supported, ReferenceResult::Complete),
        OriginalResultScript::NotSubject => match unknown_create_script(author_message) {
            UnknownCreateScript::NotSubject => return None,
            UnknownCreateScript::MissingReference => return Some(None),
            UnknownCreateScript::UnsupportedRetrieval => {
                (ReferenceRetrieval::Unsupported, ReferenceResult::Unknown)
            }
            UnknownCreateScript::Late => (supported, ReferenceResult::LateComplete),
            UnknownCreateScript::Once
            | UnknownCreateScript::Budget
            | UnknownCreateScript::UnresolvedEffect
            | UnknownCreateScript::AbsentEffect
            | UnknownCreateScript::ContextChanged
            | UnknownCreateScript::RequestChanged
            | UnknownCreateScript::RouteChanged
            | UnknownCreateScript::Authority => (supported, ReferenceResult::Unknown),
        },
    };
    Some(Some(ResponseReference {
        reference_id: mint_reference(wire_digest, result as u8),
        retrieval,
        reported_binding: binding,
    }))
}

/// A UUID-shaped reference whose last group starts with the later result code.
pub(crate) fn mint_reference(wire_digest: &str, code: u8) -> String {
    let hash = storyos_core::hex_sha256(wire_digest.as_bytes());
    format!(
        "{}-{}-7{}-8{}-{code:02x}{}",
        &hash[0..8],
        &hash[8..12],
        &hash[13..16],
        &hash[17..20],
        &hash[20..30]
    )
}

/// The reference code of a create response that the author message scripts.
pub(crate) fn continuation_code(author_message: &str) -> u8 {
    if author_message.ends_with("SCRIPT:reference-expires") {
        EXPIRED_CONTINUATION
    } else if author_message.ends_with("SCRIPT:reference-unusable") {
        UNUSABLE_CONTINUATION
    } else {
        0
    }
}

/// The rejection of an incremental create whose prior reference the destination lost.
pub(crate) fn continuation_rejection(previous_reference: &str) -> Option<&'static str> {
    match reference_code(previous_reference) {
        Some(EXPIRED_CONTINUATION) => Some("continuation_expired"),
        Some(UNUSABLE_CONTINUATION) => Some("continuation_unusable"),
        _ => None,
    }
}

fn reference_code(reference: &str) -> Option<u8> {
    reference
        .rsplit('-')
        .next()
        .and_then(|group| group.get(0..2))
        .and_then(|code| u8::from_str_radix(code, 16).ok())
}

/// Derives the retrieved result from the reference alone.
pub(crate) fn retrieve(request: &RetrieveRequest) -> Observation {
    let code = reference_code(&request.response_reference);
    let complete = || {
        Observation::Terminal(ModelResponse {
            items: vec![assistant_item(
                "1",
                StreamItemState::Complete,
                ADVISORY_TEXT,
            )],
            output: Some(ModelOutput {
                phase: OutputPhase::FinalAnswer,
                candidate: DecisionCandidate::Advisory {
                    text: ADVISORY_TEXT.to_owned(),
                },
                prose_changes: None,
            }),
            usage: ModelUsage::Unknown,
            response_reference: Some(request.response_reference.clone()),
        })
    };
    let unknown = Observation::OutcomeUnknown {
        response_reference: None,
    };
    match (code, request.purpose) {
        (Some(1), _) | (Some(4), RetrievePurpose::LateResult) => complete(),
        (Some(2), _) => Observation::Terminal(ModelResponse {
            items: vec![assistant_item(
                "1",
                StreamItemState::Incomplete,
                ADVISORY_TEXT,
            )],
            output: None,
            usage: ModelUsage::Unknown,
            response_reference: Some(request.response_reference.clone()),
        }),
        (Some(3 | 4), _) => unknown,
        _ => Observation::Rejected {
            reason: "unknown_reference".to_owned(),
        },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OriginalResultScript {
    NotSubject,
    MissingReference,
    Unsupported,
    UnknownBounds,
    ForeignScope,
    ForeignConversation,
    ForeignMapping,
    ForeignDestination,
    Incomplete,
    UnknownResult,
    CompleteSelected,
}

/// Classify one author message as an original-result retrieval subject.
fn original_result_script(author_message: &str) -> OriginalResultScript {
    match author_message {
        "SCRIPT:retrieve-missing" => OriginalResultScript::MissingReference,
        "SCRIPT:retrieve-unsupported" => OriginalResultScript::Unsupported,
        "SCRIPT:retrieve-unbounded" => OriginalResultScript::UnknownBounds,
        "SCRIPT:retrieve-foreign-scope" => OriginalResultScript::ForeignScope,
        "SCRIPT:retrieve-foreign-conversation" => OriginalResultScript::ForeignConversation,
        "SCRIPT:retrieve-foreign-mapping" => OriginalResultScript::ForeignMapping,
        "SCRIPT:retrieve-foreign-destination" => OriginalResultScript::ForeignDestination,
        "SCRIPT:retrieve-incomplete" => OriginalResultScript::Incomplete,
        "SCRIPT:retrieve-unknown" => OriginalResultScript::UnknownResult,
        "SCRIPT:retrieve-complete" => OriginalResultScript::CompleteSelected,
        _ => OriginalResultScript::NotSubject,
    }
}
