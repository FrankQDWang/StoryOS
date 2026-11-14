//! The `propose_edit` script of the fake model (ADR 0091). For each request with a prose change
//! intent, the fake proposes changes for the declared targets with a `finish` call beside it: the
//! revision of a referenced candidate with the growth scripts, or else a change of each target.
//! For a prose change intent with no scripted changes, it proposes the summary text for the first
//! target, with the text as the summary.
//! With no script, the fake proposes when the request has no `propose_edit` result after its last
//! counted `finish` output. The end of the message scripts the other cases, by the count of the
//! `propose_edit` results of the request:
//!
//! - `SCRIPT:propose_two_candidates`: one call that revises each referenced candidate.
//! - `SCRIPT:propose_refused_once`: the first call names a block that is not a target, alone,
//!   and the next call is correct.
//! - `SCRIPT:propose_always_refused`: five calls that name a block that is not a target, then
//!   only `finish`.
//! - `SCRIPT:propose_twice`: two correct calls in one response.
//! - `SCRIPT:propose_again`: a correct call alone, then a second correct call alone.
//! - `SCRIPT:propose_finish_first`: a `finish` call before a correct call in one response.
//! - `SCRIPT:propose_research_finish_first`: as `propose_finish_first`, and a `submit_research`
//!   call that cites no captured source after the correct call.
//! - `SCRIPT:propose_mixed`: two calls that name a block that is not a target in one response,
//!   then one such call, then one call with no arguments.
//! - `SCRIPT:incomplete`: one correct call with the incomplete stream state.
//!
//! `SCRIPT:propose_block <block identity>` at the end of a message of each intent gets one call
//! that changes the named Block with a `finish` call beside it, in the first request only.

use storyos_application::CreateRequest;
use storyos_core::{
    DeclaredTargetKind, NativeStreamItem, PROSE_CHANGE_TEXT, ProseChangeCandidate, ToolOperation,
};

use crate::create_plan::Intent;

use crate::prose_changes::{produce_fake_candidate_revision, produce_fake_changes};

/// The calls of one request, and whether a `finish` call follows them. `None` when the script
/// does not apply.
pub(crate) fn calls(
    request: &CreateRequest,
    call: &dyn Fn(&str, String, String) -> NativeStreamItem,
) -> Option<(Vec<NativeStreamItem>, bool)> {
    let name = ToolOperation::ProposeEdit.function_name();
    let message = request.author_message.as_str();
    let exposed = request
        .tool_exposure
        .tools
        .iter()
        .any(|tool| tool.name == name);
    let results = request
        .tool_outputs
        .iter()
        .filter(|output| output.name == name)
        .count();
    if let Some((_, block_id)) = message.rsplit_once("SCRIPT:propose_block ") {
        let change = ProseChangeCandidate {
            chapter_id: request.chapter_id.clone(),
            manuscript_block_id: block_id.trim().to_owned(),
            base_authoritative_revision_id: String::new(),
            candidate_text: PROSE_CHANGE_TEXT.to_owned(),
            explanation: "Preserve the narrator voice in the named Block.".to_owned(),
        };
        let summary = "Kept the narrator voice.";
        let proposed = call(name, "_0_0".to_owned(), arguments(&[change], summary));
        return exposed.then(|| {
            (
                (results == 0).then_some(proposed).into_iter().collect(),
                true,
            )
        });
    }
    let (intent, produce) = crate::create_plan::intent(request);
    // A prose change text with no changes opens the Proposal of the first target (Stage 1).
    let single = match intent {
        Intent::ProseChange(text) if !produce => {
            Some(request.declared_targets.first()?.location()).map(|target| (target, text))
        }
        _ => None,
    };
    if !(produce || single.is_some()) || !exposed {
        return None;
    }
    // A steering input continues a Run after a counted `finish` call. With no script, the fake
    // proposes again for it: only the results after the last counted `finish` output count.
    let counted_finish = |output: &&storyos_application::ToolOutput| {
        output.name == ToolOperation::Finish.function_name()
            && !output
                .output
                .contains(storyos_core::FINISH_DOES_NOT_COUNT_RESULT)
    };
    let step_results = request
        .tool_outputs
        .iter()
        .rev()
        .take_while(|output| !counted_finish(output))
        .filter(|output| output.name == name)
        .count();
    let proposal = |index: usize, script: &str| {
        let changes = if script.ends_with("SCRIPT:propose_two_candidates") {
            request
                .declared_targets
                .iter()
                .filter(|target| target.kind == DeclaredTargetKind::PendingCandidate)
                .flat_map(|target| {
                    produce_fake_candidate_revision(
                        &[target.location()],
                        script,
                        &target.block_text,
                    )
                })
                .collect()
        } else if let Some((target, text)) = &single {
            vec![ProseChangeCandidate {
                chapter_id: target.chapter_id.clone(),
                manuscript_block_id: target.block_id.clone(),
                base_authoritative_revision_id: target.base_revision_id.clone(),
                candidate_text: text.clone(),
                explanation: "Preserve the narrator voice in the first passage.".to_owned(),
            }]
        } else {
            produce_fake_changes(request, script)
        };
        let summary = single
            .as_ref()
            .map_or("Kept the narrator voice.", |(_, text)| text.as_str());
        call(
            name,
            format!("_{results}_{index}"),
            arguments(&changes, summary),
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
        (Some("propose_research_finish_first"), 0) => {
            let finish = call("finish", format!("_{results}_9"), "{}".into());
            let research = serde_json::json!({
                "summary": "The tides.",
                "claims": [{"text": "Two high waters.", "citations": ["uncaptured"]}],
            });
            let research = call(
                ToolOperation::SubmitResearch.function_name(),
                format!("_{results}_8"),
                research.to_string(),
            );
            (vec![finish, correct(0), research], false)
        }
        (Some("incomplete"), 0) => {
            let mut incomplete = correct(0);
            incomplete.state = storyos_core::StreamItemState::Incomplete;
            (vec![incomplete], false)
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
                | "propose_finish_first"
                | "propose_research_finish_first",
            ),
            _,
        ) => (Vec::new(), true),
        _ if step_results == 0 => (vec![correct(0)], true),
        _ => (Vec::new(), true),
    })
}

/// The `propose_edit` arguments of `changes` with `summary`. The text has line breaks, so a
/// refused call shows that it keeps its text.
pub(crate) fn arguments(changes: &[ProseChangeCandidate], summary: &str) -> String {
    let changes: Vec<_> = changes
        .iter()
        .map(|change| {
            serde_json::json!({
                "block_id": change.manuscript_block_id,
                "candidate_text": change.candidate_text,
                "explanation": change.explanation,
            })
        })
        .collect();
    format!(
        "{:#}",
        serde_json::json!({"summary": summary, "changes": changes})
    )
}
