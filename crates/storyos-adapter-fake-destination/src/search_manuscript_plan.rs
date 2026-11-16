//! The manuscript search script of the fake model (ADR 0095 section 1). An author message whose
//! first line is `SCRIPT:search_manuscript <query>` gets one `search_manuscript` call with the
//! rest of the line as its query in the first request. Each later line
//! `SCRIPT:search_manuscript <query>` or `SCRIPT:read_chapter <number>` of the message adds one
//! more call to the same response, in line order. The call identifiers are
//! `call_search_manuscript_<n>` and `call_read_chapter_<n>`, where `<n>` is the position of the
//! call in the response. Each other request gets the usual script of the fake model, so the next
//! request calls `finish`. The script can follow the compaction prefix of the Host fake (ADR
//! 0093).

use storyos_application::CreateRequest;
use storyos_core::{ToolChoice, ToolOperation};

use crate::create_plan::FakeCreate;
use crate::research_plan::function_call;

const SEARCH: &str = "SCRIPT:search_manuscript ";
const READ: &str = "SCRIPT:read_chapter ";

/// The scripted response of `request`, or `None` when the usual script applies.
pub(crate) fn plan(request: &CreateRequest) -> Option<FakeCreate> {
    let message = crate::script_message(&request.author_message);
    if !message.starts_with(SEARCH)
        || !request.tool_outputs.is_empty()
        || request.tool_choice == ToolChoice::None
    {
        return None;
    }
    let calls = message.lines().filter_map(|line| {
        match (line.strip_prefix(SEARCH), line.strip_prefix(READ)) {
            (Some(query), _) => Some((
                ToolOperation::SearchManuscript,
                serde_json::json!({ "query": query }).to_string(),
            )),
            (_, Some(number)) => Some((
                ToolOperation::ReadChapter,
                format!("{{\"chapter\":{number}}}"),
            )),
            (None, None) => None,
        }
    });
    let items = calls
        .enumerate()
        .map(|(index, (operation, arguments))| {
            function_call(operation, &format!("_{index}"), &arguments)
        })
        .collect();
    Some(FakeCreate { items })
}
