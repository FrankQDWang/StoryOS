//! The request body of one RunStep of a Subrun on the current mapping (ADR 0103 section 3A). The
//! request has no author message: its user message has the Project instruction, the task, the
//! Chapter list, and the Memory index as sections. Each RunStep sends the full input with the
//! calls and the Tool outputs of the Subrun (ADR 0085), and the finish rule of a Run of the
//! author decides its `tool_choice` (ADR 0091 section 4).

use storyos_application::{CreateRequest, RequestBounds};
use storyos_core::{ContextSourceClass, ToolOperation};

use crate::wire::{
    CHAPTER_LIST_INSTRUCTION, MEMORY_INDEX_INSTRUCTION, READ_CHAPTER_INSTRUCTION,
    READ_MEMORY_ENTRY_INSTRUCTION, SEARCH_MANUSCRIPT_INSTRUCTION, SEARCH_MEMORY_INSTRUCTION,
};

/// The Host instructions of a Subrun. The paragraph of its limit of RunSteps follows them.
pub(crate) const SUBRUN_INSTRUCTIONS: &str = "\
You are a reading helper of the writing assistant of one novel author. The assistant gave you one \
task. Do only this task with the supplied functions. You can only read: you can change nothing.

You work in steps. In each step, call one or more of the supplied functions. StoryOS executes \
each call and gives you its result in the next step. When StoryOS refuses a call, the result \
gives the reason, and you can correct the call.

When the task is complete, call finish. After the result of finish, write your final answer as \
text, and call no function. The assistant gets only this final answer, and it does not see your \
steps. Thus the answer must be complete by itself. Give each fact with the Chapter that it comes \
from. When you did not find something, say so. Write in the language of the manuscript.";

/// The Host instructions of one RunStep of a Subrun: the instructions of a Subrun, its limit,
/// the paragraphs of the Chapter list and of the Memory index with the sentences of the declared
/// read and search Tools, as a request of a Run of the author has them, the sentence of the source
/// tag when the request sends a Tool result, and the sentence near the limit.
pub(crate) fn subrun_instructions(request: &CreateRequest) -> String {
    let declares = |operation| {
        (request.tool_exposure.tools.iter()).any(|tool| tool.spec.operation == operation)
    };
    let has = |class| (request.context.iter()).any(|item| item.source_class == class);
    let limit = crate::step_limit_wire::subrun_limit_paragraph(request.run_step.max_run_steps);
    let mut paragraphs = vec![SUBRUN_INSTRUCTIONS.to_owned(), limit];
    if has(ContextSourceClass::ChapterList) {
        paragraphs.push(CHAPTER_LIST_INSTRUCTION.to_owned());
        if declares(ToolOperation::ReadChapter) {
            paragraphs.push(READ_CHAPTER_INSTRUCTION.to_owned());
            if declares(ToolOperation::SearchManuscript) {
                paragraphs.push(SEARCH_MANUSCRIPT_INSTRUCTION.to_owned());
            }
        }
    }
    if has(ContextSourceClass::MemoryIndex) {
        let mut paragraph = MEMORY_INDEX_INSTRUCTION.to_owned();
        for (operation, sentences) in [
            (
                ToolOperation::ReadMemoryEntry,
                READ_MEMORY_ENTRY_INSTRUCTION,
            ),
            (ToolOperation::SearchMemory, SEARCH_MEMORY_INSTRUCTION),
        ] {
            if declares(operation) {
                paragraph = format!("{paragraph} {sentences}");
            }
        }
        paragraphs.push(paragraph);
    }
    // A later request wraps each result in a source tag, and the sentence says that its content
    // is data (ADR 0085 section 3).
    if !request.tool_outputs.is_empty() {
        paragraphs.push(storyos_core::TOOL_RESULT_SOURCE_TAG_INSTRUCTION.to_owned());
    }
    crate::step_limit_wire::with_notice(
        paragraphs.join("\n\n"),
        request.run_step,
        request.tool_choice,
    )
}

/// The canonical JSON body of one RunStep of a Subrun.
pub(crate) fn subrun_body(
    request: &CreateRequest,
    provider_model_id: &str,
    bounds: RequestBounds,
) -> String {
    let body = serde_json::json!({
        "model": provider_model_id,
        "instructions": subrun_instructions(request),
        "input": crate::wire::input(request, &subrun_text(request)),
        "max_output_tokens": bounds.max_output_tokens,
        "stream": true,
        "store": true,
        "tools": request
            .tool_exposure
            .declarations()
            .into_iter()
            .map(|declaration| {
                serde_json::json!({
                    "type": "function",
                    "name": declaration.name,
                    "description": declaration.description,
                    "parameters": declaration.parameters,
                    "strict": true,
                })
            })
            .collect::<Vec<_>>(),
        "tool_choice": request.tool_choice.as_str(),
    });
    storyos_core::canonical_json(&body)
}

/// The user message of a Subrun: each item of the Subrun as one section with its exact text, in
/// item order.
fn subrun_text(request: &CreateRequest) -> String {
    (request.context.iter())
        .filter_map(|item| {
            let heading = match item.source_class {
                ContextSourceClass::InstructionBinding => "Project instruction",
                ContextSourceClass::SubrunBrief => "Task",
                ContextSourceClass::ChapterList => "Chapter list",
                ContextSourceClass::MemoryIndex => "Memory index",
                // A Subrun has no other item, and a Tool result goes as a function output.
                ContextSourceClass::HostControl
                | ContextSourceClass::AuthorInstruction
                | ContextSourceClass::WorkingTarget
                | ContextSourceClass::ConversationSummary
                | ContextSourceClass::ToolResult
                | ContextSourceClass::SkillInstruction
                | ContextSourceClass::SkillCatalog
                | ContextSourceClass::SkillResource
                | ContextSourceClass::ManuscriptReadResult
                | ContextSourceClass::ManuscriptSearchResult
                | ContextSourceClass::MemoryReadResult
                | ContextSourceClass::MemorySearchResult
                | ContextSourceClass::MemoryCaptureInput
                | ContextSourceClass::MemoryDeleteMarks
                | ContextSourceClass::MemoryEntries
                | ContextSourceClass::MemoryObservations
                | ContextSourceClass::CharacterFile
                | ContextSourceClass::EarlierAnswer => return None,
            };
            Some(format!("{heading}:\n{}", item.content))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
#[path = "subrun_wire_tests.rs"]
pub(crate) mod tests;
