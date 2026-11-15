use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One Skill that a Run used (ADR 0089 section 8). No entry has a digest or a ToolCall identity.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "provenance", rename_all = "snake_case", deny_unknown_fields)]
pub enum SkillUseInspect {
    /// The author selected the Skill in the message of the Run.
    AuthorSelected { name: String },
    /// The Agent loaded the Skill with a settled `load_skill` call that gave `reason`.
    AgentSelected { name: String, reason: String },
}
