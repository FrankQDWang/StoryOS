use super::{ApiError, invalid_request_shape};
use storyos_contracts::SkillPackageReference;

/// The core references of the Skill selections of a request: 1 to 8 well-formed references with
/// distinct names. A request with no field selects nothing. The Host parses no Skill name from
/// the message text (ADR 0089 section 5).
pub(super) fn resolve(
    selections: Option<&[SkillPackageReference]>,
) -> Result<Vec<storyos_core::SkillPackageReference>, ApiError> {
    let Some(selections) = selections else {
        return Ok(Vec::new());
    };
    if selections.is_empty() || selections.len() > storyos_core::SKILL_SELECTION_LIMIT {
        return Err(invalid_request_shape());
    }
    let mut resolved: Vec<storyos_core::SkillPackageReference> =
        Vec::with_capacity(selections.len());
    for selection in selections {
        let reference = storyos_core::SkillPackageReference {
            name: selection.name.clone(),
            package_digest: selection.package_digest.clone(),
        };
        if !storyos_core::skill_reference_well_formed(&reference)
            || resolved
                .iter()
                .any(|earlier| earlier.name == reference.name)
        {
            return Err(invalid_request_shape());
        }
        resolved.push(reference);
    }
    Ok(resolved)
}

#[cfg(test)]
#[path = "skill_selections_tests.rs"]
mod tests;
