//! The ToolCall batch of one terminal Model Attempt (ADR 0033, ADR 0034, ADR 0084). The Host
//! validates the complete requested business-tool batch against the Tool Exposure that the Step
//! Snapshot pinned, against the typed intake of each named ToolSpec, and against the current
//! authority: its pinned revision must be active now. The contract of each member (name,
//! revisions, intake) comes from the Snapshot, never from the current Registration. A member
//! whose name is not exposed or whose arguments do not fit is a refused call, and its result
//! gives the model the reason (ADR 0091 section 3). A member that cannot have an output, or
//! whose pinned revision cannot execute now, rejects the whole batch, and an invalid batch makes
//! the Agent Decision invalid. A valid batch is part of the Decision: only a durable Decision
//! derives one ToolCall for each member, in item order.

use serde_json::{Value, json};

use crate::{
    FUNCTION_CALL_NATIVE_TYPE, IntakeRefusal, NativeStreamItem, StreamItemRole, StreamItemState,
    ToolIntake, ToolRegistrationLifecycle,
};

/// The longest native call identifier that a ToolCall row keeps.
pub const MAX_CALL_ID_CHARS: usize = 256;

/// The longest function name that a ToolCall row keeps.
pub const MAX_FUNCTION_NAME_CHARS: usize = 128;

/// One exposed Tool, as the Step Snapshot of the Attempt pinned it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotTool {
    pub name: String,
    pub registration_id: String,
    pub registration_revision: String,
    pub spec_revision: String,
}

/// The Tool Exposure that the Step Snapshot of one Model Attempt pinned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExposureSnapshot {
    pub tools: Vec<SnapshotTool>,
}

impl ExposureSnapshot {
    pub fn empty() -> Self {
        Self { tools: Vec::new() }
    }

    /// Decodes the `tool_exposure` record of a Step Snapshot. A missing record is the empty
    /// Exposure, because an Attempt before the record existed declared no Tool. A damaged
    /// record is `None`.
    pub fn decode(record: Option<&Value>) -> Option<Self> {
        let Some(record) = record.filter(|record| !record.is_null()) else {
            return Some(Self::empty());
        };
        let text = |tool: &Value, field: &str| tool.get(field)?.as_str().map(str::to_owned);
        let tools = record
            .get("tools")?
            .as_array()?
            .iter()
            .map(|tool| {
                Some(SnapshotTool {
                    name: text(tool, "name")?,
                    registration_id: text(tool, "registration_id")?,
                    registration_revision: text(tool, "registration_revision")?,
                    spec_revision: text(tool, "spec_revision")?,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self { tools })
    }
}

/// The current authority of one Registration revision that the pinned Exposure names. The Host
/// reads it in the transaction that selects the Decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentToolAuthority {
    pub registration_id: String,
    pub registration_revision: String,
    /// The current lifecycle of the pinned revision.
    pub lifecycle: ToolRegistrationLifecycle,
}

/// One validated member of the batch: the ToolCall that a durable Decision derives.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedToolCall {
    /// The native call identifier. With the Model Attempt, it is the identity of the ToolCall.
    pub call_id: String,
    pub name: String,
    pub registration_id: String,
    pub registration_revision: String,
    pub spec_revision: String,
    pub intake: ToolIntake,
}

impl ValidatedToolCall {
    /// The validated typed arguments: only the fields of the intake, in their typed form.
    pub fn arguments(&self) -> Value {
        match &self.intake {
            ToolIntake::WebSearch {
                query,
                result_count,
            } => json!({ "query": query, "result_count": result_count }),
            ToolIntake::WebFetch { urls } => json!({ "urls": urls }),
            ToolIntake::Finish => json!({}),
            ToolIntake::ProposeEdit(edit) => edit.arguments(),
            ToolIntake::SubmitResearch { summary, claims } => {
                crate::submit_research::arguments(summary, claims)
            }
            ToolIntake::ReadToolResult { call_id } => json!({ "call_id": call_id }),
        }
    }

    /// The digest of the validated arguments.
    pub fn argument_digest(&self) -> String {
        let text = crate::project_archive::canonical_json(&self.arguments());
        format!(
            "sha256:{}",
            crate::project_archive::hex_sha256(text.as_bytes())
        )
    }
}

/// The one member that rejects the batch. Each refusal names the item or the call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolCallBatchRefusal {
    /// The response did not complete, or one of its other items did not: the function call
    /// items are evidence of a cut stream, not a request.
    ResponseIncomplete,
    /// A Tool item of a native type that the request did not ask for.
    UnexpectedItem {
        item_id: String,
    },
    /// A function call item without a complete arguments text or without a call identifier.
    Incomplete {
        item_id: String,
    },
    /// The call identifier is empty, is longer than `MAX_CALL_ID_CHARS`, or holds a control
    /// character (Unicode category Cc). The store cannot keep U+0000 in a `jsonb` value.
    InvalidCallId {
        item_id: String,
    },
    /// The function name is missing or empty, is longer than `MAX_FUNCTION_NAME_CHARS`, or holds
    /// a control character.
    InvalidName {
        item_id: String,
    },
    /// The pinned revision of the Registration is not active now.
    RegistrationUnavailable {
        call_id: String,
    },
    /// The pinned ToolSpec revision of the name is not a ToolSpec of this build.
    UnknownSpec {
        call_id: String,
    },
    DuplicateCallId {
        call_id: String,
    },
}

