//! The load script of the fake model (ADR 0089 section 3). An author message
//! `SCRIPT:load_skill <name>[,<name>...]` gets, in its first request, one `load_skill` call for
//! each name, in order, with the call identifiers `call_load_skill_0`, `call_load_skill_1`, and
//! so on. Each later request gets the usual script of the fake model, so the next request calls
//! `finish`. The script can follow the compaction prefix of the Host fake (ADR 0093).

use storyos_application::CreateRequest;
use storyos_core::{ToolChoice, ToolOperation};

use crate::create_plan::FakeCreate;
use crate::research_plan::function_call;

/// The `reason` of each scripted load.
pub const LOAD_REASON: &str = "The author request fits the description of this Skill.";

/// The scripted response of `request`, or `None` when the usual script applies.
pub(crate) fn plan(request: &CreateRequest) -> Option<FakeCreate> {
    let names = crate::script_message(&request.author_message)
        .lines()
        .next()?
        .strip_prefix("SCRIPT:load_skill ")?;
    if request.tool_choice == ToolChoice::None || !request.tool_outputs.is_empty() {
        return None;
    }
    let items = names
        .split(',')
        .enumerate()
        .map(|(index, name)| {
            let arguments = serde_json::json!({"skill": name.trim(), "reason": LOAD_REASON});
            function_call(
                ToolOperation::LoadSkill,
                &format!("_{index}"),
                &arguments.to_string(),
            )
        })
        .collect();
    Some(FakeCreate { items })
}
