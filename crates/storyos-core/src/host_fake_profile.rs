//! Identities of the Contract-Faithful Fake Destination that Host records still bind.

pub const HOST_FAKE_MAPPING_REVISION: &str = "storyos.host-fake.mapping.v1";
pub const ADVISORY_TEXT: &str =
    "This passage is inspectable Host-fake advice. It is not Authoritative State.";
pub const PROSE_CHANGE_TEXT: &str = "Guard the narrator voice in this passage.";
pub const SECOND_PROSE_CHANGE_TEXT: &str = "Keep the second block voice in this passage.";

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
