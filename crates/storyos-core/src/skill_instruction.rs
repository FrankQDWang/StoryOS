//! The explicit Skill selection of one Run, the Skills that the author selected in the earlier
//! Runs of its Project Conversation, their `skill_instruction` Context items, and the
//! `skill_catalog` Context item of the pinned catalog (ADR 0089 sections 3, 5, and 7).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::assemble_context::{
    ConsideredSource, ContextSourceClass, CurrentPassageAssemblyRecord, ProjectionMode,
    SelectedProjection, count_context_item_tokens,
};
use crate::{SkillCatalogEntry, SkillPackageReference, SkillSet, hex_sha256, skill_catalog_text};

/// The most Skills that one message selects.
pub const SKILL_SELECTION_LIMIT: usize = 8;

/// The most Skills that the author selected that are in effect for one Run: the selections of
/// the Run and the Skills of its Project Conversation (ADR 0089 section 5).
pub const CONVERSATION_SKILL_LIMIT: usize = 8;

/// Why a Run has a Skill requirement.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillProvenance {
    /// The author selected the Skill in the message of the Run.
    AuthorSelected,
    /// The author selected the Skill in an earlier Run of the same Project Conversation, and the
    /// Run did not select it again (ADR 0089 section 5).
    ConversationSelected,
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

/// The `conversation_selected` requirements of a new Run, in the order of the Skill names.
/// `earlier` has the Skill requirements of each earlier admitted Run of the same Project
/// Conversation, oldest Run first. Only an `author_selected` requirement is a selection. For each
/// Skill name, the newest selection gives the package digest. The selections of the new Run in
/// `selected` are the newest, so they count in the limit of `CONVERSATION_SKILL_LIMIT` names, and
/// the new Run does not get them again. Inside one Run, the names count in name order.
pub fn conversation_skill_requirements(
    earlier: &[Vec<SkillRequirement>],
    selected: &[SkillRequirement],
) -> Vec<SkillRequirement> {
    let own: BTreeSet<&str> = selected
        .iter()
        .map(|requirement| requirement.skill_name.as_str())
        .collect();
    let mut in_effect = own.clone();
    let mut inherited = BTreeMap::new();
    let selections = earlier.iter().rev().flat_map(|run| {
        run.iter()
            .filter(|requirement| requirement.provenance == SkillProvenance::AuthorSelected)
    });
    for requirement in selections {
        if in_effect.len() == CONVERSATION_SKILL_LIMIT {
            break;
        }
        if in_effect.insert(requirement.skill_name.as_str()) {
            inherited.insert(&requirement.skill_name, &requirement.package_digest);
        }
    }
    inherited
        .into_iter()
        .map(|(name, digest)| SkillRequirement {
            skill_name: name.clone(),
            package_digest: digest.clone(),
            provenance: SkillProvenance::ConversationSelected,
        })
        .collect()
}

/// Whether the selected `item` of `record` is the `SKILL.md` of a `conversation_selected`
/// requirement. The Provider conversation holds that text after the first request that sends it,
/// so a request sends the item only with a full input and no prior response, and the current
/// input digest does not count it (ADR 0089 section 5).
pub fn is_conversation_skill(
    record: &CurrentPassageAssemblyRecord,
    item: &SelectedProjection,
) -> bool {
    item.source_class == ContextSourceClass::SkillInstruction
        && !crate::is_skill_load_output(record, item)
        && record
            .operation_requirement
            .skill_requirements
            .iter()
            .flatten()
            .any(|requirement| {
                requirement.provenance == SkillProvenance::ConversationSelected
                    && requirement.package_digest == item.source_version
            })
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

/// Records the references of `catalog`, the pinned catalog of the Run, in the Operation
/// Requirement of `record`, and adds one `skill_catalog` item with the catalog text. Call it after
/// `assemble_skill_instructions`, so that the item comes after the `skill_instruction` items. An
/// empty catalog adds nothing. The Server admitted the catalog text in its token limit (ADR 0089
/// section 9).
pub fn assemble_skill_catalog(
    record: &mut CurrentPassageAssemblyRecord,
    catalog: &[SkillCatalogEntry],
) {
    if catalog.is_empty() {
        return;
    }
    let text = skill_catalog_text(catalog);
    let token_count = count_context_item_tokens(&text);
    let source_version = format!("sha256:{}", hex_sha256(text.as_bytes()));
    record.considered.push(ConsideredSource {
        source_class: ContextSourceClass::SkillCatalog,
        source_version: source_version.clone(),
        token_count,
        eligible: true,
    });
    record.selected.push(SelectedProjection {
        source_class: ContextSourceClass::SkillCatalog,
        source_version,
        projection_mode: ProjectionMode::ExactRequired,
        token_count,
        content: text,
    });
    let mut references: Vec<_> = catalog
        .iter()
        .map(|entry| entry.reference.clone())
        .collect();
    references.sort();
    record.operation_requirement.skill_catalog = Some(references);
}

#[cfg(test)]
#[path = "skill_instruction_tests.rs"]
mod tests;
