use storyos_application::{ClaimedAgentRun, CompleteAgentRun, CompleteAgentRunError, ModelUsage};
use storyos_core::{
    AgentDecisionKind, AgentDecisionOutcome, NativeStreamItem, ProseChangeCandidate,
    StreamItemRole, StreamItemState, stream_batch_plan,
};
use uuid::Uuid;

use crate::agent_run_work::{
    RunPhaseRow, WorkPhase, complete_database_error, requeue_generation, update_run,
};

/// The validated result of one Create exchange for the active decision Model Attempt.
pub(crate) struct CreateResult<'a> {
    pub attempt_id: &'a str,
    pub items: &'a [NativeStreamItem],
    pub outcome: AgentDecisionOutcome,
    pub producer_output: Option<&'a [ProseChangeCandidate]>,
    pub usage: ModelUsage,
    pub response_reference: Option<&'a str>,
    pub settlement: CreateSettlement<'a>,
}

/// How the destination answered one Create.
pub(crate) enum CreateSettlement<'a> {
    Response,
    NotSubmitted,
    Rejected(&'a str),
}

/// Records the stream, the Agent Decision, and any opened Proposal of one Create exchange.
pub(crate) async fn persist_create_result(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    result: CreateResult<'_>,
    retained_payload: &serde_json::Value,
) -> Result<WorkPhase, CompleteAgentRunError> {
    let author_message = run.author_message.as_str();
    let chapter_id = run.chapter_id.as_str();
    let (decision_id, hold) = match &result.outcome {
        AgentDecisionOutcome::NoDecision => (None, None),
        AgentDecisionOutcome::Decision {
            selected,
            advances_continuation,
            ..
        } => (
            (*selected).then(|| Uuid::now_v7().to_string()),
            (*selected && *advances_continuation).then_some("decision"),
        ),
    };
    let mut stream_hold = false;
    let mut locations = None;
    let opened_proposal = match (decision_id.as_deref(), &result.outcome) {
        (
            Some(decision_id),
            AgentDecisionOutcome::Decision {
                kind:
                    AgentDecisionKind::ProseChange {
                        text,
                        locations: produced,
                    },
                selected: true,
                ..
            },
        ) => {
            if let Some(record) = crate::candidate_revision_target::admitted(client, claim).await? {
                let revised = crate::revise_candidate_generation::apply(
                    client,
                    claim,
                    &record,
                    produced.as_deref().unwrap_or_default(),
                )
                .await?;
                locations = revised.locations;
                revised.proposal_id
            } else if produced.is_none() && author_message.starts_with("Revise this phrase:") {
                crate::open_inline_proposal::open_selected_inline_change(
                    client,
                    claim,
                    chapter_id,
                    decision_id,
                    text,
                )
                .await?
            } else if produced.is_none() && stream_batch_plan(author_message).is_some() {
                let (proposal_id, work) =
                    crate::stream_proposal_generation::apply_streamed_proposal(
                        client,
                        claim,
                        chapter_id,
                        decision_id,
                        author_message,
                    )
                    .await?;
                stream_hold = matches!(work, crate::stream_proposal_generation::StreamWork::Hold);
                proposal_id
            } else {
                let opened = crate::open_block_proposal::open_selected_prose_change(
                    client,
                    claim,
                    chapter_id,
                    decision_id,
                    text,
                    author_message,
                    produced.as_deref(),
                )
                .await?;
                locations = opened.locations;
                opened.proposal_id
            }
        }
        _ => None,
    };
    let mut payload = retained_payload.clone();
    payload["items"] = merge_items(&retained_payload["items"], result.items);
    payload["decision"] = match (&result.outcome, decision_id.as_deref()) {
        (
            AgentDecisionOutcome::Decision {
                kind,
                selected,
                advances_continuation,
            },
            Some(decision_id),
        ) => encode_decision(
            kind,
            decision_id,
            *selected,
            *advances_continuation,
            opened_proposal.as_deref(),
        ),
        _ => serde_json::Value::Null,
    };
    payload["usage"] = encode_usage(result.usage);
    if let Some(reference) = result.response_reference {
        payload["response_reference"] = serde_json::json!(reference);
    }
    match result.settlement {
        CreateSettlement::Response => {}
        CreateSettlement::NotSubmitted => {
            payload["submission"] = serde_json::json!("not_submitted")
        }
        CreateSettlement::Rejected(reason) => payload["rejection"] = serde_json::json!(reason),
    }
    crate::prose_change_decision::encode(
        &mut payload,
        result.producer_output,
        locations.as_deref(),
    );
    client
        .execute(
            "UPDATE storyos.model_attempts
                SET decision_id = $4::text::uuid,
                    dispatch_state = $5,
                    payload = $6::text::jsonb
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid
                AND attempt_role = 'decision' AND model_attempt_id=$7::text::uuid",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &decision_id,
                &if hold.is_some() {
                    "uncertain"
                } else {
                    "settled"
                },
                &payload.to_string(),
                &result.attempt_id,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    update_run(
        client,
        claim,
        if hold.is_some() {
            "claimed"
        } else {
            "completed"
        },
        /*settlement*/ None,
        /*clear_lease*/ hold.is_none(),
    )
    .await?;
    if stream_hold {
        return requeue_generation(client, claim).await;
    }
    Ok(match hold {
        Some(kind) => WorkPhase::Hold(kind),
        None => WorkPhase::Done(CompleteAgentRun::Settled),
    })
}

/// Keeps the committed stream items in order. An item with a known item ID replaces that item.
pub(crate) fn merge_items(
    committed: &serde_json::Value,
    events: &[NativeStreamItem],
) -> serde_json::Value {
    let mut items = committed.as_array().cloned().unwrap_or_default();
    for event in encode_items(events).as_array().into_iter().flatten() {
        match items
            .iter_mut()
            .find(|item| item["item_id"] == event["item_id"])
        {
            Some(item) => *item = event.clone(),
            None => items.push(event.clone()),
        }
    }
    serde_json::Value::Array(items)
}

pub(crate) fn encode_items(items: &[NativeStreamItem]) -> serde_json::Value {
    items
        .iter()
        .map(|item| {
            let phase = match item.state {
                StreamItemState::Provisional => "provisional",
                StreamItemState::Complete => "complete",
                StreamItemState::Incomplete => "incomplete",
                StreamItemState::Failed => "failed",
                StreamItemState::Cancelled => "cancelled",
                StreamItemState::Unknown => "unknown",
            };
            serde_json::json!({
                "item_id": item.item_id,
                "role": match item.role {
                    StreamItemRole::Assistant => "assistant",
                    StreamItemRole::Tool => "tool",
                    StreamItemRole::Hosted => "hosted",
                },
                "state": phase,
                "phase": phase,
                "text": item.text,
                "summary": item.summary,
                "call_id": item.call_id,
                "arguments": item.arguments,
                "refusal": item.refusal,
                "hosted_report": item.hosted_report
            })
        })
        .collect()
}

pub(crate) fn encode_decision(
    kind: &AgentDecisionKind,
    decision_id: &str,
    selected: bool,
    advances_continuation: bool,
    opened_proposal: Option<&str>,
) -> serde_json::Value {
    match kind {
        AgentDecisionKind::Advisory { text } => serde_json::json!({
            "kind": "advisory",
            "decision_id": decision_id,
            "selected": selected,
            "text": text,
            "authoritative": false,
            "advances_continuation": advances_continuation
        }),
        AgentDecisionKind::ProseChange { text, .. } => serde_json::json!({
            "kind": "prose_change",
            "decision_id": decision_id,
            "selected": selected,
            "text": text,
            "producer_input": text,
            "authoritative": false,
            "advances_continuation": advances_continuation,
            "opened_proposal": match opened_proposal {
                Some(proposal_id) => serde_json::json!({
                    "kind": "present",
                    "proposal_id": proposal_id
                }),
                None => serde_json::json!({ "kind": "absent" }),
            }
        }),
        AgentDecisionKind::Clarification { question } => serde_json::json!({
            "kind": "clarification",
            "decision_id": decision_id,
            "selected": selected,
            "question": question,
            "required_reply": question,
            "authoritative": false,
            "advances_continuation": advances_continuation
        }),
    }
}

pub(crate) fn encode_usage(usage: ModelUsage) -> serde_json::Value {
    match usage {
        ModelUsage::Reported {
            input_tokens,
            output_tokens,
        } => serde_json::json!({
            "kind": "reported",
            "input_tokens": input_tokens,
            "output_tokens": output_tokens
        }),
        ModelUsage::Estimated {
            input_tokens,
            output_tokens,
        } => serde_json::json!({
            "kind": "estimated",
            "input_tokens": input_tokens,
            "output_tokens": output_tokens
        }),
        ModelUsage::Unknown => serde_json::json!({ "kind": "unknown" }),
    }
}
