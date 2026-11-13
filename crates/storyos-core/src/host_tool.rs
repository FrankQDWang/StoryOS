//! The Host Tools (ADR 0091): Tools whose Registration binds a built-in implementation that
//! StoryOS executes in its own process. A Host ToolCall has no Destination Attempt and no
//! Outbound Disclosure Event, and it gives the Agent no authority.

use crate::{
    FINISH_TOOL_SPEC, SUBMIT_RESEARCH_TOOL_SPEC, ToolIntake, ToolRegistrationKind, ToolResultText,
    ToolSpec,
};

/// The Tool Registration of the Host Tools. It has no server, credential, or destination.
#[derive(Debug, Eq, PartialEq)]
pub struct HostToolRegistration {
    pub id: &'static str,
    pub revision: &'static str,
    pub kind: ToolRegistrationKind,
    pub adapter_revision: &'static str,
    pub tools: &'static [&'static ToolSpec],
}

pub const HOST_TOOL_REGISTRATION: HostToolRegistration = HostToolRegistration {
    id: "host_tools",
    revision: "018f0000-0000-7001-8000-0000000e0101",
    kind: ToolRegistrationKind::Host,
    adapter_revision: "storyos.host-tool.v1",
    tools: &[&SUBMIT_RESEARCH_TOOL_SPEC, &FINISH_TOOL_SPEC],
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

/// The result of a `finish` call beside a refused call of the same response (ADR 0091 section 4).
pub const FINISH_DOES_NOT_COUNT_RESULT: &str = "Another call of this response was refused, so this \
finish call does not count and the work continues. Correct the refused call, and then call finish \
again.";

/// Executes one validated `finish` call. Its result is empty when no other call of its response
/// was refused, and otherwise it says that the work continues. The store executes each other
/// Host Tool.
pub fn execute_host_tool(intake: &ToolIntake, refused_beside: bool) -> Option<ToolResultText> {
    match intake {
        ToolIntake::Finish => Some(host_tool_result(if refused_beside {
            FINISH_DOES_NOT_COUNT_RESULT
        } else {
            ""
        })),
        ToolIntake::WebSearch { .. }
        | ToolIntake::WebFetch { .. }
        | ToolIntake::SubmitResearch { .. } => None,
    }
}

/// The result of a Host ToolCall with the text `text`.
pub fn host_tool_result(text: &str) -> ToolResultText {
    ToolResultText {
        text: text.to_owned(),
        digest: format!(
            "sha256:{}",
            crate::project_archive::hex_sha256(text.as_bytes())
        ),
        bytes: text.len(),
    }
}
