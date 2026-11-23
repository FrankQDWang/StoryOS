//! The plan script of the fake model (ADR 0103 section 2). An author message `SCRIPT:update_plan`
//! gets one `update_plan` call with the same three step texts in each of its first three
//! requests: in the first request the first step is `in_progress`, in the second request the
//! first step is `completed` and the second step is `in_progress`, and in the third request each
//! step is `completed` and a `finish` call follows. The call identifiers are
//! `call_update_plan_<n>`, where `<n>` is the RunStep from 1. Each later request gets the usual
//! script of the fake model, so the closing request writes the final answer. The script can
//! follow the compaction prefix of the Host fake (ADR 0093).

use storyos_application::CreateRequest;
use storyos_core::{ToolChoice, ToolOperation};

use crate::create_plan::FakeCreate;
use crate::research_plan::function_call;

/// The author message of the plan script.
pub const UPDATE_PLAN_SCRIPT: &str = "SCRIPT:update_plan";

/// The three step texts of the plan script.
pub const UPDATE_PLAN_STEPS: [&str; 3] = ["读第一章到第五章", "列出不一致之处", "改写第三章第一段"];

/// The scripted response of `request`, or `None` when the usual script applies.
pub(crate) fn plan(request: &CreateRequest) -> Option<FakeCreate> {
    if crate::script_message(&request.author_message) != UPDATE_PLAN_SCRIPT
        || request.tool_choice == ToolChoice::None
    {
        return None;
    }
    let recorded = (request.tool_outputs.iter())
        .filter(|output| output.name == ToolOperation::UpdatePlan.function_name())
        .count();
    let statuses = match recorded {
        0 => ["in_progress", "pending", "pending"],
        1 => ["completed", "in_progress", "pending"],
        2 => ["completed", "completed", "completed"],
        _ => return None,
    };
    let steps: Vec<serde_json::Value> = (UPDATE_PLAN_STEPS.iter().zip(statuses))
        .map(|(text, status)| serde_json::json!({ "text": text, "status": status }))
        .collect();
    let arguments = serde_json::json!({ "steps": steps }).to_string();
    let call = function_call(
        ToolOperation::UpdatePlan,
        &format!("_{}", recorded + 1),
        &arguments,
    );
    let finish = (recorded == 2).then(|| function_call(ToolOperation::Finish, "", "{}"));
    Some(FakeCreate {
        items: std::iter::once(call).chain(finish).collect(),
    })
}
