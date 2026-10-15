//! The exact request body of one Agent Plan Responses Create request.

use storyos_application::{CreateRequest, RequestBounds};
use storyos_core::{ContextSourceClass, DeclaredTargetKind, SelectionReference};

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
target blocks.

A target block that is marked as a pending suggestion holds the text of a suggestion that the \
author has not accepted, and its change replaces that suggestion text. Change only Chapter target \
blocks, or exactly one pending suggestion block.

Author references show the exact text that the author selected for this request. Use the author \
request to decide if you change the referenced text or only use it. A change replaces all of a \
block: keep the text outside the references unchanged unless the author request says otherwise.";

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
    let references = request
        .selection_references
        .iter()
        .filter_map(|admitted| Some((admitted, admitted.selected_text.as_ref()?)))
        .enumerate()
        .map(|(index, (admitted, text))| {
            let position = index + 1;
            let range = match &admitted.reference {
                SelectionReference::ChapterText {
                    chapter_id,
                    start,
                    end,
                    ..
                } => format!(
                    "Chapter text of chapter_id {chapter_id}, from block_id {} character {} to \
                     block_id {} character {}",
                    start.manuscript_block_id, start.offset, end.manuscript_block_id, end.offset
                ),
                SelectionReference::ProposalCandidateText {
                    start_offset,
                    end_offset,
                    ..
                } => format!(
                    "Pending suggestion text of block_id {}, from character {start_offset} to \
                     character {end_offset}",
                    admitted.manuscript_block_ids.join(", ")
                ),
            };
            format!("[reference {position}] {range} (UTF-16 code units):\n{text}")
        })
        .collect::<Vec<_>>();
    if !references.is_empty() {
        sections.push(format!("Author references:\n{}", references.join("\n\n")));
    }
    if !request.declared_targets.is_empty() {
        let targets = request
            .declared_targets
            .iter()
            .map(|target| match target.kind {
                DeclaredTargetKind::ChapterBlock => {
                    format!("[block_id: {}]\n{}", target.block_id, target.block_text)
                }
                DeclaredTargetKind::PendingCandidate => format!(
                    "[block_id: {}] (pending suggestion)\n{}",
                    target.block_id, target.block_text
                ),
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        sections.push(format!("Target blocks:\n{targets}"));
    }
    sections.join("\n\n")
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
