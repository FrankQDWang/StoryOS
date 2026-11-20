//! The function calling entry of a Model Capability Profile and its author-authorized
//! qualification probe on the Agent Plan route (ADR 0049).

use crate::{
    CapabilityEvidence, ModelCapability, ModelCapabilityProfile, ModelRegistration,
    RuntimeQualification, model_registration,
};

impl ModelRegistration {
    /// True when the authorization link of a compatibility Decision has a requirement or the
    /// function calling entry of this Registration to qualify. Documentation alone leaves the
    /// entry pending.
    pub fn awaits_qualification(&self) -> bool {
        awaits(self.capability_profile, self.requirement)
    }
}

/// The rule of `ModelRegistration::awaits_qualification` for a Registration with `profile` and
/// `requirement`: the profile does not qualify the requirement, or its function calling entry is
/// only documented.
fn awaits(profile: &ModelCapabilityProfile, requirement: &[ModelCapability]) -> bool {
    !profile.qualifies(requirement)
        || matches!(
            profile.entries.function_calling,
            CapabilityEvidence::Documented { .. }
        )
}

/// One request of the function calling probe. The author message is its constant probe input,
/// and the request sends no Project content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolProbe {
    /// Asks for one call of the probe function `storyos_probe_echo`.
    FunctionCall,
    /// Continues the response of the latest function call probe with the output of that call.
    FunctionCallOutput,
}

impl ToolProbe {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FunctionCall => "function_call",
            Self::FunctionCallOutput => "function_call_output",
        }
    }

    /// The exact author message that asks for this probe.
    pub fn instruction(self) -> &'static str {
        match self {
            Self::FunctionCall => {
                "StoryOS Tool probe: call the function storyos_probe_echo one time with the text \
                 storyos-probe."
            }
            Self::FunctionCallOutput => {
                "StoryOS Tool probe: send the output of the storyos_probe_echo call."
            }
        }
    }
}

/// The probe that an author message asks for, only when the pinned Registration has the function
/// calling entry pending and the compatibility Decision is `qualification_authorized`. Otherwise
/// the message is an ordinary request.
pub fn tool_probe(
    author_message: &str,
    model_registration_revision: &str,
    qualification: RuntimeQualification,
) -> Option<ToolProbe> {
    let pending = matches!(
        model_registration(model_registration_revision)?
            .capability_profile
            .entries
            .function_calling,
        CapabilityEvidence::Documented { .. }
    );
    [ToolProbe::FunctionCall, ToolProbe::FunctionCallOutput]
        .into_iter()
        .find(|probe| {
            pending
                && qualification == RuntimeQualification::QualificationAuthorized
                && probe.instruction() == author_message
        })
}

/// The profile of the probe fixture: the current profile with the function calling entry
/// pending. Each other entry and each combination keep their qualification.
pub const PROBE_FIXTURE_CAPABILITY_PROFILE: ModelCapabilityProfile = ModelCapabilityProfile {
    revision: "storyos.volcengine-agent-plan.doubao-seed-2-1-pro-260915.capability.v3",
    entries: crate::model_registration::agent_plan_entries(CapabilityEvidence::Documented {
        source: "General Ark tool calling, docs.volcengine.com/docs/82379/1958524, not the Agent \
                 Plan route",
    }),
    context_window: crate::model_registration::AGENT_PLAN_CONTEXT_WINDOW,
    combinations: crate::model_registration::AGENT_PLAN_COMBINATIONS,
};

/// The probe fixture: the one Registration of the tests of ADR 0049, the current Registration
/// with the function calling entry pending, so a Decision with the authorization link dispatches
/// its probe. No deployment path binds it. The Registration of a later production model with a
/// pending entry replaces it (ADR 0097).
pub const PROBE_FIXTURE_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa16",
    capability_profile: &PROBE_FIXTURE_CAPABILITY_PROFILE,
    ..crate::AGENT_PLAN_REGISTRATION
};

#[cfg(test)]
#[path = "function_calling_tests.rs"]
mod tests;
