//! The shared test support from `model_registration_tests.rs`.

use storyos_core::DeploymentDestination;

pub(crate) fn agent_plan() -> DeploymentDestination {
    DeploymentDestination::VolcengineAgentPlan {
        qualification_authorization: None,
    }
}
