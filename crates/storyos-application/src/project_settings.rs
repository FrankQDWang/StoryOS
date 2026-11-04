//! Inputs and settlements of the Project setting commands.

use std::convert::Infallible;

use storyos_core::{
    ResearchToolOffer, SetProjectToolEnablementApplied, SetProjectToolEnablementNoEffect,
    SetProjectToolEnablementRefusal, ToolEnablement, UpdateProjectApplied, UpdateProjectConflict,
    UpdateProjectNoEffect,
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

/// One author request to enable or disable one Tool Registration for a Project (ADR 0083).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetProjectToolEnablementInput {
    pub registration_id: String,
    pub requested: ToolEnablement,
    /// The research destination that the deployment offers. The Host supplies it from its
    /// configuration. `None` offers no Tool Registration.
    pub offer: Option<&'static ResearchToolOffer>,
}

pub type SetProjectToolEnablementSettlement = ProjectCommandSettlement<
    ActivityApplied<SetProjectToolEnablementApplied>,
    SetProjectToolEnablementNoEffect,
    Infallible,
    SetProjectToolEnablementRefusal,
>;
