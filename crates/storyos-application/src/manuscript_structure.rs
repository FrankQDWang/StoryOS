//! Inputs and settlements of the Manuscript Structure Transition commands.

use std::convert::Infallible;

use storyos_core::{
    CreateChapterConflict, CreateChapterCurrent, CreateChapterPlacement, CreateChapterRefusal,
    CreateVolumeConflict, CreateVolumeRefusal, DeleteChapterConflict, DeleteChapterCurrent,
    DeleteChapterNoEffect, DeleteChapterRefusal, DeleteVolumeConflict, DeleteVolumeNoEffect,
    DeleteVolumeRefusal, UpdateChapterApplied, UpdateChapterConflict, UpdateChapterNoEffect,
    UpdateChapterRefusal, UpdateVolumeApplied, UpdateVolumeConflict, UpdateVolumeNoEffect,
    UpdateVolumeRefusal,
};

use crate::{ChapterId, StructureSettlement, VolumeId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateVolumeInput {
    pub title: String,
    pub expected_tree_revision: u64,
}

/// The Canonical Sibling Order that a Create Volume acknowledgement reports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateVolumePublicOrder {
    CanonicalSiblingOrder(u64),
    HistoricalCreateVolumeAck,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolumeCreated {
    pub volume_id: String,
    pub tree_revision: u64,
    pub order: CreateVolumePublicOrder,
}

pub type CreateVolumeSettlement =
    StructureSettlement<VolumeCreated, Infallible, CreateVolumeConflict, CreateVolumeRefusal>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateVolumeInput {
    pub volume_id: VolumeId,
    pub title: String,
    pub order: u64,
    pub expected_tree_revision: u64,
}

pub type UpdateVolumeSettlement = StructureSettlement<
    UpdateVolumeApplied,
    UpdateVolumeNoEffect,
    UpdateVolumeConflict,
    UpdateVolumeRefusal,
>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteVolumeInput {
    pub volume_id: VolumeId,
    pub expected_tree_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolumeDeleted {
    pub volume_id: String,
    pub tree_revision: u64,
}

pub type DeleteVolumeSettlement = StructureSettlement<
    VolumeDeleted,
    DeleteVolumeNoEffect,
    DeleteVolumeConflict,
    DeleteVolumeRefusal,
>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateChapterInput {
    pub volume_id: String,
    pub title: String,
    pub placement: CreateChapterPlacement,
    pub expected_tree_revision: u64,
}

/// The Canonical Sibling Order that a Create Chapter acknowledgement reports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateChapterPublicOrder {
    CanonicalSiblingOrder(u64),
    /// A historical Activity storage key, reported when the Receipt has no order.
    HistoricalCreateChapterAck(u64),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChapterCreated {
    pub chapter_id: String,
    pub tree_revision: u64,
    pub current: CreateChapterCurrent,
    pub order: CreateChapterPublicOrder,
}

pub type CreateChapterSettlement =
    StructureSettlement<ChapterCreated, Infallible, CreateChapterConflict, CreateChapterRefusal>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateChapterInput {
    pub chapter_id: ChapterId,
    pub title: String,
    pub order: u64,
    pub expected_tree_revision: u64,
}

pub type UpdateChapterSettlement = StructureSettlement<
    UpdateChapterApplied,
    UpdateChapterNoEffect,
    UpdateChapterConflict,
    UpdateChapterRefusal,
>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteChapterInput {
    pub chapter_id: ChapterId,
    pub expected_tree_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChapterDeleted {
    pub volume_id: String,
    pub tree_revision: u64,
    pub current: DeleteChapterCurrent,
}

pub type DeleteChapterSettlement = StructureSettlement<
    ChapterDeleted,
    DeleteChapterNoEffect,
    DeleteChapterConflict,
    DeleteChapterRefusal,
>;
