//! The Skills that one Run used, for the Run read (ADR 0089 section 8). The usage record comes
//! from the Skill selection of the Run and from its stored `load_skill` calls. It has no table.

use serde_json::Value;

use crate::{LOAD_SKILL_TOOL_SPEC, SkillProvenance, SkillRequirement, ToolCallState, ToolIntake};

/// One Skill that a Run used.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkillUse {
    /// The author selected the Skill in the message of the Run.
    AuthorSelected { name: String },
    /// The Agent loaded the Skill with a `load_skill` call that StoryOS settled, for `reason`.
    AgentSelected { name: String, reason: String },
}

/// One stored `load_skill` call of a Run: its state and its stored arguments.
#[derive(Clone, Copy, Debug)]
pub struct StoredSkillLoad<'a> {
    pub state: ToolCallState,
    pub arguments: &'a Value,
}

/// The Skills that a Run used: each Skill of `selected` in the order of the Skill names, then
/// each settled load of `loads` in item order, with the reason of its call. A load that did not
/// settle gives no entry.
pub fn skill_uses(selected: &[SkillRequirement], loads: &[StoredSkillLoad<'_>]) -> Vec<SkillUse> {
    let mut selected: Vec<&SkillRequirement> = selected.iter().collect();
    selected.sort_by(|left, right| left.skill_name.cmp(&right.skill_name));
    let selected = selected
        .into_iter()
        .map(|requirement| match requirement.provenance {
            SkillProvenance::AuthorSelected => SkillUse::AuthorSelected {
                name: requirement.skill_name.clone(),
            },
        });
    let loaded = loads.iter().filter_map(|load| match load.state {
        ToolCallState::Settled => match LOAD_SKILL_TOOL_SPEC.intake(load.arguments) {
            Ok(ToolIntake::LoadSkill { skill, reason }) => Some(SkillUse::AgentSelected {
                name: skill,
                reason,
            }),
            // A settled load always has valid arguments.
            Ok(_) | Err(_) => None,
        },
        ToolCallState::Pending
        | ToolCallState::Dispatched
        | ToolCallState::Failed
        | ToolCallState::Unknown
        | ToolCallState::Refused
        | ToolCallState::AwaitingApproval => None,
    });
    selected.chain(loaded).collect()
}
