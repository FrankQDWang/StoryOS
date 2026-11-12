//! The tool loop profile of ADR 0091: a Model Registration whose request mapping declares the Host
//! Tools. The reply form profile of the other Registrations declares none. The two profiles exist
//! together until the reply form goes.

use crate::{
    CapabilityEntries, CapabilityEvidence, HOST_FAKE_CAPABILITY_PROFILE, HOST_FAKE_REGISTRATION,
    ModelCapabilityProfile, ModelRegistration,
};

/// The fake model mapping of the tool loop profile. Its script calls `finish` in the first
/// RunStep and replies in the next one.
pub const HOST_FAKE_TOOL_LOOP_MAPPING_REVISION: &str = "storyos.host-fake.mapping.v2";

const TOOL_LOOP_MAPPING_REVISIONS: &[&str] = &[HOST_FAKE_TOOL_LOOP_MAPPING_REVISION];

pub const HOST_FAKE_TOOL_LOOP_CAPABILITY_PROFILE: ModelCapabilityProfile = ModelCapabilityProfile {
    revision: "storyos.host-fake.capability.v2",
    entries: CapabilityEntries {
        function_calling: CapabilityEvidence::Qualified {
            evidence: "Contract proof of the Contract-Faithful Fake Destination (ADR 0091)",
        },
        ..HOST_FAKE_CAPABILITY_PROFILE.entries
    },
    ..HOST_FAKE_CAPABILITY_PROFILE
};

/// The second fake model profile.
pub const HOST_FAKE_TOOL_LOOP_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa18",
    mapping_revision: HOST_FAKE_TOOL_LOOP_MAPPING_REVISION,
    capability_profile: &HOST_FAKE_TOOL_LOOP_CAPABILITY_PROFILE,
    ..HOST_FAKE_REGISTRATION
};

impl ModelRegistration {
    /// Whether the request mapping declares the Host Tools.
    pub fn declares_host_tools(&self) -> bool {
        TOOL_LOOP_MAPPING_REVISIONS.contains(&self.mapping_revision)
    }
}
