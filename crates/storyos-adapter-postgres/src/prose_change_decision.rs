use storyos_core::ProseChangeCandidate;

/// Binds the producer input and the locations of a prose change to its decision. The native
/// stream items stay as the destination reported them.
pub(crate) fn encode(
    payload: &mut serde_json::Value,
    output: Option<&[ProseChangeCandidate]>,
    locations: Option<&[storyos_contracts::ProseChangeLocationInspect]>,
) {
    if let Some(output) = output {
        let text = storyos_core::canonical_json(&serde_json::json!(output));
        if payload["decision"]["kind"].as_str() == Some("prose_change") {
            payload["decision"]["producer_input"] = serde_json::json!(text);
        }
    }
    if let Some(locations) = locations {
        payload["decision"]["locations"] = serde_json::json!(locations);
    }
}
