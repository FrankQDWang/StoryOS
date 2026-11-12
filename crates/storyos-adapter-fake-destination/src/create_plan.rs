//! Derive one scripted Create result from the exact request, with no process memory.

use storyos_application::{CreateRequest, DeclaredTarget};
use storyos_core::DeclaredTargetKind;
use storyos_core::{
    ADVISORY_TEXT, DecisionCandidate, ModelOutput, NativeStreamItem, OrdinaryPassageResolution,
    OutputPhase, PASSAGE_REFERENCE_QUESTION, PROSE_CHANGE_TEXT, STREAM_FIRST_TEXT,
    STREAM_SECOND_TEXT, StreamItemRole, StreamItemState, ToolOperation,
};

use crate::prose_changes::{
    is_fake_candidate_revision_request, produce_fake_candidate_revision, produce_fake_growth,
    produce_fake_prose_changes,
};

const CLARIFICATION_QUESTION: &str = "Which wording should stay in this sentence?";

/// The native items and structured output of one fake Create exchange.
pub(crate) struct FakeCreate {
    pub items: Vec<NativeStreamItem>,
    pub output: Option<ModelOutput>,
}

pub(crate) fn plan_create(request: &CreateRequest) -> FakeCreate {
    if request.tool_outputs.is_empty()
        && request
            .tool_exposure
            .tools
            .iter()
            .any(|tool| tool.spec.operation == ToolOperation::Finish)
    {
        return finish_call(request);
    }
    let mut planned = plan_message(&request.author_message, request.passage_resolution);
    apply_prose_changes(request, &mut planned);
    planned
}

fn plan_message(author_message: &str, resolution: Option<OrdinaryPassageResolution>) -> FakeCreate {
    if let Some(resolution) = resolution {
        return resolved(resolution);
    }
    if let Some(scripted) = scripted_plan(author_message) {
        return scripted;
    }
    if author_message.starts_with("Stream this passage:") {
        return FakeCreate {
            items: vec![
                assistant_item("1", StreamItemState::Provisional, STREAM_FIRST_TEXT),
                assistant_item("2", StreamItemState::Provisional, STREAM_SECOND_TEXT),
                assistant_item("3", StreamItemState::Complete, PROSE_CHANGE_TEXT),
            ],
            output: Some(ModelOutput {
                phase: OutputPhase::FinalAnswer,
                candidate: DecisionCandidate::ProseChange {
                    text: PROSE_CHANGE_TEXT.to_owned(),
                },
                prose_changes: None,
            }),
        };
    }
    if author_message.starts_with("Revise this passage:")
        || author_message.starts_with("Revise these passages")
        || author_message.starts_with("Tighten this paragraph")
    {
        return complete(
            DecisionCandidate::ProseChange {
                text: PROSE_CHANGE_TEXT.to_owned(),
            },
            OutputPhase::FinalAnswer,
        );
    }
    if author_message.starts_with("Which wording should I keep?") {
        return complete(
            DecisionCandidate::Clarification {
                question: CLARIFICATION_QUESTION.to_owned(),
            },
            OutputPhase::FinalAnswer,
        );
    }
    complete(
        DecisionCandidate::Advisory {
            text: ADVISORY_TEXT.to_owned(),
        },
        OutputPhase::FinalAnswer,
    )
}

/// The first RunStep of the tool loop profile: a `finish` call and no text. An author message
/// `Search the web: <query>` also asks for one `web_search` call first when the Run exposes it.
fn finish_call(request: &CreateRequest) -> FakeCreate {
    let call = |name: &str, arguments: String| NativeStreamItem {
        item_id: format!("fc_{name}"),
        call_id: Some(format!("call_{name}")),
        arguments: Some(arguments),
        native_type: Some(storyos_core::FUNCTION_CALL_NATIVE_TYPE.to_owned()),
        name: Some(name.to_owned()),
        ..evidence_item(
            StreamItemRole::Tool,
            StreamItemState::Complete,
            "host_fake_function_call",
        )
    };
    let search = request
        .author_message
        .strip_prefix("Search the web: ")
        .filter(|_| {
            request
                .tool_exposure
                .tools
                .iter()
                .any(|tool| tool.spec.operation == ToolOperation::WebSearch)
        })
        .map(|query| {
            let arguments = serde_json::json!({"query": query, "result_count": null});
            call("web_search", arguments.to_string())
        });
    FakeCreate {
        items: search
            .into_iter()
            .chain([call("finish", "{}".to_owned())])
            .collect(),
        output: None,
    }
}

fn resolved(resolution: OrdinaryPassageResolution) -> FakeCreate {
    match resolution {
        OrdinaryPassageResolution::Resolved => complete(
            DecisionCandidate::ProseChange {
                text: PROSE_CHANGE_TEXT.to_owned(),
            },
            OutputPhase::FinalAnswer,
        ),
        OrdinaryPassageResolution::Clarification => complete(
            DecisionCandidate::Clarification {
                question: PASSAGE_REFERENCE_QUESTION.to_owned(),
            },
            OutputPhase::FinalAnswer,
        ),
    }
}

