use std::convert::Infallible;

use storyos_core::{
    CancelAgentRunConflict, CancelAgentRunNoEffect, PauseAgentRunConflict, PauseAgentRunNoEffect,
    SteerAgentRunConflict, SteerAgentRunNoEffect,
};

use crate::{ActivityApplied, Project, ProjectCommandSettlement, RefusableCommandError};

/// One pauseAgentRun of an existing AgentRun.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PauseAgentRunInput {
    pub run_id: String,
}

/// The applied effect of one pauseAgentRun: the Run is paused at a new fence generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PauseAgentRunApplied {
    pub run_id: String,
    pub fence_generation: u64,
}

/// The refusal of an AgentRun control command before its Admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentRunControlRefusal {
    /// The AgentRun is not in exact Scope, or the steering conversation is not its conversation.
    MissingRun,
    /// The steering input makes the effective author input exceed the Context item bound.
    InputLimit,
}

pub type PauseAgentRunSettlement = ProjectCommandSettlement<
    ActivityApplied<PauseAgentRunApplied>,
    PauseAgentRunNoEffect,
    PauseAgentRunConflict,
    Infallible,
>;

pub type PauseAgentRunError = RefusableCommandError<AgentRunControlRefusal>;

/// One cancelAgentRun of an existing AgentRun.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CancelAgentRunInput {
    pub run_id: String,
}

/// The applied effect of one cancelAgentRun: the Run is cancelled at a new fence generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CancelAgentRunApplied {
    pub run_id: String,
    pub fence_generation: u64,
}

pub type CancelAgentRunSettlement = ProjectCommandSettlement<
    ActivityApplied<CancelAgentRunApplied>,
    CancelAgentRunNoEffect,
    CancelAgentRunConflict,
    Infallible,
>;

pub type CancelAgentRunError = RefusableCommandError<AgentRunControlRefusal>;

/// One steerAgentRun: a correction for an existing AgentRun in its conversation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SteerAgentRunInput {
    pub run_id: String,
    pub conversation_id: String,
    pub author_message: String,
}

/// The steering input that one steerAgentRun retains at its input position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SteeringRetained {
    pub run_id: String,
    pub steering_input_id: String,
    pub input_position: u64,
}

pub type SteerAgentRunSettlement = ProjectCommandSettlement<
    ActivityApplied<Infallible>,
    SteerAgentRunNoEffect,
    SteerAgentRunConflict,
    Infallible,
    Project,
    SteeringRetained,
>;

pub type SteerAgentRunError = RefusableCommandError<AgentRunControlRefusal>;