impl ToolCallBatchRefusal {
    /// The stable label of the refusal, for the evidence record of the Attempt.
    pub fn label(&self) -> &'static str {
        match self {
            Self::ResponseIncomplete => "response_incomplete",
            Self::UnexpectedItem { .. } => "unexpected_item",
            Self::Incomplete { .. } => "incomplete_arguments",
            Self::InvalidCallId { .. } => "invalid_call_id",
            Self::InvalidName { .. } => "invalid_name",
            Self::RegistrationUnavailable { .. } => "tool_registration_unavailable",
            Self::UnknownSpec { .. } => "unknown_spec",
            Self::DuplicateCallId { .. } => "duplicate_call_id",
        }
    }
}

/// Why the Host refused one call of a batch. The model reads the reason as the call result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallValidationRefusal {
    /// The function name is not a Tool of the pinned Exposure.
    Unexposed,
    /// The arguments text is not a JSON object.
    Malformed,
    Intake(IntakeRefusal),
    ProposeEdit(crate::ProposeEditRefusal),
}

impl CallValidationRefusal {
    /// The stable label of the refusal, for the `refusal` of the ToolCall row.
    pub fn label(self) -> &'static str {
        match self {
            Self::Unexposed => "unexposed_name",
            Self::Malformed => "malformed_arguments",
            Self::Intake(IntakeRefusal::Malformed) => "intake_malformed",
            Self::Intake(IntakeRefusal::Query) => "intake_query",
            Self::Intake(IntakeRefusal::ResultCount) => "intake_result_count",
            Self::Intake(IntakeRefusal::UrlCount) => "intake_url_count",
            Self::Intake(IntakeRefusal::Url) => "intake_url",
            Self::Intake(IntakeRefusal::UnstorableText) => "unstorable_text",
            Self::ProposeEdit(refusal) => refusal.label(),
        }
    }

    /// The result of the refused call: the reason in one sentence.
    pub fn result_text(self) -> String {
        let reason = match self {
            Self::Unexposed => "this Run has no Tool with this function name",
            Self::Malformed => "its arguments text is not a JSON object",
            Self::Intake(IntakeRefusal::Malformed) => {
                "its arguments do not agree with the parameters of the Tool"
            }
            Self::Intake(IntakeRefusal::Query) => {
                "its query is empty, has more than 512 characters, or has a control character"
            }
            Self::Intake(IntakeRefusal::ResultCount) => "its result count is not from 1 to 10",
            Self::Intake(IntakeRefusal::UrlCount) => "it does not have from 1 to 5 URLs",
            Self::Intake(IntakeRefusal::Url) => {
                "one of its URLs is not an absolute http or https URL with a host and no user \
                 information"
            }
            Self::Intake(IntakeRefusal::UnstorableText) => {
                "a text of its arguments has the character U+0000"
            }
            Self::ProposeEdit(refusal) => return refusal.result_text(),
        };
        format!(
            "The Host refused this call because {reason}, so no result is available, and this \
             is not an empty result."
        )
    }
}

/// One member of a batch that failed its own validation. It is a refused ToolCall.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefusedToolCall {
    pub call_id: String,
    pub name: String,
    /// The pinned Tool of the name. `None` when the Exposure has no Tool with the name.
    pub tool: Option<SnapshotTool>,
    /// The arguments text of the item, as the model sent it.
    pub arguments_text: String,
    pub refusal: CallValidationRefusal,
}

impl RefusedToolCall {
    /// The arguments text as a JSON string. The store cannot keep U+0000, so U+FFFD replaces it.
    pub fn arguments(&self) -> Value {
        Value::String(self.arguments_text.replace('\0', "\u{fffd}"))
    }

    /// The digest of the exact arguments text.
    pub fn argument_digest(&self) -> String {
        format!(
            "sha256:{}",
            crate::project_archive::hex_sha256(self.arguments_text.as_bytes())
        )
    }
}

/// One member of a valid batch, in item order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RequestedToolCall {
    Validated(ValidatedToolCall),
    Refused(RefusedToolCall),
}

impl RequestedToolCall {
    pub fn call_id(&self) -> &str {
        match self {
            Self::Validated(call) => &call.call_id,
            Self::Refused(call) => &call.call_id,
        }
    }
}

/// The complete requested batch of one terminal Model Attempt, after validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolCallBatch {
    /// Each member validated or was refused alone, in item order. An empty batch requests no
    /// Tool.
    Valid(Vec<RequestedToolCall>),
    Invalid(ToolCallBatchRefusal),
}

impl ToolCallBatch {
    /// The batch of a response without a Tool item.
    pub fn none() -> Self {
        Self::Valid(Vec::new())
    }
}