fn apply_prose_changes(request: &CreateRequest, planned: &mut FakeCreate) {
    let author_message = request.author_message.as_str();
    let candidate = request
        .declared_targets
        .iter()
        // The last referenced candidate, so that a test can revise a later Operation.
        .rfind(|target| target.kind == DeclaredTargetKind::PendingCandidate);
    if candidate.is_some()
        && is_fake_candidate_revision_request(author_message)
        && let Some(output) = planned.output.as_mut()
        && let DecisionCandidate::Advisory { text } = &output.candidate
    {
        output.candidate = DecisionCandidate::ProseChange { text: text.clone() };
    }
    let prose_change = planned
        .output
        .as_ref()
        .is_some_and(|output| matches!(output.candidate, DecisionCandidate::ProseChange { .. }));
    let produce = match candidate {
        Some(_) => prose_change,
        None => {
            author_message.starts_with("Revise these passages")
                || (request
                    .declared_targets
                    .first()
                    .is_some_and(|target| target.collection)
                    && prose_change)
        }
    };
    if !produce {
        return;
    }
    let changes = match candidate {
        Some(candidate) => {
            let mut changes = produce_fake_candidate_revision(
                &[candidate.location()],
                author_message,
                &candidate.block_text,
            );
            changes.extend(produce_fake_growth(
                &request.declared_targets,
                author_message,
            ));
            changes
        }
        None => produce_fake_prose_changes(
            &request
                .declared_targets
                .iter()
                .map(DeclaredTarget::location)
                .collect::<Vec<_>>(),
            author_message,
        ),
    };
    if let Some(item) = planned.items.first_mut() {
        item.text = Some(storyos_core::canonical_json(&serde_json::json!(changes)));
        item.summary = Some("host_fake_native_prose_changes".to_owned());
    }
    if author_message.ends_with("SCRIPT:incomplete") {
        planned.output = None;
        if let Some(item) = planned.items.first_mut() {
            item.state = StreamItemState::Incomplete;
        }
        return;
    }
    if let Some(output) = planned.output.as_mut() {
        if author_message.ends_with("SCRIPT:unselected")
            && matches!(output.candidate, DecisionCandidate::ProseChange { .. })
        {
            output.phase = OutputPhase::Commentary;
        }
        output.prose_changes = Some(changes);
    }
}

fn complete(candidate: DecisionCandidate, phase: OutputPhase) -> FakeCreate {
    let text = match &candidate {
        DecisionCandidate::Advisory { text }
        | DecisionCandidate::ProseChange { text }
        | DecisionCandidate::Clarification { question: text }
        | DecisionCandidate::ResearchSynthesis { summary: text, .. } => text.clone(),
    };
    FakeCreate {
        items: vec![assistant_item("1", StreamItemState::Complete, &text)],
        output: Some(ModelOutput {
            phase,
            candidate,
            prose_changes: None,
        }),
    }
}

pub(crate) fn assistant_item(
    item_id: &str,
    state: StreamItemState,
    text: &str,
) -> NativeStreamItem {
    NativeStreamItem {
        item_id: item_id.to_owned(),
        role: StreamItemRole::Assistant,
        state,
        text: Some(text.to_owned()),
        summary: Some("host_fake_native_text".to_owned()),
        call_id: None,
        arguments: None,
        refusal: None,
        native_type: None,
        name: None,
    }
}

fn evidence_item(role: StreamItemRole, state: StreamItemState, summary: &str) -> NativeStreamItem {
    NativeStreamItem {
        item_id: "1".to_owned(),
        role,
        state,
        text: None,
        summary: Some(summary.to_owned()),
        call_id: None,
        arguments: None,
        refusal: None,
        native_type: None,
        name: None,
    }
}

fn scripted_plan(author_message: &str) -> Option<FakeCreate> {
    let state = match author_message {
        "SCRIPT:partial" => StreamItemState::Provisional,
        "SCRIPT:incomplete" => StreamItemState::Incomplete,
        "SCRIPT:failed" => StreamItemState::Failed,
        "SCRIPT:cancelled" => StreamItemState::Cancelled,
        "SCRIPT:unknown" => StreamItemState::Unknown,
        "SCRIPT:invalid" => StreamItemState::Complete,
        "SCRIPT:unselected" => {
            return Some(complete(
                DecisionCandidate::Advisory {
                    text: ADVISORY_TEXT.to_owned(),
                },
                OutputPhase::Commentary,
            ));
        }
        "SCRIPT:tool_partial" => {
            return Some(FakeCreate {
                items: vec![NativeStreamItem {
                    call_id: Some("call-1".to_owned()),
                    arguments: Some("{\"q\"".to_owned()),
                    ..evidence_item(
                        StreamItemRole::Tool,
                        StreamItemState::Provisional,
                        "partial_tool_arguments",
                    )
                }],
                output: None,
            });
        }
        "SCRIPT:refusal" => {
            return Some(FakeCreate {
                items: vec![NativeStreamItem {
                    refusal: Some("host_fake_refusal".to_owned()),
                    ..evidence_item(
                        StreamItemRole::Assistant,
                        StreamItemState::Complete,
                        "host_fake_refusal",
                    )
                }],
                output: None,
            });
        }
        _ => return None,
    };
    Some(FakeCreate {
        items: vec![assistant_item("1", state, ADVISORY_TEXT)],
        output: None,
    })
}
