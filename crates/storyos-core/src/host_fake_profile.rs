//! Identities of the Contract-Faithful Fake Destination that Host records still bind.

pub const HOST_FAKE_MAPPING_REVISION: &str = "storyos.host-fake.mapping.v1";
pub const ADVISORY_TEXT: &str =
    "This passage is inspectable Host-fake advice. It is not Authoritative State.";
pub const PROSE_CHANGE_TEXT: &str = "Guard the narrator voice in this passage.";
pub const SECOND_PROSE_CHANGE_TEXT: &str = "Keep the second block voice in this passage.";
pub const INLINE_PROSE_CHANGE_TEXT: &str = "narrator tone";
pub const INLINE_PROSE_CHANGE_SOURCE: &str = "narrator voice";
pub const STREAM_FIRST_TEXT: &str = "Guard";
pub const STREAM_SECOND_TEXT: &str = "Guard the narrator";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionCapability {
    Tool,
    Mcp,
    Research,
    Embedding,
    Memory,
    Skill,
    Subrun,
    Eval,
}

/// Canonical Proposal batches for one scripted fake-model stream.
pub fn stream_batch_plan(author_message: &str) -> Option<Vec<(u64, &'static str)>> {
    author_message
        .starts_with("Stream this passage:")
        .then_some(vec![
            (1, STREAM_FIRST_TEXT),
            (2, STREAM_SECOND_TEXT),
            (3, PROSE_CHANGE_TEXT),
        ])
}

/// Name the unavailable execution capability that the Host refuses before dispatch.
pub fn requested_execution_capability(author_message: &str) -> Option<ExecutionCapability> {
    let lowered = author_message.to_ascii_lowercase();
    [
        ("invoke a tool", ExecutionCapability::Tool),
        ("open an mcp", ExecutionCapability::Mcp),
        ("run a research fetch", ExecutionCapability::Research),
        ("embed this passage", ExecutionCapability::Embedding),
        ("extract conversation memory", ExecutionCapability::Memory),
        ("load a skill", ExecutionCapability::Skill),
        ("start a subrun", ExecutionCapability::Subrun),
        ("run an eval", ExecutionCapability::Eval),
    ]
    .into_iter()
    .find_map(|(needle, capability)| lowered.contains(needle).then_some(capability))
}
