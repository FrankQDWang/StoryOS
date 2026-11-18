//! The Memory rules of the mapping `v9` (ADR 0094 sections 5 and 6). Each text is the text of the
//! mapping `v8` with the literal sentences of its rule, so that a Run that pinned `v8` keeps its
//! request form (ADR 0093 section 3).

use serde_json::json;
use storyos_application::{CreatePurpose, CreateRequest};
use storyos_core::{
    AgentRunKind, CaptureInputKind, ContextSourceClass, ExposureFacts, ToolChoice, ToolExposure,
};

use crate::skill_wire_tests::{body, first, item};

const V8: &str = storyos_core::AGENT_PLAN_CONTINUITY_MAPPING_REVISION;
const V9: &str = storyos_core::AGENT_PLAN_MEMORY_RULES_MAPPING_REVISION;

/// A request on `mapping` of its Registration with `context`, the Tools of `run_kind`, and the
/// purpose `purpose`.
fn request(
    mapping: &str,
    purpose: CreatePurpose,
    run_kind: AgentRunKind,
    context: Vec<storyos_application::RequestContextItem>,
) -> CreateRequest {
    let registration = match mapping {
        V8 => storyos_core::AGENT_PLAN_CONTINUITY_REGISTRATION.revision,
        _ => storyos_core::AGENT_PLAN_REGISTRATION.revision,
    };
    CreateRequest {
        purpose,
        context,
        tool_exposure: ToolExposure::compute(&ExposureFacts {
            bound: &[],
            model_registration_revision: registration,
            skill_catalog: false,
            memory_use: true,
            hybrid_search: false,
            run_kind,
        }),
        tool_choice: ToolChoice::Required,
        ..first(mapping, registration)
    }
}

/// The body of `make(mapping)` on the mappings `v8` and `v9`.
async fn bodies(make: impl Fn(&str) -> CreateRequest) -> (serde_json::Value, serde_json::Value) {
    (body(make(V8)).await, body(make(V9)).await)
}

/// `v9` with its instructions in place of the instructions of `v8` is the body of `v8`, and its
/// instructions are those of `v8` with `added` after `anchor`.
fn assert_adds(v8: &serde_json::Value, v9: &serde_json::Value, anchor: &str, added: &str) {
    let old = v8["instructions"].as_str().unwrap();
    let mut same = v9.clone();
    same["instructions"] = v8["instructions"].clone();
    assert_eq!(
        (&same, v9["instructions"].as_str().unwrap()),
        (
            v8,
            old.replacen(anchor, &format!("{anchor}{added}"), 1)
                .as_str()
        )
    );
}

/// ADR 0094 section 5: on the mapping `v9`, the paragraph of `write_memory` says that an entry
/// holds the words of the author. Each other part of the request is the request of `v8`.
#[tokio::test]
async fn the_write_memory_paragraph_of_v9_keeps_an_entry_to_the_words_of_the_author() {
    let (v8, v9) = bodies(|mapping| {
        request(
            mapping,
            CreatePurpose::Decision,
            AgentRunKind::Author,
            vec![item(
                ContextSourceClass::MemoryIndex,
                "{\"type\":\"other\"}",
            )],
        )
    })
    .await;

    assert_adds(
        &v8,
        &v9,
        "When the Host refuses a write, tell the author.",
        " An entry holds only what the author said in this Conversation. Do not add a quotation, \
         a clue, a name, a relation, or another fact that you read in the manuscript or in Memory \
         and that the author did not say. If such a fact can help the author, say it in your \
         answer and do not put it in the entry. When the author asks you to remember what a \
         Chapter says, the author said so, and the entry can hold it. A decision that the author \
         states with no request to remember it is not a request: do not call write_memory for \
         it, because the Memory work of the Host records it later.",
    );
}

/// ADR 0094 section 5: on the mapping `v9`, the Host instructions of a capture say what a capture
/// of a Project Conversation records. The sentences about a Chapter do not change.
#[tokio::test]
async fn the_capture_instructions_of_v9_keep_a_conversation_capture_to_the_messages() {
    let (v8, v9) = bodies(|mapping| {
        request(
            mapping,
            CreatePurpose::MemoryCapture(CaptureInputKind::Conversation),
            AgentRunKind::MemoryCapture,
            vec![item(ContextSourceClass::MemoryCaptureInput, "继续写。")],
        )
    })
    .await;

    assert_adds(
        &v8,
        &v9,
        "it is not a decision of the author.",
        " In a capture of a Project Conversation, an observation is a decision, a preference, or \
         an intent that the messages have. You can read the manuscript to understand a message, \
         and you do not record a fact that only the manuscript has.",
    );
}

/// ADR 0094 section 6: on the mapping `v9`, the Host instructions of a pass say what the key
/// `refused` of an observation means.
#[tokio::test]
async fn the_pass_instructions_of_v9_say_what_the_key_refused_means() {
    let (v8, v9) = bodies(|mapping| {
        let line = json!({ "observation": 1, "refused": "memory_entry_invalid" }).to_string();
        request(
            mapping,
            CreatePurpose::MemoryConsolidation,
            AgentRunKind::MemoryConsolidation,
            vec![item(ContextSourceClass::MemoryObservations, &line)],
        )
    })
    .await;

    assert_adds(
        &v8,
        &v9,
        "the numbers of the observations that it comes from.",
        " An observation with the key refused was in an operation that the Host refused for that \
         reason. Give a correct operation for it, or no operation.",
    );
}
