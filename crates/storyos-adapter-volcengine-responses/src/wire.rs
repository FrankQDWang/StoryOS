//! The exact request body of one Agent Plan Responses Create request.

use storyos_application::{CreateRequest, RequestBounds};
use storyos_core::ContextSourceClass;

/// The Host instructions and the reply contract that `decision::map_output` reads.
const HOST_INSTRUCTIONS: &str = "\
You are the writing assistant of one novel author. The author keeps authority over the novel: \
you only suggest, and the author decides. Answer the author request with the supplied text. \
Write in the language of the manuscript.

Reply with exactly one JSON object and no other text. Use one of these forms:
{\"kind\":\"advisory\",\"text\":\"<your answer to the author>\"}
{\"kind\":\"clarification\",\"question\":\"<the one question that you must ask before you can help>\"}
{\"kind\":\"prose_change\",\"summary\":\"<one sentence that tells what you changed>\",\
\"changes\":[{\"block_id\":\"<the id of one target block>\",\"candidate_text\":\"<the complete \
new text of that block>\",\"explanation\":\"<one sentence that tells why>\"}]}

Use \"prose_change\" only when the request has target blocks. Then give exactly one change for \
each target block, with its exact block_id. Do not change text outside the target blocks.";

/// The canonical JSON body, which is also the committed Wire Payload Projection.
pub(crate) fn create_body(
    request: &CreateRequest,
    provider_model_id: &str,
    bounds: RequestBounds,
) -> String {
    storyos_core::canonical_json(&serde_json::json!({
        "model": provider_model_id,
        "instructions": HOST_INSTRUCTIONS,
        "input": [{
            "role": "user",
            "content": [{"type": "input_text", "text": request_text(request)}],
        }],
        "max_output_tokens": bounds.max_output_tokens,
        "stream": false,
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
