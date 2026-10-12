//! The one model destination that a deployment offers to each Project (ADR 0048).

use std::time::Duration;

use crate::{
    AGENT_PLAN_REGISTRATION, CREATE_REQUIREMENT, HOST_FAKE_REGISTRATION, ModelRegistration,
};

pub const AGENT_PLAN_ENDPOINT: &str = "https://ark.cn-beijing.volces.com/api/plan/v3";
pub const AGENT_PLAN_ACCOUNT_BOUNDARY: &str =
    "The Volcengine account of the deployment operator that holds the Agent Plan subscription";
/// The Credential Reference of the operator-quota key: the Worker setting that holds it (ADR 0050).
pub const AGENT_PLAN_CREDENTIAL_REFERENCE: &str = "settings:STORYOS_AGENT_PLAN_API_KEY";
pub const AGENT_PLAN_ELIGIBILITY_EVIDENCE: &str =
    "https://github.com/FrankQDWang/StoryOS/issues/392#issuecomment-5978758415";

/// The destination that the deployment configuration offers. Only a development or test
/// deployment offers the Contract-Faithful Fake Destination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeploymentDestination {
    HostFake,
    /// The operator-quota credential is `AGENT_PLAN_CREDENTIAL_REFERENCE`, never its value.
    VolcengineAgentPlan {
        /// The link to the author authorization of the runtime qualification (ADR 0049).
        qualification_authorization: Option<String>,
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
    /// The profile is pending, and the author authorized the runtime qualification (ADR 0049).
    QualificationAuthorized,
}

impl RuntimeQualification {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Qualified => "qualified",
            Self::Pending => "pending",
            Self::QualificationAuthorized => "qualification_authorized",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [
            Self::Qualified,
            Self::Pending,
            Self::QualificationAuthorized,
        ]
        .into_iter()
        .find(|qualification| qualification.as_str() == value)
    }
}

/// The author-adopted bounds of one real destination request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestBounds {
    /// The output token bound, reasoning tokens included.
    pub max_output_tokens: u32,
    pub timeout: Duration,
}

/// The request bounds of a use binding. A binding without known bounds cannot send a real request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BudgetBounds {
    /// The Contract-Faithful Fake Destination sends nothing.
    NotRequired,
    Unqualified,
    /// The bounds that the author adopted for the qualification on 2026-10-10 (ADR 0049).
    AgentPlanRequestV1,
    /// The bounds that the author adopted for a long Chapter on 2026-10-12 (ADR 0052).
    AgentPlanRequestV2,
}

impl BudgetBounds {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotRequired => "not_required",
            Self::Unqualified => "unqualified",
            Self::AgentPlanRequestV1 => "agent_plan_request_v1",
            Self::AgentPlanRequestV2 => "agent_plan_request_v2",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [
            Self::NotRequired,
            Self::Unqualified,
            Self::AgentPlanRequestV1,
            Self::AgentPlanRequestV2,
        ]
        .into_iter()
        .find(|bounds| bounds.as_str() == value)
    }

    pub fn request_bounds(self) -> Option<RequestBounds> {
        match self {
            Self::NotRequired | Self::Unqualified => None,
            Self::AgentPlanRequestV1 => Some(RequestBounds {
                max_output_tokens: 8192,
                timeout: Duration::from_secs(/*secs*/ 180),
            }),
            Self::AgentPlanRequestV2 => Some(RequestBounds {
                max_output_tokens: 32_768,
                timeout: Duration::from_secs(/*secs*/ 600),
            }),
        }
    }

    /// Known bounds, or bounds that the route does not need.
    pub fn known(self) -> bool {
        self == Self::NotRequired || self.request_bounds().is_some()
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

    /// The qualification of a new compatibility Decision for this destination.
    pub fn runtime_qualification(&self) -> RuntimeQualification {
        match self {
            _ if self
                .registration()
                .capability_profile
                .qualifies(CREATE_REQUIREMENT) =>
            {
                RuntimeQualification::Qualified
            }
            Self::VolcengineAgentPlan {
                qualification_authorization: Some(_),
                ..
            } => RuntimeQualification::QualificationAuthorized,
            Self::HostFake | Self::VolcengineAgentPlan { .. } => RuntimeQualification::Pending,
        }
    }

    /// The authorization that a new compatibility Decision records. Only an authorized
    /// qualification records one.
    pub fn qualification_authorization(&self) -> Option<&str> {
        match self {
            Self::VolcengineAgentPlan {
                qualification_authorization: Some(authorization),
                ..
            } if self.runtime_qualification() == RuntimeQualification::QualificationAuthorized => {
                Some(authorization)
            }
            Self::HostFake | Self::VolcengineAgentPlan { .. } => None,
        }
    }

    pub fn credential_reference(&self) -> Option<&'static str> {
        match self {
            Self::HostFake => None,
            Self::VolcengineAgentPlan { .. } => Some(AGENT_PLAN_CREDENTIAL_REFERENCE),
        }
    }

    pub fn budget_bounds(&self) -> BudgetBounds {
        match self {
            Self::HostFake => BudgetBounds::NotRequired,
            Self::VolcengineAgentPlan { .. } => BudgetBounds::AgentPlanRequestV2,
        }
    }
}
