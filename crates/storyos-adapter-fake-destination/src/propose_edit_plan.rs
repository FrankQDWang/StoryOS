//! The `propose_edit` script of the tool loop profile (ADR 0091). For an author message that
//! starts with `Revise these passages`, the fake proposes the changes of the reply form fake for
//! the declared targets with a `finish` call beside it. The end of the message scripts the other
//! cases, by the count of the `propose_edit` results of the request:
//!
//! - `SCRIPT:propose_refused_once`: the first call names a block that is not a target, alone,
//!   and the next call is correct.
//! - `SCRIPT:propose_always_refused`: five calls that name a block that is not a target, then
//!   only `finish`.
//! - `SCRIPT:propose_twice`: two correct calls in one response.
//! - `SCRIPT:propose_again`: a correct call alone, then a second correct call alone.
//! - `SCRIPT:propose_finish_first`: a `finish` call before a correct call in one response.
//! - `SCRIPT:propose_mixed`: two calls that name a block that is not a target in one response,
//!   then one such call, then one call with no arguments.

use storyos_application::CreateRequest;
use storyos_core::{DeclaredLocation, NativeStreamItem, ToolOperation};

use crate::prose_changes::produce_fake_prose_changes;

/// The calls of one request, and whether a `finish` call follows them. `None` when the script
/// does not apply.
pub(crate) fn calls(
    request: &CreateRequest,
    call: &dyn Fn(&str, String, String) -> NativeStreamItem,
) -> Option<(Vec<NativeStreamItem>, bool)> {
    let name = ToolOperation::ProposeEdit.function_name();
    let message = request.author_message.as_str();
    if !message.starts_with("Revise these passages")
        || !request
            .tool_exposure
            .tools
            .iter()
            .any(|tool| tool.name == name)
    {
        return None;
    }
    let results = request
        .tool_outputs
        .iter()
        .filter(|output| output.name == name)
        .count();
    let targets: Vec<DeclaredLocation> = request
        .declared_targets
        .iter()
        .map(storyos_application::DeclaredTarget::location)
        .collect();
    let proposal = |index: usize, script: &str| {
        let changes: Vec<_> = produce_fake_prose_changes(&targets, script)
            .into_iter()
            .map(|change| {
                serde_json::json!({
                    "block_id": change.manuscript_block_id,
                    "candidate_text": change.candidate_text,
                    "explanation": change.explanation,
                })
            })
            .collect();
        let arguments =
            serde_json::json!({"summary": "Kept the narrator voice.", "changes": changes});
        // The arguments text has line breaks, so a refused call shows that it keeps its text.
        call(
            name,
            format!("_{results}_{index}"),
            format!("{arguments:#}"),
        )
    };
    let correct = |index| proposal(index, message);
    let undeclared = |index| proposal(index, "SCRIPT:undeclared_location");
    let script = message.rsplit_once("SCRIPT:").map(|(_, script)| script);
    Some(match (script, results) {
        (Some("propose_refused_once"), 0) => (vec![undeclared(0)], false),
        (Some("propose_refused_once"), 1) => (vec![correct(0)], true),
        (Some("propose_always_refused"), 0..5) => (vec![undeclared(0)], false),
        (Some("propose_twice"), 0) => (vec![correct(0), correct(1)], true),
        (Some("propose_again"), 0 | 1) => (vec![correct(0)], false),
        (Some("propose_finish_first"), 0) => {
            let finish = call("finish", format!("_{results}_9"), "{}".into());
            (vec![finish, correct(0)], false)
        }
        (Some("propose_mixed"), 0) => (vec![undeclared(0), undeclared(1)], false),
        (Some("propose_mixed"), 2) => (vec![undeclared(0)], false),
        (Some("propose_mixed"), 3) => (
            vec![call(name, format!("_{results}_0"), "{}".into())],
            false,
        ),
        (
            Some(
                "propose_refused_once"
                | "propose_always_refused"
                | "propose_twice"
                | "propose_again"
                | "propose_mixed"
                | "propose_finish_first",
            ),
            _,
        ) => (Vec::new(), true),
        (_, 0) => (vec![correct(0)], true),
        (_, _) => (Vec::new(), true),
    })
}
