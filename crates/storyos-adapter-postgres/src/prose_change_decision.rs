use storyos_core::ProseChangeCandidate;

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
