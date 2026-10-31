use super::{GENERATED_CROSSWALK_PATH, generate_crosswalk, repository_root};

#[test]
fn checked_in_crosswalk_matches_fresh_generation() {
    let actual = std::fs::read(repository_root().join(GENERATED_CROSSWALK_PATH))
        .expect("checked-in crosswalk should exist");
    let expected =
        generate_crosswalk(repository_root()).expect("crosswalk generation should succeed");
    assert_eq!(actual, expected);
}
