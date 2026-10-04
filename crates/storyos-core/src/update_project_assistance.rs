//! Pure Core classification for Project assistance availability.

use std::convert::Infallible;

use super::TransitionOutcome;
use crate::transition_outcome::reason_codes;

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
    pub binding: AssistanceBindingPresence,
    pub expected_revision: u64,
    pub requested: AssistanceAvailability,
}

/// The applied assistance change; both kinds record an `authoritative_applied` Receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateProjectAssistanceApplied {
    /// The first assistance setting of the Project, which also creates its binding.
    Initialized {
        availability: AssistanceAvailability,
        revision: u64,
    },
    Changed {
        availability: AssistanceAvailability,
        revision: u64,
    },
}

pub type UpdateProjectAssistanceResult = TransitionOutcome<
    UpdateProjectAssistanceApplied,
    UpdateProjectAssistanceNoEffect,
    UpdateProjectAssistanceConflict,
    Infallible,
>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateProjectAssistanceNoEffect {
    AvailabilityUnchanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateProjectAssistanceConflict {
    StaleAssistanceRevision,
}

reason_codes!(UpdateProjectAssistanceNoEffect { AvailabilityUnchanged => "availability_unchanged" });
reason_codes!(UpdateProjectAssistanceConflict {
    StaleAssistanceRevision => "stale_assistance_revision",
});

/// Classify one Project assistance setting against the current binding and expected revision.
pub fn update_project_assistance(
    command: &UpdateProjectAssistance,
) -> UpdateProjectAssistanceResult {
    match command.binding {
        AssistanceBindingPresence::Uninitialized => {
            if command.expected_revision != 0 {
                return TransitionOutcome::Conflicted(
                    UpdateProjectAssistanceConflict::StaleAssistanceRevision,
                );
            }
            TransitionOutcome::Applied(UpdateProjectAssistanceApplied::Initialized {
                availability: command.requested,
                revision: 1,
            })
        }
        AssistanceBindingPresence::Initialized {
            availability,
            revision,
        } => {
            if command.expected_revision != revision {
                return TransitionOutcome::Conflicted(
                    UpdateProjectAssistanceConflict::StaleAssistanceRevision,
                );
            }
            if command.requested == availability {
                return TransitionOutcome::NoEffect(
                    UpdateProjectAssistanceNoEffect::AvailabilityUnchanged,
                );
            }
            TransitionOutcome::Applied(UpdateProjectAssistanceApplied::Changed {
                availability: command.requested,
                revision: revision + 1,
            })
        }
    }
}

#[cfg(test)]
#[path = "update_project_assistance_tests.rs"]
mod tests;
