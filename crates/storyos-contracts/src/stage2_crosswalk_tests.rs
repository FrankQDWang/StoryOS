use crate::repository_root;

use super::{GENERATED_STAGE2_CROSSWALK_PATH, generate_stage2_crosswalk};

#[test]
fn binding_evidence_paths_exist_in_the_worktree() {
    let root = repository_root();
    for binding in super::BINDINGS {
        assert!(
            root.join(binding.evidence).is_file(),
            "missing evidence file {} for {}",
            binding.evidence,
            binding.id
        );
    }
}

#[test]
fn checked_in_stage2_crosswalk_matches_fresh_generation() {
    let actual = std::fs::read(repository_root().join(GENERATED_STAGE2_CROSSWALK_PATH))
        .expect("checked-in Stage 2 crosswalk should exist");
    let expected = generate_stage2_crosswalk(repository_root())
        .expect("Stage 2 crosswalk generation should succeed");
    assert_eq!(actual, expected);
}
