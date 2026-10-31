use crate::repository_root;

use super::{GENERATED_STAGE3_CROSSWALK_PATH, generate_stage3_crosswalk};

#[test]
fn binding_evidence_paths_exist_in_the_worktree() {
    let root = repository_root();
    for binding in super::BINDINGS {
        for path in
            std::iter::once(binding.evidence).chain(binding.supporting_evidence.iter().copied())
        {
            assert!(
                root.join(path).is_file(),
                "missing evidence file {path} for {}",
                binding.id
            );
        }
    }
}

#[test]
fn checked_in_stage3_crosswalk_matches_fresh_generation() {
    let actual = std::fs::read(repository_root().join(GENERATED_STAGE3_CROSSWALK_PATH))
        .expect("checked-in Stage 3 crosswalk should exist");
    let expected = generate_stage3_crosswalk(repository_root())
        .expect("Stage 3 crosswalk generation should succeed");
    assert_eq!(actual, expected);
}
