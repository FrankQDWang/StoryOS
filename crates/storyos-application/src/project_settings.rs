//! Inputs and settlements of the Project setting commands.

use std::convert::Infallible;

use storyos_core::{
    ArchiveProjectApplied, ArchiveProjectConflict, ArchiveProjectNoEffect, UpdateProjectApplied,
    UpdateProjectConflict, UpdateProjectNoEffect,
};

use crate::{ActivityApplied, ProjectCommandSettlement};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateProjectInput {
    pub title: String,
    pub expected_revision: u64,
}

pub type UpdateProjectSettlement = ProjectCommandSettlement<
    ActivityApplied<UpdateProjectApplied>,
    UpdateProjectNoEffect,
    UpdateProjectConflict,
    Infallible,
>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveProjectInput {
    pub expected_revision: u64,
}

pub type ArchiveProjectSettlement = ProjectCommandSettlement<
    ActivityApplied<ArchiveProjectApplied>,
    ArchiveProjectNoEffect,
    ArchiveProjectConflict,
    Infallible,
>;
