//! The Host Tools (ADR 0091): Tools whose Registration binds a built-in implementation that
//! StoryOS executes in its own process. A Host ToolCall has no Destination Attempt and no
//! Outbound Disclosure Event, and it gives the Agent no authority.

use crate::{FINISH_TOOL_SPEC, ToolIntake, ToolRegistrationKind, ToolResultText, ToolSpec};

/// The Tool Registration of the Host Tools. It has no server, credential, or destination.
#[derive(Debug, Eq, PartialEq)]
pub struct HostToolRegistration {
    pub id: &'static str,
    pub revision: &'static str,
    pub kind: ToolRegistrationKind,
    pub tools: &'static [&'static ToolSpec],
}

pub const HOST_TOOL_REGISTRATION: HostToolRegistration = HostToolRegistration {
    id: "host_tools",
    revision: "018f0000-0000-7001-8000-0000000e0101",
    kind: ToolRegistrationKind::Host,
    tools: &[&FINISH_TOOL_SPEC],
};

/// The Host Tool of the pinned revisions of a ToolCall. `None` is a ToolCall of another
/// Registration.
pub fn host_tool(registration_revision: &str, spec_revision: &str) -> Option<&'static ToolSpec> {
    (registration_revision == HOST_TOOL_REGISTRATION.revision)
        .then(|| {
            HOST_TOOL_REGISTRATION
                .tools
                .iter()
                .copied()
                .find(|spec| spec.revision == spec_revision)
        })
        .flatten()
}

/// Executes one validated Host ToolCall. The result of `finish` is empty.
pub fn execute_host_tool(intake: &ToolIntake) -> Option<ToolResultText> {
    match intake {
        ToolIntake::Finish => Some(ToolResultText {
            text: String::new(),
            digest: format!("sha256:{}", crate::project_archive::hex_sha256(b"")),
            bytes: 0,
        }),
        ToolIntake::WebSearch { .. } | ToolIntake::WebFetch { .. } => None,
    }
}
