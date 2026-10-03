use super::{
    DeleteVolume, DeleteVolumeApplied, DeleteVolumeConflict, DeleteVolumeNoEffect,
    DeleteVolumeRefusal, DeleteVolumeResult, VolumeChildPolicy, VolumeRemovalLifecycle,
    delete_volume,
};
use crate::{ProjectLifecycle, VolumeJoin};

fn command() -> DeleteVolume {
    DeleteVolume {
        volume_join: VolumeJoin::ExactScope,
        volume_lifecycle: VolumeRemovalLifecycle::Active,
        child_chapters: VolumeChildPolicy::Empty,
        expected_tree_revision: 4,
        current_tree_revision: 4,
        current_lifecycle: ProjectLifecycle::Active,
    }
}

#[test]
fn removing_an_empty_volume_applies_the_next_tree_revision() {
    assert_eq!(
        delete_volume(&command()),
        DeleteVolumeResult::Applied(DeleteVolumeApplied { tree_revision: 5 }),
    );
}

#[test]
fn a_nonempty_volume_is_refused_with_zero_authority_effect() {
    let nonempty = DeleteVolume {
        child_chapters: VolumeChildPolicy::Nonempty,
        ..command()
    };
    assert_eq!(
        delete_volume(&nonempty),
        DeleteVolumeResult::Refused(DeleteVolumeRefusal::NonemptyVolume),
    );
}

#[test]
fn an_already_removed_volume_is_a_no_effect_retry() {
    let removed = DeleteVolume {
        volume_lifecycle: VolumeRemovalLifecycle::Removed,
        child_chapters: VolumeChildPolicy::Empty,
        ..command()
    };
    assert_eq!(
        delete_volume(&removed),
        DeleteVolumeResult::NoEffect(DeleteVolumeNoEffect::AlreadyRemoved),
    );
}

#[test]
fn a_stale_tree_revision_is_conflicted_with_zero_authority_effect() {
    let stale = DeleteVolume {
        expected_tree_revision: 3,
        ..command()
    };
    assert_eq!(
        delete_volume(&stale),
        DeleteVolumeResult::Conflicted(DeleteVolumeConflict::StaleTreeRevision),
    );
}

#[test]
fn an_invalid_volume_join_is_refused_with_zero_authority_effect() {
    let invalid = DeleteVolume {
        volume_join: VolumeJoin::Invalid,
        ..command()
    };
    assert_eq!(
        delete_volume(&invalid),
        DeleteVolumeResult::Refused(DeleteVolumeRefusal::InvalidVolumeJoin),
    );
}

#[test]
fn an_archived_project_is_refused_with_zero_authority_effect() {
    let archived = DeleteVolume {
        current_lifecycle: ProjectLifecycle::Archived,
        ..command()
    };
    assert_eq!(
        delete_volume(&archived),
        DeleteVolumeResult::Refused(DeleteVolumeRefusal::ArchivedProject),
    );
}
