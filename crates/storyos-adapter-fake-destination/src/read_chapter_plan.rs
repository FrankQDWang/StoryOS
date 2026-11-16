//! The Chapter read script of the fake model (ADR 0095 section 1). An author message
//! `SCRIPT:read_chapter <number>[,<number>...][;<number>[,<number>...]...]` gets one
//! `read_chapter` call for each number of a group, in order, in one response, with the call
//! identifiers `call_read_chapter_0`, `call_read_chapter_1`, and so on across the groups. The
//! first request gets the first group, and the request that sends the outputs of the reads of the
//! groups before a group gets that group. Each number is the JSON text of the argument `chapter`,
//! so a script can also give a value that is not an integer. Each other request gets the usual
//! script of the fake model, so the next request calls `finish`. The script can follow the
//! compaction prefix of the Host fake (ADR 0093).

use storyos_application::CreateRequest;
use storyos_core::{ToolChoice, ToolOperation};

use crate::create_plan::FakeCreate;
use crate::research_plan::function_call;

/// The scripted response of `request`, or `None` when the usual script applies.
pub(crate) fn plan(request: &CreateRequest) -> Option<FakeCreate> {
    let groups = crate::script_message(&request.author_message)
        .lines()
        .next()?
        .strip_prefix("SCRIPT:read_chapter ")?;
    let reads = ToolOperation::ReadChapter.function_name();
    let sent = request.tool_outputs.len();
    if request.tool_choice == ToolChoice::None
        || request
            .tool_outputs
            .iter()
            .any(|output| output.name != reads)
    {
        return None;
    }
    // The index of the first call of each group is the count of the calls before it.
    let mut first = 0;
    for group in groups.split(';') {
        let numbers: Vec<&str> = group.split(',').map(str::trim).collect();
        if first == sent {
            let items = numbers
                .iter()
                .enumerate()
                .map(|(index, number)| {
                    function_call(
                        ToolOperation::ReadChapter,
                        &format!("_{}", first + index),
                        &format!("{{\"chapter\":{number}}}"),
                    )
                })
                .collect();
            return Some(FakeCreate { items });
        }
        first += numbers.len();
    }
    None
}
