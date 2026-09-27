//! Decide whether one staged active-context compaction may install.

use crate::{canonical_json, hex_sha256};

pub const ACTIVE_COMPACTION_REQUEST_PREFIX: &str = "Compact active context between calls.";
pub const HOST_FAKE_COMPACTION_PRODUCER: &str = "host_fake_summary";
pub const HOST_FAKE_COMPACTION_OUTPUT: &str =
    "Bounded later-request summary. Semantic preservation is unknown.";
pub const COMPACTION_LOSS_SEMANTIC_PRESERVATION_UNKNOWN: &str = "semantic_preservation_unknown";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompactionInstallRefusal {
    RestrictedSource,
    ExactRequiredUnsatisfied,
    ChangedInput,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactionInstallFacts {
    pub source_restricted: bool,
    pub exact_required_satisfied: bool,
    pub staged_input_digest: String,
    pub current_input_digest: String,
}

/// Author text that asks the Host-fake path to compact before the next request.
pub fn requests_active_compaction(author_message: &str) -> bool {
    author_message.starts_with(ACTIVE_COMPACTION_REQUEST_PREFIX)
}

/// Digest the exact author message and Working Target captured for one install check.
pub fn active_context_input_digest(
    author_message: &str,
    chapter_revision_id: &str,
    chapter_body: &str,
) -> String {
    format!(
        "sha256:{}",
        hex_sha256(
            canonical_json(&serde_json::json!({
                "author_message": author_message,
                "chapter_revision_id": chapter_revision_id,
                "chapter_body": chapter_body,
            }))
            .as_bytes(),
        )
    )
}

/// Install only when the staged input is still current, unrestricted, and exact-required complete.
pub fn decide_compaction_install(
    facts: &CompactionInstallFacts,
) -> Result<(), CompactionInstallRefusal> {
    if facts.source_restricted {
        return Err(CompactionInstallRefusal::RestrictedSource);
    }
    if !facts.exact_required_satisfied {
        return Err(CompactionInstallRefusal::ExactRequiredUnsatisfied);
    }
    if facts.staged_input_digest != facts.current_input_digest {
        return Err(CompactionInstallRefusal::ChangedInput);
    }
    Ok(())
}

#[cfg(test)]
#[path = "compact_active_context_tests.rs"]
mod tests;
