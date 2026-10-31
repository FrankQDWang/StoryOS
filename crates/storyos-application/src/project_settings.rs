//! Inputs and settlements of the Project setting commands.

use std::convert::Infallible;

use storyos_core::{UpdateProjectApplied, UpdateProjectConflict, UpdateProjectNoEffect};

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
