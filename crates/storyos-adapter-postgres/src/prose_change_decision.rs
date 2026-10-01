use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{
    FakeAttemptOutcome, FakeDecisionKind, NativeStreamItem, ProseChangeCandidate, StreamItemState,
};

pub(crate) struct PreparedProseChange {
    pub outcome: FakeAttemptOutcome,
    pub items: Vec<NativeStreamItem>,
    pub output: Option<Vec<ProseChangeCandidate>>,
}

pub(crate) async fn prepare(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    chapter_id: &str,
    author_message: &str,
    items: &[NativeStreamItem],
    outcome: &FakeAttemptOutcome,
) -> Result<PreparedProseChange, CompleteAgentRunError> {
    let mut outcome = outcome.clone();
    let mut items = items.to_vec();
    let targets =
        crate::admitted_proposal_target::load_admitted_targets(client, claim, chapter_id).await?;
    let producer_output = if author_message.starts_with("Revise these passages")
        || (targets.first().is_some_and(|target| target.collection)
            && matches!(
                outcome,
                FakeAttemptOutcome::Decision {
                    kind: FakeDecisionKind::ProseChange { .. },
                    ..
                }
            )) {
        let declared: Vec<_> = targets
            .iter()
            .map(|target| {
                (
                    target.chapter_id.clone(),
                    target.block_id.clone(),
                    target.revision_id.clone(),
                )
            })
            .collect();
        let candidates = storyos_core::produce_fake_prose_changes(&declared, author_message);
        let incomplete = author_message.ends_with("SCRIPT:incomplete");
        let unselected = author_message.ends_with("SCRIPT:unselected");
        if !storyos_core::prose_changes_match_targets(&declared, &candidates) || incomplete {
            outcome = FakeAttemptOutcome::NoDecision {
                reason: if incomplete {
                    storyos_core::NoDecisionReason::Incomplete
                } else {
                    storyos_core::NoDecisionReason::Invalid
                },
            };
            if incomplete && let Some(item) = items.first_mut() {
                item.state = StreamItemState::Incomplete;
            }
        } else if let FakeAttemptOutcome::Decision {
            kind: FakeDecisionKind::ProseChange { locations, .. },
            selected,
            advances_continuation,
        } = &mut outcome
        {
            *locations = Some(candidates.clone());
            if unselected {
                *selected = false;
                *advances_continuation = false;
            }
        }
        Some(candidates)
    } else {
        None
    };
    Ok(PreparedProseChange {
        outcome,
        items,
        output: producer_output,
    })
}

pub(crate) fn encode(
    payload: &mut serde_json::Value,
    output: Option<&[ProseChangeCandidate]>,
    locations: Option<&[storyos_contracts::ProseChangeLocationInspect]>,
) {
    if let Some(output) = output {
        let text = storyos_core::canonical_json(&serde_json::json!(output));
        if let Some(item) = payload["items"]
            .as_array_mut()
            .and_then(|items| items.first_mut())
        {
            item["text"] = serde_json::json!(text);
            item["summary"] = serde_json::json!("host_fake_native_prose_changes");
        }
        if payload["decision"]["kind"].as_str() == Some("prose_change") {
            payload["decision"]["producer_input"] = serde_json::json!(text);
        }
    }
    if let Some(locations) = locations {
        payload["decision"]["locations"] = serde_json::json!(locations);
    }
}
