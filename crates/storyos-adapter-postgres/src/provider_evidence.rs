//! The Provider evidence of a Model Attempt on a real route. The fake destination keeps its own
//! report, because it reaches no Provider.

/// The report of a real Attempt before its exchange settles.
pub(crate) const PENDING_REPORT: &str = "provider_usage_pending";

/// Whether the Attempt payload records network I/O, which only a real route does.
pub(crate) fn real_route(payload: &serde_json::Value) -> bool {
    payload.pointer("/execution_profile/network_io") == Some(&serde_json::Value::Bool(true))
}

/// Sets the Provider report of a real Attempt to its usage classification, and lists its
/// response reference as a known response identifier.
pub(crate) fn settle(
    payload: &mut serde_json::Value,
    usage_kind: &str,
    response_reference: Option<&str>,
) {
    if !real_route(payload) {
        return;
    }
    let attempt_id = payload
        .pointer("/evidence/0/attempt_id")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let Some(evidence) = payload
        .get_mut("evidence")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };
    for item in evidence.iter_mut() {
        if item.get("kind").and_then(serde_json::Value::as_str) == Some("provider_report") {
            item["report"] = serde_json::json!(format!("provider_usage_{usage_kind}"));
        }
    }
    if let Some(reference) = response_reference {
        let position = evidence
            .iter()
            .position(|item| {
                item.get("kind").and_then(serde_json::Value::as_str) == Some("provider_report")
            })
            .unwrap_or(evidence.len());
        evidence.insert(
            position,
            serde_json::json!({
                "kind": "stored_reference",
                "attempt_id": attempt_id,
                "availability": "current",
                "reference_id": reference
            }),
        );
    }
}

#[cfg(test)]
#[path = "provider_evidence_tests.rs"]
mod tests;
