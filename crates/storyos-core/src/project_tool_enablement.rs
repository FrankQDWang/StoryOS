//! Project Tool Enablement of the research Tool Registration (ADR 0083): the classification of
//! one enablement setting and the tool state that the assistance query reports.

use std::convert::Infallible;

use crate::transition_outcome::reason_codes;
use crate::{ToolRegistrationLifecycle, TransitionOutcome};

/// The selection that one enablement revision records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolEnablement {
    Enabled,
    Disabled,
}

impl ToolEnablement {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [Self::Enabled, Self::Disabled]
            .into_iter()
            .find(|enablement| enablement.as_str() == value)
    }
}

/// Whether the binding records of the current enablement match the offered Registration revision,
/// Credential Reference, and destination identity (ADR 0048 and ADR 0049 pattern).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnablementBinding {
    Current,
    Changed,
}

/// The current enablement revision of the Project.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentEnablement {
    pub enablement: ToolEnablement,
    pub revision: u64,
    pub binding: EnablementBinding,
}

/// One enablement setting against the offer of the deployment and the current revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SetProjectToolEnablement {
    /// The lifecycle of the offered Registration revision, or `None` when the deployment does
    /// not offer the requested Registration.
    pub offered: Option<ToolRegistrationLifecycle>,
    pub current: Option<CurrentEnablement>,
    pub requested: ToolEnablement,
}

/// The new enablement revision. An enabled revision binds the offered Registration revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SetProjectToolEnablementApplied {
    pub enablement: ToolEnablement,
    pub revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SetProjectToolEnablementNoEffect {
    EnablementUnchanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SetProjectToolEnablementRefusal {
    RegistrationNotOffered,
    /// The offered revision is quarantined or retired.
    RegistrationUnavailable,
}

reason_codes!(SetProjectToolEnablementNoEffect { EnablementUnchanged => "enablement_unchanged" });
reason_codes!(SetProjectToolEnablementRefusal {
    RegistrationNotOffered => "registration_not_offered",
    RegistrationUnavailable => "registration_unavailable",
});

pub type SetProjectToolEnablementResult = TransitionOutcome<
    SetProjectToolEnablementApplied,
    SetProjectToolEnablementNoEffect,
    Infallible,
    SetProjectToolEnablementRefusal,
>;

/// Classifies one enablement setting. An enablement whose binding records changed writes a new
/// revision with new records; a disablement keeps the records of the earlier revision as history.
pub fn set_project_tool_enablement(
    command: &SetProjectToolEnablement,
) -> SetProjectToolEnablementResult {
    let Some(lifecycle) = command.offered else {
        return TransitionOutcome::Refused(SetProjectToolEnablementRefusal::RegistrationNotOffered);
    };
    let unchanged = match (command.requested, command.current) {
        (ToolEnablement::Enabled, _) if lifecycle != ToolRegistrationLifecycle::Active => {
            return TransitionOutcome::Refused(
                SetProjectToolEnablementRefusal::RegistrationUnavailable,
            );
        }
        (ToolEnablement::Enabled, Some(current)) => {
            current.enablement == ToolEnablement::Enabled
                && current.binding == EnablementBinding::Current
        }
        (ToolEnablement::Disabled, Some(current)) => current.enablement == ToolEnablement::Disabled,
        (ToolEnablement::Disabled, None) => true,
        (ToolEnablement::Enabled, None) => false,
    };
    if unchanged {
        return TransitionOutcome::NoEffect(SetProjectToolEnablementNoEffect::EnablementUnchanged);
    }
    TransitionOutcome::Applied(SetProjectToolEnablementApplied {
        enablement: command.requested,
        revision: command.current.map_or(0, |current| current.revision) + 1,
    })
}

/// The research tool state of a Project in the assistance query. It names no server or key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectToolState {
    NotOffered,
    Disabled,
    Enabled,
    /// The enabled revision pins a quarantined or retired Registration revision.
    Quarantined,
}

/// The current enablement revision of a Project with the lifecycle of the Registration revision
/// that an enabled revision pins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PinnedEnablement {
    Disabled,
    Enabled(ToolRegistrationLifecycle),
}

impl ProjectToolState {
    /// The state of an offered Registration from the current enablement revision of the Project.
    pub fn offered(current: Option<PinnedEnablement>) -> Self {
        match current {
            None | Some(PinnedEnablement::Disabled) => Self::Disabled,
            Some(PinnedEnablement::Enabled(ToolRegistrationLifecycle::Active)) => Self::Enabled,
            Some(PinnedEnablement::Enabled(
                ToolRegistrationLifecycle::Quarantined | ToolRegistrationLifecycle::Retired,
            )) => Self::Quarantined,
        }
    }
}

#[cfg(test)]
#[path = "project_tool_enablement_tests.rs"]
mod tests;
