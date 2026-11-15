use storyos_contracts::SkillPackageReference;

use super::resolve;

fn reference(name: &str, hex: &str) -> SkillPackageReference {
    SkillPackageReference {
        name: name.to_owned(),
        package_digest: format!("sha256:{}", hex.repeat(64 / hex.len())),
    }
}

/// The names of the resolved references, or the status and the problem code of the refusal.
fn resolved(selections: Option<&[SkillPackageReference]>) -> Result<Vec<String>, (u16, String)> {
    resolve(selections)
        .map(|references| {
            references
                .into_iter()
                .map(|reference| reference.name)
                .collect()
        })
        .map_err(|error| (error.0.as_u16(), error.1.1.code.clone()))
}

#[test]
fn a_selection_has_one_to_eight_well_formed_references_with_distinct_names() {
    let names: Vec<String> = (0..9).map(|index| format!("skill-{index}")).collect();
    let nine: Vec<_> = names.iter().map(|name| reference(name, "a")).collect();
    let refused = Err((422, "invalid_request_shape".to_owned()));

    assert_eq!(
        [
            resolved(None),
            resolved(Some(&nine[..8])),
            resolved(Some(&[])),
            resolved(Some(&nine)),
            resolved(Some(&[reference("voice", "a"), reference("voice", "b")])),
            resolved(Some(&[reference("Voice", "a")])),
            resolved(Some(&[reference("voice", "A")])),
        ],
        [
            Ok(Vec::new()),
            Ok(names[..8].to_vec()),
            refused.clone(),
            refused.clone(),
            refused.clone(),
            refused.clone(),
            refused,
        ]
    );
}