/// Validates the Tool items of a response in `state` against the pinned `exposure` and the
/// `current` authority of its Registrations. Each Tool item is a member of the batch. A batch
/// with a member exists only in a complete response whose other items completed.
///
/// The batch is valid only when each member is a complete function call whose call identifier
/// and name have a kept length and no control character, whose call identifier no earlier
/// member has, and whose exposed name has a pinned revision that admits a new ToolCall now and a
/// ToolSpec of this build. A member is refused when its name is not exposed, its arguments text
/// is not a JSON object, or the object does not fit the typed intake of the pinned ToolSpec.
pub fn validate_tool_call_batch(
    state: StreamItemState,
    items: &[NativeStreamItem],
    exposure: &ExposureSnapshot,
    current: &[CurrentToolAuthority],
) -> ToolCallBatch {
    let mut calls: Vec<RequestedToolCall> = Vec::new();
    let members = items
        .iter()
        .filter(|item| item.role == StreamItemRole::Tool);
    if items.iter().any(|item| item.role == StreamItemRole::Tool)
        && (state != StreamItemState::Complete
            || items.iter().any(|item| {
                item.role != StreamItemRole::Tool && item.state != StreamItemState::Complete
            }))
    {
        return ToolCallBatch::Invalid(ToolCallBatchRefusal::ResponseIncomplete);
    }
    for item in members {
        match validate_member(item, exposure, current, &calls) {
            Ok(call) => calls.push(call),
            Err(refusal) => return ToolCallBatch::Invalid(refusal),
        }
    }
    ToolCallBatch::Valid(calls)
}

fn validate_member(
    item: &NativeStreamItem,
    exposure: &ExposureSnapshot,
    current: &[CurrentToolAuthority],
    earlier: &[RequestedToolCall],
) -> Result<RequestedToolCall, ToolCallBatchRefusal> {
    if item.native_type.as_deref() != Some(FUNCTION_CALL_NATIVE_TYPE) {
        return Err(ToolCallBatchRefusal::UnexpectedItem {
            item_id: item.item_id.clone(),
        });
    }
    let (Some(call_id), Some(arguments), StreamItemState::Complete) =
        (&item.call_id, &item.arguments, item.state)
    else {
        return Err(ToolCallBatchRefusal::Incomplete {
            item_id: item.item_id.clone(),
        });
    };
    if call_id.is_empty()
        || call_id.chars().count() > MAX_CALL_ID_CHARS
        || call_id.chars().any(char::is_control)
    {
        return Err(ToolCallBatchRefusal::InvalidCallId {
            item_id: item.item_id.clone(),
        });
    }
    let call_id = call_id.clone();
    if earlier.iter().any(|call| call.call_id() == call_id) {
        return Err(ToolCallBatchRefusal::DuplicateCallId { call_id });
    }
    let name = item.name.clone().unwrap_or_default();
    if name.is_empty()
        || name.chars().count() > MAX_FUNCTION_NAME_CHARS
        || name.chars().any(char::is_control)
    {
        return Err(ToolCallBatchRefusal::InvalidName {
            item_id: item.item_id.clone(),
        });
    }
    let refused = |tool: Option<&SnapshotTool>, refusal| {
        Ok(RequestedToolCall::Refused(RefusedToolCall {
            call_id: call_id.clone(),
            name: name.clone(),
            tool: tool.cloned(),
            arguments_text: arguments.clone(),
            refusal,
        }))
    };
    let Some(exposed) = exposure.tools.iter().find(|tool| tool.name == name) else {
        return refused(None, CallValidationRefusal::Unexposed);
    };
    let admitted = current.iter().any(|authority| {
        authority.registration_id == exposed.registration_id
            && authority.registration_revision == exposed.registration_revision
            && authority.lifecycle.admit_new_tool_call().is_ok()
    });
    if !admitted {
        return Err(ToolCallBatchRefusal::RegistrationUnavailable {
            call_id: call_id.clone(),
        });
    }
    let Some(spec) = crate::tool_spec(&exposed.spec_revision)
        .filter(|spec| spec.operation.function_name() == exposed.name)
    else {
        return Err(ToolCallBatchRefusal::UnknownSpec {
            call_id: call_id.clone(),
        });
    };
    let value = match serde_json::from_str::<Value>(arguments) {
        Ok(value) if value.is_object() => value,
        Ok(_) | Err(_) => return refused(Some(exposed), CallValidationRefusal::Malformed),
    };
    let intake = match spec.intake(&value) {
        Ok(intake) => intake,
        Err(refusal) => return refused(Some(exposed), CallValidationRefusal::Intake(refusal)),
    };
    Ok(RequestedToolCall::Validated(ValidatedToolCall {
        call_id,
        name,
        registration_id: exposed.registration_id.clone(),
        registration_revision: exposed.registration_revision.clone(),
        spec_revision: exposed.spec_revision.clone(),
        intake,
    }))
}

#[cfg(test)]
#[path = "tool_call_tests.rs"]
mod tests;
