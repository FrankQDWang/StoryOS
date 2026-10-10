//! The Host decision before the dispatch claim of a new Create request.

use crate::{CREATE_REQUIREMENT, ExecutionCapability, model_registration};

/// The facts of one claimed AgentRun that Create admission examines.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateAdmissionFacts<'a> {
    pub author_message: &'a str,
    pub context_complete: bool,
    pub assistance_available: bool,
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
    if !facts.binding_current {
        return CreateAdmission::Refuse("model_use_binding_stale");
    }
    if !facts.registration_current {
        return CreateAdmission::Refuse("model_registration_drift");
    }
    match model_registration(facts.model_registration_revision) {
        None => CreateAdmission::Refuse("model_registration_unknown"),
        Some(registration)
            if !registration
                .capability_profile
                .qualifies(CREATE_REQUIREMENT) =>
        {
            CreateAdmission::Refuse("model_runtime_qualification_pending")
        }
        Some(_) => CreateAdmission::Dispatch,
    }
}
