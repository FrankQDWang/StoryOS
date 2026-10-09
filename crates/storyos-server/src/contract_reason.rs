use serde::de::DeserializeOwned;
use storyos_core::ReasonCode;

use super::*;

/// Maps one Core reason to the public reason that has the same stable code.
pub(super) fn contract_reason<W: DeserializeOwned>(
    reason: &impl ReasonCode,
) -> Result<W, ApiError> {
    serde_json::from_value(serde_json::Value::from(reason.code())).map_err(|_| {
        tracing::warn!(
            reason = "unmapped_contract_reason",
            "a Core reason has no public reason"
        );
        problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "project_store_unavailable",
            "The Project store is unavailable.",
        )
    })
}

#[cfg(test)]
#[path = "contract_reason_tests.rs"]
mod tests;
