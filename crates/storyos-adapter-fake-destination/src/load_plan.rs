//! The load script of the fake model (ADR 0089 section 3). An author message
//! `SCRIPT:load_skill <name>[,<name>...][;<name>[,<name>...]...]` gets one `load_skill` call for
//! each name of a group, in order, with the call identifiers `call_load_skill_0`,
//! `call_load_skill_1`, and so on across the groups. The first request gets the first group, and
//! the request that sends the outputs of the loads of the groups before a group gets that group.
//! Each other request gets the usual script of the fake model, so the next request calls
//! `finish`. The script can follow the compaction prefix of the Host fake (ADR 0093).

use storyos_application::CreateRequest;
use storyos_core::{ToolChoice, ToolOperation};

use crate::create_plan::FakeCreate;
use crate::research_plan::function_call;

/// The `reason` of each scripted load.
pub const LOAD_REASON: &str = "The author request fits the description of this Skill.";

/// The scripted response of `request`, or `None` when the usual script applies.
pub(crate) fn plan(request: &CreateRequest) -> Option<FakeCreate> {
    let groups = crate::script_message(&request.author_message)
        .lines()
        .next()?
        .strip_prefix("SCRIPT:load_skill ")?;
    let loads = ToolOperation::LoadSkill.function_name();
    let sent = request.tool_outputs.len();
    if request.tool_choice == ToolChoice::None
        || request
            .tool_outputs
            .iter()
            .any(|output| output.name != loads)
    {
        return None;
    }
    // The index of the first call of each group is the count of the calls before it.
    let mut first = 0;
    for group in groups.split(';') {
        let names: Vec<&str> = group.split(',').map(str::trim).collect();
        if first == sent {
            let items = names
                .iter()
                .enumerate()
                .map(|(index, name)| {
                    let arguments = serde_json::json!({"skill": name, "reason": LOAD_REASON});
                    function_call(
                        ToolOperation::LoadSkill,
                        &format!("_{}", first + index),
                        &arguments.to_string(),
                    )
                })
                .collect();
            return Some(FakeCreate { items });
        }
        first += names.len();
    }
    None
}
