//! The Host Tool `read_tool_result` (ADR 0093): the model reads a stored research result of its
//! Run again by the identity of its call. The read gives the bounded form of the first read, with
//! the source tag and the citable sources, and it has no external effect.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    CitableSource, IntakeRefusal, TOOL_RESULT_TEXT_MAX_BYTES, ToolCallState, ToolIntake,
    ToolOperation, ToolResultProjection, ToolResultText, ToolSpec,
};

pub const READ_TOOL_RESULT_TOOL_SPEC: ToolSpec = ToolSpec {
    revision: "storyos.tool.read-tool-result.v1",
    operation: ToolOperation::ReadToolResult,
    intake_contract: &[],
    envelope: None,
    result_text_max_bytes: TOOL_RESULT_TEXT_MAX_BYTES,
};

pub(crate) const DESCRIPTION: &str = "Reads again the full result of an earlier web_search or \
web_fetch call of this work, with its tool_sources tag. Give the identity in the source attribute \
of the tool_result tag of that result.";

/// The strict JSON Schema of the arguments.
pub(crate) fn parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "call_id": {
                "type": "string",
                "description": "The source attribute of the tool_result tag of the result."
            }
        },
        "required": ["call_id"],
        "additionalProperties": false
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    call_id: String,
}

/// The typed intake. An identity with U+0000 does not fit, because the ToolCall row keeps the
/// arguments as `jsonb`.
pub(crate) fn intake(arguments: &Value) -> Result<ToolIntake, IntakeRefusal> {
    let Arguments { call_id } =
        Arguments::deserialize(arguments).map_err(|_| IntakeRefusal::Malformed)?;
    if call_id.contains('\0') {
        return Err(IntakeRefusal::UnstorableText);
    }
    Ok(ToolIntake::ReadToolResult { call_id })
}

/// The stored ToolCall that a read names, as the store finds it in the Project.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadTarget<'a> {
    /// No ToolCall of the Project has the identity.
    Unknown,
    /// Only a ToolCall of another Run has the identity.
    OtherRun,
    /// The latest ToolCall of the Run, in item order, with the identity.
    SameRun(StoredCall<'a>),
}

/// One stored ToolCall of the Run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoredCall<'a> {
    pub call_id: &'a str,
    /// The pinned ToolSpec revision. `None` is a call that failed its validation before a pin.
    pub spec_revision: Option<&'a str>,
    pub state: ToolCallState,
    pub result_text: Option<&'a str>,
    /// The Source Snapshots and then the Source Gaps of the ToolCall.
    pub sources: &'a [CitableSource],
}

/// Why the Host refused one `read_tool_result` call. The result gives the reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadToolResultRefusal {
    UnknownCall,
    OtherRun,
    HostToolCall,
    NoStoredResult,
    RestrictedSource,
}

impl ReadToolResultRefusal {
    /// The stable label of the refusal, for the `refusal` of the ToolCall row.
    pub fn label(self) -> &'static str {
        match self {
            Self::UnknownCall => "unknown_call",
            Self::OtherRun => "call_of_other_run",
            Self::HostToolCall => "host_tool_call",
            Self::NoStoredResult => "no_stored_result",
            Self::RestrictedSource => "restricted_source",
        }
    }

    /// The result of the refused call: the reason in one sentence.
    pub fn result(self) -> ToolResultText {
        let reason = match self {
            Self::UnknownCall => "no call of this work has this identity",
            Self::OtherRun => "the call with this identity belongs to an earlier author request",
            Self::HostToolCall => {
                "the call with this identity is not a web_search or web_fetch call"
            }
            Self::NoStoredResult => "the call with this identity has no stored result",
            Self::RestrictedSource => "the source of this result is now restricted",
        };
        crate::host_tool_result(&format!(
            "The Host refused this call because {reason}, so no result is available, and this is \
             not an empty result."
        ))
    }
}

/// The result of one read: the bounded form of the first read of `target`, with its source tag
/// and its citable sources, in the item limit `item_token_limit`.
pub fn read_tool_result(
    target: ReadTarget<'_>,
    source_restricted: bool,
    item_token_limit: u64,
) -> Result<ToolResultText, ReadToolResultRefusal> {
    let call = match target {
        ReadTarget::Unknown => return Err(ReadToolResultRefusal::UnknownCall),
        ReadTarget::OtherRun => return Err(ReadToolResultRefusal::OtherRun),
        ReadTarget::SameRun(call) => call,
    };
    let research = call
        .spec_revision
        .and_then(crate::tool_spec)
        .is_some_and(|spec| spec.envelope.is_some());
    if call.spec_revision.is_some() && !research {
        return Err(ReadToolResultRefusal::HostToolCall);
    }
    let projection = match (call.state, call.result_text) {
        (ToolCallState::Settled, Some(text)) if research => {
            ToolResultProjection::of(call.state, Some(text))
        }
        _ => None,
    }
    .ok_or(ReadToolResultRefusal::NoStoredResult)?;
    if source_restricted {
        return Err(ReadToolResultRefusal::RestrictedSource);
    }
    let item = crate::tool_result_context::project_item(
        call.call_id,
        projection,
        call.sources,
        item_token_limit,
    );
    Ok(crate::host_tool_result(&item.content))
}

#[cfg(test)]
#[path = "read_tool_result_tests.rs"]
mod tests;
