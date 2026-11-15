//! The explicit Skill selection of one Run and its `skill_instruction` Context items (ADR 0089
//! sections 3, 5, and 7).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::assemble_context::{
    ConsideredSource, ContextSourceClass, CurrentPassageAssemblyRecord, ProjectionMode,
    SelectedProjection, count_context_item_tokens,
};
use crate::{SkillPackageReference, SkillSet};

/// The most Skills that one message selects.
pub const SKILL_SELECTION_LIMIT: usize = 8;

/// Why a Run has a Skill requirement.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillProvenance {
    /// The author selected the Skill in the message of the Run.
    AuthorSelected,
}

/// One Run Skill Requirement of an Operation Requirement.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillRequirement {
    pub skill_name: String,
    pub package_digest: String,
    pub provenance: SkillProvenance,
}

/// A selection that names a reference that the Server does not offer
/// (`skill_selection_unavailable`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SkillSelectionUnavailable;

/// Whether `reference` has the name rule of ADR 0089 and the digest form of the package digest.
pub fn skill_reference_well_formed(reference: &SkillPackageReference) -> bool {
    crate::skill_package::is_skill_name(&reference.name)
        && reference
            .package_digest
            .strip_prefix("sha256:")
            .is_some_and(|hex| {
                hex.len() == 64
                    && hex
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
}

/// The requirement of each selected Skill, in the order of the Skill names. Each selection must
/// be equal to the reference of one Skill of `offered`, which is the catalog that the Run pins.
pub fn admit_skill_selections(
    offered: &SkillSet,
    selections: &[SkillPackageReference],
) -> Result<Vec<SkillRequirement>, SkillSelectionUnavailable> {
    let mut admitted = BTreeMap::new();
    for selection in selections {
        if !offered
            .packages()
            .iter()
            .any(|package| package.reference() == *selection)
        {
            return Err(SkillSelectionUnavailable);
        }
        admitted.insert(&selection.name, &selection.package_digest);
    }
    Ok(admitted
        .into_iter()
        .map(|(name, digest)| SkillRequirement {
            skill_name: name.clone(),
            package_digest: digest.clone(),
            provenance: SkillProvenance::AuthorSelected,
        })
        .collect())
}

/// One requirement and the `SKILL.md` bytes of the Snapshot of the Project with its digest, or
/// `None` when the Snapshot has no such file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillInstructionSource {
    pub requirement: SkillRequirement,
    pub skill_md: Option<Vec<u8>>,
}

/// A Snapshot with no `SKILL.md` text: damaged evidence. The RunStep sends nothing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SkillInstructionUnavailable;

/// Records the requirements of `sources` in the Operation Requirement of `record`. Adds one
/// `skill_instruction` item for each source. The item has the exact `SKILL.md` text, and its
/// source version is the package digest. The Host does not shorten, merge, or rank the items. The reader admitted each
/// `SKILL.md` in the item token limit (ADR 0089 section 9).
pub fn assemble_skill_instructions(
    record: &mut CurrentPassageAssemblyRecord,
    sources: Vec<SkillInstructionSource>,
) -> Result<(), SkillInstructionUnavailable> {
    if sources.is_empty() {
        return Ok(());
    }
    let mut requirements = Vec::with_capacity(sources.len());
    for source in sources {
        let text = source
            .skill_md
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .ok_or(SkillInstructionUnavailable)?;
        let token_count = count_context_item_tokens(&text);
        record.considered.push(ConsideredSource {
            source_class: ContextSourceClass::SkillInstruction,
            source_version: source.requirement.package_digest.clone(),
            token_count,
            eligible: true,
        });
        record.selected.push(SelectedProjection {
            source_class: ContextSourceClass::SkillInstruction,
            source_version: source.requirement.package_digest.clone(),
            projection_mode: ProjectionMode::ExactRequired,
            token_count,
            content: text,
        });
        requirements.push(source.requirement);
    }
    record.operation_requirement.skill_requirements = Some(requirements);
    Ok(())
}

#[cfg(test)]
#[path = "skill_instruction_tests.rs"]
mod tests;
