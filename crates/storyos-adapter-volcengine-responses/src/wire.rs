//! The exact request body of one Agent Plan Responses Create request.

use storyos_application::{CreateRequest, RequestBounds};
use storyos_core::ContextSourceClass;

/// The Host instructions and the reply contract that `decision::map_output` reads.
pub(crate) const HOST_INSTRUCTIONS: &str = "\
You are the writing assistant of one novel author. The author keeps authority over the novel: \
you only suggest, and the author decides. Answer the author request with the supplied text. \
Write in the language of the manuscript.

Reply with exactly one JSON object and no other text. Use one of these forms:
{\"kind\":\"advisory\",\"text\":\"<your answer to the author>\"}
{\"kind\":\"clarification\",\"question\":\"<the one question that you must ask before you can help>\"}
{\"kind\":\"prose_change\",\"summary\":\"<one sentence that tells what you changed>\",\
\"changes\":[{\"block_id\":\"<the id of one target block>\",\"candidate_text\":\"<the complete \
new text of that block>\",\"explanation\":\"<one sentence that tells why>\"}]}

Use \"prose_change\" only when the request has target blocks. Then give one change for each \
target block that you change, with its exact block_id. Give at least one change. Do not include a \
target block that you do not change, because it keeps its text. Do not change text outside the \
target blocks.";

/// The canonical JSON body, which is also the committed Wire Payload Projection. Instructions do
/// not carry forward with a prior response reference, so each request sends them.
pub(crate) fn create_body(
    request: &CreateRequest,
    provider_model_id: &str,
    bounds: RequestBounds,
    stream: bool,
) -> String {
    let mut body = serde_json::json!({
        "model": provider_model_id,
        "instructions": HOST_INSTRUCTIONS,
        "input": [{
            "role": "user",
            "content": [{"type": "input_text", "text": request_text(request)}],
        }],
        "max_output_tokens": bounds.max_output_tokens,
        "stream": stream,
        "store": true,
    });
    if let Some(reference) = &request.previous_response_reference {
        body["previous_response_id"] = serde_json::json!(reference);
    }
    storyos_core::canonical_json(&body)
}

/// The Host instruction of a compaction summary request. The prior response reference supplies
/// the history, and the request sends no project content.
pub(crate) const SUMMARY_INSTRUCTIONS: &str = "\
You help one novel author. Summarize the conversation so far for a later request that will not \
see it. Keep the decisions of the author, the constraints, the open questions, the pending work, \
and the state of the manuscript passages that the conversation discussed. Write in the language \
of the manuscript. Reply with the summary text only, with no JSON and no other text.";

/// The body of a compaction summary request: the summary instruction and the prior reference.
pub(crate) fn summary_body(
    reference: &str,
    provider_model_id: &str,
    bounds: RequestBounds,
    stream: bool,
) -> String {
    storyos_core::canonical_json(&serde_json::json!({
        "model": provider_model_id,
        "instructions": SUMMARY_INSTRUCTIONS,
        "input": [{
            "role": "user",
            "content": [{"type": "input_text", "text": "Summarize the conversation so far."}],
        }],
        "previous_response_id": reference,
        "max_output_tokens": bounds.max_output_tokens,
        "stream": stream,
        "store": true,
    }))
}

/// The author request, the selected Context, and the declared targets as one user message.
fn request_text(request: &CreateRequest) -> String {
    let mut sections = Vec::new();
    for item in &request.context {
        let heading = match item.source_class {
            ContextSourceClass::HostControl => continue,
            ContextSourceClass::AuthorInstruction => "Author request",
            ContextSourceClass::WorkingTarget => "Working text",
            ContextSourceClass::InstructionBinding => "Project instruction",
            ContextSourceClass::ConversationSummary => "Summary of the earlier conversation",
        };
        sections.push(format!("{heading}:\n{}", item.content));
    }
    if let Some(candidate) = &request.candidate_revision {
        sections.push(format!("Current candidate text:\n{candidate}"));
    }
    if !request.declared_targets.is_empty() {
        let targets = request
            .declared_targets
            .iter()
            .map(|target| format!("[block_id: {}]\n{}", target.block_id, target.block_text))
            .collect::<Vec<_>>()
            .join("\n\n");
        sections.push(format!("Target blocks:\n{targets}"));
    }
    sections.join("\n\n")
}
