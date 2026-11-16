//! The Host Tools (ADR 0091): Tools whose Registration binds a built-in implementation that
//! StoryOS executes in its own process. A Host ToolCall has no Destination Attempt and no
//! Outbound Disclosure Event, and it gives the Agent no authority.

use crate::{
    FINISH_TOOL_SPEC, LOAD_SKILL_TOOL_SPEC, ModelRegistration, PROPOSE_EDIT_TOOL_SPEC,
    READ_SKILL_RESOURCE_TOOL_SPEC, READ_TOOL_RESULT_TOOL_SPEC, SUBMIT_RESEARCH_TOOL_SPEC,
    ToolIntake, ToolRegistrationKind, ToolResultText, ToolSpec,
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

/// The three Host Tools of ADR 0091. A Model Registration before the mapping `v6` keeps them.
pub const FIRST_HOST_TOOL_REGISTRATION: HostToolRegistration = HostToolRegistration {
    id: "host_tools",
    revision: "018f0000-0000-7001-8000-0000000e0101",
    kind: ToolRegistrationKind::Host,
    adapter_revision: "storyos.host-tool.v1",
    tools: &[
        &PROPOSE_EDIT_TOOL_SPEC,
        &SUBMIT_RESEARCH_TOOL_SPEC,
        &FINISH_TOOL_SPEC,
    ],
};

/// The three Host Tools and `read_tool_result` (ADR 0093). The mapping `v6` keeps them.
pub const READ_RESULT_HOST_TOOL_REGISTRATION: HostToolRegistration = HostToolRegistration {
    revision: "018f0000-0000-7001-8000-0000000e0102",
    tools: &[
        &PROPOSE_EDIT_TOOL_SPEC,
        &SUBMIT_RESEARCH_TOOL_SPEC,
        &READ_TOOL_RESULT_TOOL_SPEC,
        &FINISH_TOOL_SPEC,
    ],
    ..FIRST_HOST_TOOL_REGISTRATION
};

/// The four Host Tools of the mapping `v6`, `load_skill`, and `read_skill_resource` (ADR 0089
/// section 3). The mapping `v7` keeps them.
pub const SKILL_HOST_TOOL_REGISTRATION: HostToolRegistration = HostToolRegistration {
    revision: "018f0000-0000-7001-8000-0000000e0103",
    tools: &[
        &PROPOSE_EDIT_TOOL_SPEC,
        &SUBMIT_RESEARCH_TOOL_SPEC,
        &READ_TOOL_RESULT_TOOL_SPEC,
        &LOAD_SKILL_TOOL_SPEC,
        &READ_SKILL_RESOURCE_TOOL_SPEC,
        &FINISH_TOOL_SPEC,
    ],
    ..FIRST_HOST_TOOL_REGISTRATION
};

/// The current Host Tools: the six Host Tools of the mapping `v7`, to which the Stage 7 tickets
/// add their Tools (ADR 0094, ADR 0095). `ExposureFacts::has_host_tool` decides which of them a
/// RunStep declares.
pub const HOST_TOOL_REGISTRATION: HostToolRegistration = HostToolRegistration {
    revision: "018f0000-0000-7001-8000-0000000e0104",
    ..SKILL_HOST_TOOL_REGISTRATION
};

impl ModelRegistration {
    /// The Host Tools that the request mapping declares. The Agent Plan mapping `v8` and the fake
    /// tool loop mapping declare the current Host Tools, the mapping `v7` keeps six, the mapping
    /// `v6` keeps four, and each earlier mapping keeps the first three (ADR 0053).
    pub fn host_tools(&self) -> &'static HostToolRegistration {
        match self.mapping_revision {
            crate::AGENT_PLAN_CONTINUITY_MAPPING_REVISION
            | crate::HOST_FAKE_TOOL_LOOP_MAPPING_REVISION => &HOST_TOOL_REGISTRATION,
            crate::AGENT_PLAN_SKILL_MAPPING_REVISION => &SKILL_HOST_TOOL_REGISTRATION,
            crate::AGENT_PLAN_READ_RESULT_MAPPING_REVISION => &READ_RESULT_HOST_TOOL_REGISTRATION,
            _ => &FIRST_HOST_TOOL_REGISTRATION,
        }
    }
}

/// Each Host Tool Registration revision of this build.
pub const HOST_TOOL_REGISTRATIONS: [&HostToolRegistration; 4] = [
    &FIRST_HOST_TOOL_REGISTRATION,
    &READ_RESULT_HOST_TOOL_REGISTRATION,
    &SKILL_HOST_TOOL_REGISTRATION,
    &HOST_TOOL_REGISTRATION,
];

/// The Host Tool of the pinned revisions of a ToolCall. `None` is a ToolCall of another
/// Registration.
pub fn host_tool(registration_revision: &str, spec_revision: &str) -> Option<&'static ToolSpec> {
    HOST_TOOL_REGISTRATIONS
        .into_iter()
        .find(|registration| registration.revision == registration_revision)?
        .tools
        .iter()
        .copied()
        .find(|spec| spec.revision == spec_revision)
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
        // The store executes `propose_edit`, because it opens a Proposal, `submit_research`,
        // because it records a Research Artifact, `read_tool_result`, because it reads a stored
        // ToolCall, and `load_skill` and `read_skill_resource`, because they read a SkillPackage
        // Snapshot.
        ToolIntake::WebSearch { .. }
        | ToolIntake::WebFetch { .. }
        | ToolIntake::ProposeEdit(_)
        | ToolIntake::SubmitResearch { .. }
        | ToolIntake::ReadToolResult { .. }
        | ToolIntake::LoadSkill { .. }
        | ToolIntake::ReadSkillResource { .. } => None,
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
