//! Pure Core classification for Project assistance availability.

use super::ProjectPresence;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssistanceAvailability {
    Available,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssistanceBindingPresence {
    Uninitialized,
    Initialized {
        availability: AssistanceAvailability,
        revision: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateProjectAssistance {
    pub presence: ProjectPresence,
    pub binding: AssistanceBindingPresence,
    pub expected_revision: u64,
    pub requested: AssistanceAvailability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateProjectAssistanceResult {
    Initialized {
        availability: AssistanceAvailability,
        revision: u64,
    },
    Applied {
        availability: AssistanceAvailability,
        revision: u64,
    },
    NoEffect {
        reason: UpdateProjectAssistanceNoEffect,
    },
    Conflicted {
        reason: UpdateProjectAssistanceConflict,
    },
    Refused {
        reason: UpdateProjectAssistanceRefusal,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateProjectAssistanceNoEffect {
    AvailabilityUnchanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateProjectAssistanceConflict {
    StaleAssistanceRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateProjectAssistanceRefusal {
    MissingProject,
}

/// Classify one Project assistance setting against exact Scope presence and expected revision.
pub fn update_project_assistance(
    command: &UpdateProjectAssistance,
) -> UpdateProjectAssistanceResult {
    if command.presence == ProjectPresence::Absent {
        return UpdateProjectAssistanceResult::Refused {
            reason: UpdateProjectAssistanceRefusal::MissingProject,
        };
    }
    match command.binding {
        AssistanceBindingPresence::Uninitialized => {
            if command.expected_revision != 0 {
                return UpdateProjectAssistanceResult::Conflicted {
                    reason: UpdateProjectAssistanceConflict::StaleAssistanceRevision,
                };
            }
            UpdateProjectAssistanceResult::Initialized {
                availability: command.requested,
                revision: 1,
            }
        }
        AssistanceBindingPresence::Initialized {
            availability,
            revision,
        } => {
            if command.expected_revision != revision {
                return UpdateProjectAssistanceResult::Conflicted {
                    reason: UpdateProjectAssistanceConflict::StaleAssistanceRevision,
                };
            }
            if command.requested == availability {
                return UpdateProjectAssistanceResult::NoEffect {
                    reason: UpdateProjectAssistanceNoEffect::AvailabilityUnchanged,
                };
            }
            UpdateProjectAssistanceResult::Applied {
                availability: command.requested,
                revision: revision + 1,
            }
        }
    }
}

#[cfg(test)]
#[path = "update_project_assistance_tests.rs"]
mod tests;
