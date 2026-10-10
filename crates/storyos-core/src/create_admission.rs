//! The Host decision before the dispatch claim of a new Create request.

use crate::{CREATE_REQUIREMENT, ExecutionCapability, model_registration};

/// The facts of one claimed AgentRun that Create admission examines.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateAdmissionFacts<'a> {
    pub author_message: &'a str,
    pub context_complete: bool,
    pub assistance_available: bool,
    pub route: RouteFacts<'a>,
}

/// The model route that one AgentRun pinned, compared with the current Project records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteFacts<'a> {
    /// The Model Registration revision that the AgentRun pinned.
    pub model_registration_revision: &'a str,
    /// The AgentRun pinned the current use binding and compatibility Decision of its Project.
    pub binding_current: bool,
    /// The pinned Registration is still the head Registration of its adapter.
    pub registration_current: bool,
}

/// Dispatch, or a refusal before the dispatch claim with its recorded capability reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateAdmission {
    Dispatch,
    Refuse(&'static str),
}

pub fn admit_create(facts: &CreateAdmissionFacts<'_>) -> CreateAdmission {
    if let Some(capability) = crate::requested_execution_capability(facts.author_message) {
        return CreateAdmission::Refuse(match capability {
            ExecutionCapability::Tool => "tool",
            ExecutionCapability::Mcp => "mcp",
            ExecutionCapability::Research => "research",
            ExecutionCapability::Embedding => "embedding",
            ExecutionCapability::Memory => "memory",
            ExecutionCapability::Skill => "skill",
            ExecutionCapability::Subrun => "subrun",
            ExecutionCapability::Eval => "eval",
        });
    }
    if !facts.context_complete || !facts.assistance_available {
        return CreateAdmission::Refuse("blocked_context");
    }
    match admit_route(&facts.route) {
        Ok(()) => CreateAdmission::Dispatch,
        Err(reason) => CreateAdmission::Refuse(reason),
    }
}

/// Admits a new Create submission on the pinned route, or names why the route refuses it. Each
/// Create Attempt, an automatic successor included, needs this current admission.
pub fn admit_route(route: &RouteFacts<'_>) -> Result<(), &'static str> {
    if !route.binding_current {
        return Err("model_use_binding_stale");
    }
    if !route.registration_current {
        return Err("model_registration_drift");
    }
    match model_registration(route.model_registration_revision) {
        None => Err("model_registration_unknown"),
        Some(registration)
            if !registration
                .capability_profile
                .qualifies(CREATE_REQUIREMENT) =>
        {
            Err("model_runtime_qualification_pending")
        }
        Some(_) => Ok(()),
    }
}
