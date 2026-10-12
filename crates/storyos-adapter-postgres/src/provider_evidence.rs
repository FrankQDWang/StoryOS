//! The Provider evidence of a Model Attempt on a real route. The fake destination keeps its own
//! report, because it reaches no Provider.

use storyos_application::{AttemptCache, CacheSetting, CacheUse};

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

/// The cache setting of a real request, before the destination reports its cache use.
pub(crate) fn encode_cache_setting(setting: CacheSetting) -> serde_json::Value {
    match setting {
        CacheSetting::DestinationDefault => serde_json::json!({
            "implicit": "destination_default",
            "explicit": "not_requested",
            "observed": encode_cache_use(CacheUse::Unknown)
        }),
    }
}

/// The reported cache use. StoryOS infers no cache hit and no saving from it.
pub(crate) fn encode_cache_use(cache_use: CacheUse) -> serde_json::Value {
    match cache_use {
        CacheUse::Reported {
            cached_input_tokens,
        } => serde_json::json!({ "kind": "reported", "cached_input_tokens": cached_input_tokens }),
        CacheUse::Unknown => serde_json::json!({ "kind": "unknown" }),
    }
}

/// The cache facts of an Attempt payload. `None` when the Attempt recorded no cache setting.
pub(crate) fn decode_cache(payload: &serde_json::Value) -> Option<AttemptCache> {
    let cache = payload.get("cache")?;
    let setting = match (
        cache.get("implicit")?.as_str()?,
        cache.get("explicit")?.as_str()?,
    ) {
        ("destination_default", "not_requested") => CacheSetting::DestinationDefault,
        _ => return None,
    };
    let observed = match cache.pointer("/observed/kind")?.as_str()? {
        "reported" => CacheUse::Reported {
            cached_input_tokens: cache.pointer("/observed/cached_input_tokens")?.as_u64()?,
        },
        _ => CacheUse::Unknown,
    };
    Some(AttemptCache { setting, observed })
}

#[cfg(test)]
#[path = "provider_evidence_tests.rs"]
mod tests;
