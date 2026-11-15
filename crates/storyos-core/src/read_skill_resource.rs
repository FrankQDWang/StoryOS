//! The Host Tool `read_skill_resource` (ADR 0089 sections 3, 7, and 9): the model reads one text
//! file of the Snapshot of a Skill that its Run has. The result is the exact text of the file, and
//! the next request sends it as the output of the call. The call has no external effect.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::assemble_context::count_context_item_tokens;
use crate::{
    CONTEXT_ITEM_TOKEN_LIMIT, IntakeRefusal, SkillPackageReference, TOOL_RESULT_TEXT_MAX_BYTES,
    ToolIntake, ToolOperation, ToolResultText, ToolSpec,
};

pub const READ_SKILL_RESOURCE_TOOL_SPEC: ToolSpec = ToolSpec {
    revision: "storyos.tool.read-skill-resource.v1",
    operation: ToolOperation::ReadSkillResource,
    intake_contract: &[],
    envelope: None,
    // A readable file has at most 32,000 counted tokens (ADR 0089 section 9).
    result_text_max_bytes: TOOL_RESULT_TEXT_MAX_BYTES,
};

/// The most settled reads of one Run (ADR 0089 section 9).
pub const SKILL_RESOURCE_READ_LIMIT: usize = 16;

pub(crate) const DESCRIPTION: &str = "Reads one file of the package of a Skill that this work \
has. Call it when the instructions of a Skill name a file of its package.";

/// The strict JSON Schema of the arguments.
pub(crate) fn parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "skill": {
                "type": "string",
                "description": "The name of a Skill that this work has."
            },
            "path": {
                "type": "string",
                "description": "The path of one file in the package of the Skill, relative to \
                                the root of the package, with / between the parts."
            }
        },
        "required": ["skill", "path"],
        "additionalProperties": false
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    skill: String,
    path: String,
}

/// The typed intake. A text with U+0000 does not fit, because the ToolCall row keeps the
/// arguments as `jsonb`. The Snapshot of the Run decides each other name and path.
pub(crate) fn intake(arguments: &Value) -> Result<ToolIntake, IntakeRefusal> {
    let Arguments { skill, path } =
        Arguments::deserialize(arguments).map_err(|_| IntakeRefusal::Malformed)?;
    if skill.contains('\0') || path.contains('\0') {
        return Err(IntakeRefusal::UnstorableText);
    }
    Ok(ToolIntake::ReadSkillResource { skill, path })
}

/// Why the Host refused one validated `read_skill_resource` call. The result gives the reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkillResourceRefusal {
    /// The Run does not have the Skill: the author did not select it, and no `load_skill` call
    /// of an earlier response loaded it.
    NotLoaded,
    /// The Run has `SKILL_RESOURCE_READ_LIMIT` settled reads.
    ReadLimit,
    /// No file of the Snapshot of the Skill has the exact path.
    NotFound,
    /// The file is not UTF-8 text, or it is above the token limit.
    NotReadable,
}

impl SkillResourceRefusal {
    /// The stable label of the refusal, for the `refusal` of the ToolCall row.
    pub fn label(self) -> &'static str {
        match self {
            Self::NotLoaded => "skill_not_loaded",
            Self::ReadLimit => "skill_resource_read_limit",
            Self::NotFound => "skill_resource_not_found",
            Self::NotReadable => "skill_resource_not_readable",
        }
    }

    /// The result of the refused call: the reason in one sentence.
    pub fn result(self) -> ToolResultText {
        let reason = match self {
            Self::NotLoaded => {
                "this run does not have this Skill: load it with load_skill first, and read its \
                 files in a later response"
                    .to_owned()
            }
            Self::ReadLimit => {
                format!("this run already read {SKILL_RESOURCE_READ_LIMIT} files of Skills")
            }
            Self::NotFound => {
                "the package of this Skill has no file with this exact path".to_owned()
            }
            Self::NotReadable => {
                "this file is not UTF-8 text or it is longer than 32,000 tokens".to_owned()
            }
        };
        crate::host_tool_result(&format!(
            "The Host refused this call because {reason}, so no result is available, and this is \
             not an empty result."
        ))
    }
}

/// The stored facts of a Run that decide one `read_skill_resource` call.
#[derive(Clone, Copy, Debug)]
pub struct SkillResourceReadFacts<'a> {
    /// The pinned catalog of the Run.
    pub catalog: &'a [SkillPackageReference],
    /// The name of each Skill that the author selected for the Run.
    pub selected: &'a [String],
    /// The Skill name of each settled `load_skill` call of an earlier response of the Run.
    pub loaded: &'a [String],
    /// The count of the settled `read_skill_resource` calls of the Run before this call.
    pub settled_reads: usize,
    /// The bytes of the file of the Snapshot of the named Skill whose path is the exact path of
    /// the call. `None` when no such file exists.
    pub file: Option<&'a [u8]>,
}

/// The result of a read of `path` in the Skill `skill`: the exact text of the file. The Run must
/// have the Skill, it must have fewer than `SKILL_RESOURCE_READ_LIMIT` settled reads, a file of
/// the Snapshot must have the exact path, and the file must be UTF-8 text of at most
/// `CONTEXT_ITEM_TOKEN_LIMIT` counted tokens, in this order.
pub fn admit_skill_resource_read(
    facts: &SkillResourceReadFacts<'_>,
    skill: &str,
    path: &str,
) -> Result<ToolResultText, SkillResourceRefusal> {
    let in_catalog = facts
        .catalog
        .iter()
        .any(|reference| reference.name == skill);
    let held = facts
        .selected
        .iter()
        .chain(facts.loaded)
        .any(|name| name == skill);
    if !(in_catalog && held) {
        return Err(SkillResourceRefusal::NotLoaded);
    }
    if facts.settled_reads >= SKILL_RESOURCE_READ_LIMIT {
        return Err(SkillResourceRefusal::ReadLimit);
    }
    // A package path is relative, with `/` between parts that are not empty and not `..`.
    let relative = !path.contains('\\') && path.split('/').all(|part| !matches!(part, "" | ".."));
    let file = facts
        .file
        .filter(|_| relative)
        .ok_or(SkillResourceRefusal::NotFound)?;
    let text = std::str::from_utf8(file)
        .ok()
        .filter(|text| count_context_item_tokens(text) <= CONTEXT_ITEM_TOKEN_LIMIT)
        .ok_or(SkillResourceRefusal::NotReadable)?;
    Ok(crate::host_tool_result(text))
}

#[cfg(test)]
#[path = "read_skill_resource_tests.rs"]
mod tests;
