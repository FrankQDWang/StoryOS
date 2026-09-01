use super::*;

#[test]
fn protocol_example_path_is_admitted() {
    let admitted = admit_archive_path("canonical/project.json").expect("example path");
    assert_eq!(admitted.path, "canonical/project.json");
    assert_eq!(admitted.collision_key, "canonical/project.json");
}

#[test]
fn structural_refusals_match_the_pinned_profile() {
    assert_eq!(admit_archive_path(""), Err(ArchivePathRefusal::Empty));
    assert_eq!(
        admit_archive_path("/canonical/project.json"),
        Err(ArchivePathRefusal::Absolute)
    );
    assert_eq!(
        admit_archive_path("canonical/project.json/"),
        Err(ArchivePathRefusal::TrailingSeparator)
    );
    assert_eq!(
        admit_archive_path("canonical\\project.json"),
        Err(ArchivePathRefusal::Backslash)
    );
    assert_eq!(
        admit_archive_path("canonical//project.json"),
        Err(ArchivePathRefusal::RepeatedSeparator)
    );
    assert_eq!(
        admit_archive_path("canonical/./project.json"),
        Err(ArchivePathRefusal::DotSegment)
    );
    assert_eq!(
        admit_archive_path("../project.json"),
        Err(ArchivePathRefusal::DotSegment)
    );
    assert_eq!(
        admit_archive_path("C:project.json"),
        Err(ArchivePathRefusal::DrivePrefix)
    );
}

#[test]
fn length_and_depth_limits_are_exact() {
    let too_long = "a".repeat(ARCHIVE_PATH_MAX_BYTES + 1);
    assert_eq!(
        admit_archive_path(&too_long),
        Err(ArchivePathRefusal::TooLong)
    );
    let too_deep = (0..=ARCHIVE_PATH_MAX_SEGMENTS)
        .map(|index| format!("s{index}"))
        .collect::<Vec<_>>()
        .join("/");
    assert_eq!(
        admit_archive_path(&too_deep),
        Err(ArchivePathRefusal::TooDeep)
    );
}

#[test]
fn ascii_case_folding_does_not_rewrite_the_admitted_path() {
    let admitted = admit_archive_path("canonical/Project.json").expect("mixed case");
    assert_eq!(admitted.path, "canonical/Project.json");
    assert_eq!(admitted.collision_key, "canonical/project.json");
}

#[test]
fn combining_mark_paths_are_refused_rather_than_normalized() {
    assert_eq!(
        admit_archive_path("canonical/cafe\u{0301}.json"),
        Err(ArchivePathRefusal::NotAlreadyNfc)
    );
}
