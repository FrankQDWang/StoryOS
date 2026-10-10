//! The one model destination that a deployment offers to each Project (ADR 0048).

use crate::{AGENT_PLAN_REGISTRATION, HOST_FAKE_REGISTRATION, ModelRegistration};

pub const AGENT_PLAN_ENDPOINT: &str = "https://ark.cn-beijing.volces.com/api/plan/v3";
pub const AGENT_PLAN_ACCOUNT_BOUNDARY: &str =
    "The Volcengine account of the deployment operator that holds the Agent Plan subscription";
pub const AGENT_PLAN_ELIGIBILITY_EVIDENCE: &str =
    "https://github.com/FrankQDWang/StoryOS/issues/392#issuecomment-5978758415";

/// The destination that the deployment configuration offers. Only a development or test
/// deployment offers the Contract-Faithful Fake Destination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeploymentDestination {
    HostFake,
    /// The operator-quota credential is the opaque `credential_reference`, never its value.
    VolcengineAgentPlan {
        credential_reference: String,
    },
}

/// The kind of a Processing Destination Identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DestinationKind {
    HostFake,
    VolcengineAgentPlan,
}

impl DestinationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HostFake => "host_fake",
            Self::VolcengineAgentPlan => "volcengine_agent_plan",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [Self::HostFake, Self::VolcengineAgentPlan]
            .into_iter()
            .find(|kind| kind.as_str() == value)
    }
}

/// Whether attributable runtime evidence qualifies the route of a compatibility Decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeQualification {
    Qualified,
    Pending,
}

impl RuntimeQualification {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Qualified => "qualified",
            Self::Pending => "pending",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [Self::Qualified, Self::Pending]
            .into_iter()
            .find(|qualification| qualification.as_str() == value)
    }
}

impl DeploymentDestination {
    pub fn kind(&self) -> DestinationKind {
        match self {
            Self::HostFake => DestinationKind::HostFake,
            Self::VolcengineAgentPlan { .. } => DestinationKind::VolcengineAgentPlan,
        }
    }

    pub fn registration(&self) -> &'static ModelRegistration {
        match self {
            Self::HostFake => &HOST_FAKE_REGISTRATION,
            Self::VolcengineAgentPlan { .. } => &AGENT_PLAN_REGISTRATION,
        }
    }
}
