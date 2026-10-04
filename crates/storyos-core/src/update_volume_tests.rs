use super::{
    ProjectLifecycle, UpdateVolume, UpdateVolumeApplied, UpdateVolumeConflict,
    UpdateVolumeNoEffect, UpdateVolumeRefusal, UpdateVolumeResult, VolumeJoin, update_volume,
};

fn command() -> UpdateVolume {
    UpdateVolume {
        volume_join: VolumeJoin::ExactScope,
        expected_tree_revision: 2,
        current_tree_revision: 2,
        current_lifecycle: ProjectLifecycle::Active,
        title: "Volume B".to_owned(),
        current_title: "Volume A".to_owned(),
        order: 2,
        current_order: 1,
        volume_count: 2,
    }
}

#[test]
fn a_matching_revision_rename_and_reorder_classifies_as_applied() {
    assert_eq!(
        update_volume(&command()),
        UpdateVolumeResult::Applied(UpdateVolumeApplied {
            title: "Volume B".to_owned(),
            order: 2,
            tree_revision: 3,
        })
    );
}

#[test]
fn a_stale_tree_revision_classifies_as_conflicted_with_zero_authority_effect() {
    let mut stale = command();
    stale.current_tree_revision = 3;
    assert_eq!(
        update_volume(&stale),
        UpdateVolumeResult::Conflicted(UpdateVolumeConflict::StaleTreeRevision)
    );
}

#[test]
fn an_unchanged_title_and_order_classifies_as_no_effect() {
    let mut unchanged = command();
    unchanged.title = unchanged.current_title.clone();
    unchanged.order = unchanged.current_order;
    assert_eq!(
        update_volume(&unchanged),
        UpdateVolumeResult::NoEffect(UpdateVolumeNoEffect::Unchanged)
    );
}

#[test]
fn a_wrong_scope_volume_classifies_as_refused_with_zero_authority_effect() {
    let mut wrong_scope = command();
    wrong_scope.volume_join = VolumeJoin::Invalid;
    assert_eq!(
        update_volume(&wrong_scope),
        UpdateVolumeResult::Refused(UpdateVolumeRefusal::InvalidVolumeJoin)
    );
}

#[test]
fn an_invalid_order_classifies_as_refused_with_zero_authority_effect() {
    let mut zero = command();
    zero.order = 0;
    assert_eq!(
        update_volume(&zero),
        UpdateVolumeResult::Refused(UpdateVolumeRefusal::InvalidOrder)
    );
    let mut past_end = command();
    past_end.order = 3;
    assert_eq!(
        update_volume(&past_end),
        UpdateVolumeResult::Refused(UpdateVolumeRefusal::InvalidOrder)
    );
}

#[test]
fn an_archived_project_classifies_as_refused_with_zero_authority_effect() {
    let mut archived = command();
    archived.current_lifecycle = ProjectLifecycle::Archived;
    assert_eq!(
        update_volume(&archived),
        UpdateVolumeResult::Refused(UpdateVolumeRefusal::ArchivedProject)
    );
}

#[test]
fn an_invalid_title_classifies_as_refused_with_zero_authority_effect() {
    let mut empty = command();
    empty.title.clear();
    assert_eq!(
        update_volume(&empty),
        UpdateVolumeResult::Refused(UpdateVolumeRefusal::InvalidTitle)
    );
    let mut too_long = command();
    too_long.title = "n".repeat(1025);
    assert_eq!(
        update_volume(&too_long),
        UpdateVolumeResult::Refused(UpdateVolumeRefusal::InvalidTitle)
    );
}
