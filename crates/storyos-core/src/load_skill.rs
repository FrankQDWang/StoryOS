//! The Host Tool `load_skill` (ADR 0089 sections 3 and 7): the model loads the complete
//! `SKILL.md` of one Skill of the pinned catalog of its Run. The result is the exact text of the
//! Snapshot, and the next request sends it as the output of the call. The call has no external
//! effect.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    IntakeRefusal, SkillPackageReference, TOOL_RESULT_TEXT_MAX_BYTES, ToolIntake, ToolOperation,
    ToolResultText, ToolSpec,
};

pub const LOAD_SKILL_TOOL_SPEC: ToolSpec = ToolSpec {
    revision: "storyos.tool.load-skill.v1",
    operation: ToolOperation::LoadSkill,
    intake_contract: &[],
    envelope: None,
    // A `SKILL.md` has at most 32,000 characters (ADR 0089 section 9), so at most 128,000 bytes.
    result_text_max_bytes: TOOL_RESULT_TEXT_MAX_BYTES,
};

/// The longest `reason` of a load, in characters.
const REASON_MAX_CHARS: usize = 512;

pub(crate) const DESCRIPTION: &str = "Loads the instructions of one Skill of the section Skill \
catalog. Call it when the description of a Skill fits the author request.";

/// The strict JSON Schema of the arguments.
pub(crate) fn parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "skill": {
                "type": "string",
                "description": "The name of one Skill of the section Skill catalog."
            },
            "reason": {
                "type": "string",
                "description": "One sentence that says why the Skill helps with the author request."
            }
        },
        "required": ["skill", "reason"],
        "additionalProperties": false
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    skill: String,
    reason: String,
}

/// The typed intake. A `reason` must have 1 to 512 characters, not only white space, and no
/// control character (Unicode category Cc). A `skill` with U+0000 does not fit, because the
/// ToolCall row keeps the arguments as `jsonb`. The catalog decides each other name.
pub(crate) fn intake(arguments: &Value) -> Result<ToolIntake, IntakeRefusal> {
    let Arguments { skill, reason } =
        Arguments::deserialize(arguments).map_err(|_| IntakeRefusal::Malformed)?;
    if reason.trim().is_empty()
        || reason.chars().count() > REASON_MAX_CHARS
        || reason.chars().any(char::is_control)
    {
        return Err(IntakeRefusal::Reason);
    }
    if skill.contains('\0') {
        return Err(IntakeRefusal::UnstorableText);
    }
    Ok(ToolIntake::LoadSkill { skill, reason })
}

/// The most Skills that one Run loads, the selected Skills included (ADR 0089 section 9).
pub const SKILL_LOAD_LIMIT: usize = 8;

/// Why the Host refused one validated `load_skill` call. The result gives the reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoadSkillRefusal {
    /// The pinned catalog of the Run has no Skill with the name.
    NotInCatalog,
    /// The Run loaded the Skill before, or the author selected it for the Run.
    AlreadyLoaded,
    /// The selected Skills and the settled loads of the Run are at the limit.
    LoadLimit,
}

impl LoadSkillRefusal {
    /// The stable label of the refusal, for the `refusal` of the ToolCall row.
    pub fn label(self) -> &'static str {
        match self {
            Self::NotInCatalog => "skill_not_in_catalog",
            Self::AlreadyLoaded => "skill_already_loaded",
            Self::LoadLimit => "skill_load_limit",
        }
    }

    /// The result of the refused call: the reason in one sentence.
    pub fn result(self) -> ToolResultText {
        let reason = match self {
            Self::NotInCatalog => {
                "the section Skill catalog has no Skill with this name".to_owned()
            }
            Self::AlreadyLoaded => "this run already has the instructions of this Skill".to_owned(),
            Self::LoadLimit => {
                format!("this run already has the instructions of {SKILL_LOAD_LIMIT} Skills")
            }
        };
        crate::host_tool_result(&format!(
            "The Host refused this call because {reason}, so no result is available, and this is \
             not an empty result."
        ))
    }
}

/// The stored facts of a Run that decide one `load_skill` call.
#[derive(Clone, Copy, Debug)]
pub struct SkillLoadFacts<'a> {
    /// The pinned catalog of the Run.
    pub catalog: &'a [SkillPackageReference],
    /// The name of each Skill that the author selected for the Run.
    pub selected: &'a [String],
    /// The Skill name of each settled `load_skill` call of the Run before this call.
    pub loaded: &'a [String],
}

/// The reference of the Skill that a load of `skill` names in the pinned catalog of the Run. The
/// name must be equal to the name of one Skill of the catalog, the Run must not have the Skill,
/// and the Run must have fewer than `SKILL_LOAD_LIMIT` Skills, in this order.
pub fn admit_skill_load<'a>(
    facts: &SkillLoadFacts<'a>,
    skill: &str,
) -> Result<&'a SkillPackageReference, LoadSkillRefusal> {
    let reference = facts
        .catalog
        .iter()
        .find(|reference| reference.name == skill)
        .ok_or(LoadSkillRefusal::NotInCatalog)?;
    let held = || facts.selected.iter().chain(facts.loaded);
    if held().any(|name| name == skill) {
        return Err(LoadSkillRefusal::AlreadyLoaded);
    }
    if held().count() >= SKILL_LOAD_LIMIT {
        return Err(LoadSkillRefusal::LoadLimit);
    }
    Ok(reference)
}

/// The result of an admitted load: the exact `SKILL.md` text of the Snapshot, with no change.
/// `None` is a Snapshot with no `SKILL.md` text: damaged evidence.
pub fn skill_load_result(skill_md: Option<Vec<u8>>) -> Option<ToolResultText> {
    let text = String::from_utf8(skill_md?).ok()?;
    Some(crate::host_tool_result(&text))
}

#[cfg(test)]
#[path = "load_skill_tests.rs"]
mod tests;
