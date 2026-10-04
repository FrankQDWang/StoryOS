use std::fmt::Debug;

use serde::de::DeserializeOwned;
use storyos_contracts as contracts;
use storyos_core::{
    ArchiveProjectConflict, ArchiveProjectNoEffect, CreateChapterConflict, CreateChapterRefusal,
    CreateVolumeConflict, CreateVolumeRefusal, DeleteChapterConflict, DeleteChapterNoEffect,
    DeleteChapterRefusal, DeleteVolumeConflict, DeleteVolumeNoEffect, DeleteVolumeRefusal,
    ReasonCode, UpdateChapterConflict, UpdateChapterNoEffect, UpdateChapterRefusal,
    UpdateProjectConflict, UpdateProjectNoEffect, UpdateVolumeConflict, UpdateVolumeNoEffect,
    UpdateVolumeRefusal,
};

use super::contract_reason;

fn assert_maps<C: ReasonCode + Debug + PartialEq, W: DeserializeOwned + Debug + PartialEq>(
    pairs: Vec<(C, W)>,
) {
    for (core, wire) in pairs {
        let mapped =
            contract_reason::<W>(&core).unwrap_or_else(|_| panic!("{core:?} has no wire reason"));
        assert_eq!(mapped, wire);
        assert_eq!(C::from_code(core.code()).as_ref(), Some(&core));
    }
}

#[test]
fn every_volume_reason_maps_to_its_public_reason() {
    assert_maps(vec![(
        CreateVolumeConflict::StaleTreeRevision,
        contracts::CreateVolumeConflictReason::StaleTreeRevision,
    )]);
    assert_maps(vec![
        (
            CreateVolumeRefusal::ArchivedProject,
            contracts::CreateVolumeRefusalReason::ArchivedProject,
        ),
        (
            CreateVolumeRefusal::InvalidTitle,
            contracts::CreateVolumeRefusalReason::InvalidTitle,
        ),
    ]);
    assert_maps(vec![(
        UpdateVolumeNoEffect::Unchanged,
        contracts::UpdateVolumeNoEffectReason::Unchanged,
    )]);
    assert_maps(vec![(
        UpdateVolumeConflict::StaleTreeRevision,
        contracts::UpdateVolumeConflictReason::StaleTreeRevision,
    )]);
    assert_maps(vec![
        (
            UpdateVolumeRefusal::ArchivedProject,
            contracts::UpdateVolumeRefusalReason::ArchivedProject,
        ),
        (
            UpdateVolumeRefusal::InvalidTitle,
            contracts::UpdateVolumeRefusalReason::InvalidTitle,
        ),
        (
            UpdateVolumeRefusal::InvalidOrder,
            contracts::UpdateVolumeRefusalReason::InvalidOrder,
        ),
        (
            UpdateVolumeRefusal::InvalidVolumeJoin,
            contracts::UpdateVolumeRefusalReason::InvalidVolumeJoin,
        ),
    ]);
    assert_maps(vec![(
        DeleteVolumeNoEffect::AlreadyRemoved,
        contracts::DeleteVolumeNoEffectReason::AlreadyRemoved,
    )]);
    assert_maps(vec![(
        DeleteVolumeConflict::StaleTreeRevision,
        contracts::DeleteVolumeConflictReason::StaleTreeRevision,
    )]);
    assert_maps(vec![
        (
            DeleteVolumeRefusal::ArchivedProject,
            contracts::DeleteVolumeRefusalReason::ArchivedProject,
        ),
        (
            DeleteVolumeRefusal::InvalidVolumeJoin,
            contracts::DeleteVolumeRefusalReason::InvalidVolumeJoin,
        ),
        (
            DeleteVolumeRefusal::NonemptyVolume,
            contracts::DeleteVolumeRefusalReason::NonemptyVolume,
        ),
    ]);
}

#[test]
fn every_chapter_reason_maps_to_its_public_reason() {
    assert_maps(vec![(
        CreateChapterConflict::StaleTreeRevision,
        contracts::CreateChapterConflictReason::StaleTreeRevision,
    )]);
    assert_maps(vec![
        (
            CreateChapterRefusal::ArchivedProject,
            contracts::CreateChapterRefusalReason::ArchivedProject,
        ),
        (
            CreateChapterRefusal::InvalidTitle,
            contracts::CreateChapterRefusalReason::InvalidTitle,
        ),
        (
            CreateChapterRefusal::InvalidVolumeJoin,
            contracts::CreateChapterRefusalReason::InvalidVolumeJoin,
        ),
        (
            CreateChapterRefusal::InvalidPlacement,
            contracts::CreateChapterRefusalReason::InvalidPlacement,
        ),
    ]);
    assert_maps(vec![(
        UpdateChapterNoEffect::Unchanged,
        contracts::UpdateChapterNoEffectReason::Unchanged,
    )]);
    assert_maps(vec![(
        UpdateChapterConflict::StaleTreeRevision,
        contracts::UpdateChapterConflictReason::StaleTreeRevision,
    )]);
    assert_maps(vec![
        (
            UpdateChapterRefusal::ArchivedProject,
            contracts::UpdateChapterRefusalReason::ArchivedProject,
        ),
        (
            UpdateChapterRefusal::InvalidTitle,
            contracts::UpdateChapterRefusalReason::InvalidTitle,
        ),
        (
            UpdateChapterRefusal::InvalidOrder,
            contracts::UpdateChapterRefusalReason::InvalidOrder,
        ),
        (
            UpdateChapterRefusal::InvalidChapterJoin,
            contracts::UpdateChapterRefusalReason::InvalidChapterJoin,
        ),
    ]);
    assert_maps(vec![(
        DeleteChapterNoEffect::AlreadyRemoved,
        contracts::DeleteChapterNoEffectReason::AlreadyRemoved,
    )]);
    assert_maps(vec![(
        DeleteChapterConflict::StaleTreeRevision,
        contracts::DeleteChapterConflictReason::StaleTreeRevision,
    )]);
    assert_maps(vec![
        (
            DeleteChapterRefusal::ArchivedProject,
            contracts::DeleteChapterRefusalReason::ArchivedProject,
        ),
        (
            DeleteChapterRefusal::InvalidChapterJoin,
            contracts::DeleteChapterRefusalReason::InvalidChapterJoin,
        ),
    ]);
}

#[test]
fn every_project_setting_reason_maps_to_its_public_reason() {
    assert_maps(vec![(
        UpdateProjectNoEffect::TitleUnchanged,
        contracts::UpdateProjectNoEffectReason::TitleUnchanged,
    )]);
    assert_maps(vec![(
        UpdateProjectConflict::StaleProjectRevision,
        contracts::UpdateProjectConflictReason::StaleProjectRevision,
    )]);
    assert_maps(vec![(
        ArchiveProjectNoEffect::AlreadyArchived,
        contracts::ArchiveProjectNoEffectReason::AlreadyArchived,
    )]);
    assert_maps(vec![(
        ArchiveProjectConflict::StaleProjectRevision,
        contracts::ArchiveProjectConflictReason::StaleProjectRevision,
    )]);
}
