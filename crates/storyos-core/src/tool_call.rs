//! The ToolCall batch of one terminal Model Attempt (ADR 0033, ADR 0034, ADR 0084). The Host
//! validates the complete requested business-tool batch against the Tool Exposure that the Step
//! Snapshot pinned, against the typed intake of each named ToolSpec, and against the current
//! authority: the Project must enable the Registration now, and its pinned revision must be
//! active now. The contract of each member (name, revisions, intake) comes from the Snapshot,
//! never from the current Registration. One invalid member rejects the whole batch, and an
//! invalid batch makes the Agent Decision invalid. A valid batch is part of the Decision: only a durable Decision derives
//! one ToolCall for each member, in item order.

use serde_json::{Value, json};

use crate::{
    FUNCTION_CALL_NATIVE_TYPE, IntakeRefusal, NativeStreamItem, StreamItemRole, StreamItemState,
    ToolIntake, ToolRegistrationLifecycle, WEB_FETCH_TOOL_SPEC, WEB_SEARCH_TOOL_SPEC,
};

/// The longest native call identifier that a ToolCall row keeps.
pub const MAX_CALL_ID_CHARS: usize = 256;

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
    /// Whether the latest Project Tool Enablement of the Registration enables it.
    pub enabled: bool,
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
    /// The function name is not a Tool of the pinned Exposure.
    Unexposed {
        call_id: String,
    },
    /// The Project does not enable the Registration of the name now.
    NotEnabled {
        call_id: String,
    },
    /// The pinned revision of the Registration is not active now.
    RegistrationUnavailable {
        call_id: String,
    },
    /// The pinned ToolSpec revision of the name is not a ToolSpec of this build.
    UnknownSpec {
        call_id: String,
    },
    /// The arguments text is not a JSON object.
    Malformed {
        call_id: String,
    },
    Intake {
        call_id: String,
        refusal: IntakeRefusal,
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
            Self::Unexposed { .. } => "unexposed_name",
            Self::NotEnabled { .. } => "tool_not_enabled",
            Self::RegistrationUnavailable { .. } => "tool_registration_unavailable",
            Self::UnknownSpec { .. } => "unknown_spec",
            Self::Malformed { .. } => "malformed_arguments",
            Self::Intake { refusal, .. } => match refusal {
                IntakeRefusal::Malformed => "intake_malformed",
                IntakeRefusal::Query => "intake_query",
                IntakeRefusal::ResultCount => "intake_result_count",
                IntakeRefusal::UrlCount => "intake_url_count",
                IntakeRefusal::Url => "intake_url",
            },
            Self::DuplicateCallId { .. } => "duplicate_call_id",
        }
    }
}

/// The complete requested batch of one terminal Model Attempt, after validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolCallBatch {
    /// Each member validated, in item order. An empty batch requests no Tool.
    Valid(Vec<ValidatedToolCall>),
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
/// with a member exists only in a complete response whose other items completed. A member is
/// valid when it is a complete function call with a call identifier of a kept length and no
/// control character, its name is exposed, the Project enables its Registration now, its pinned
/// revision admits a new ToolCall now, its arguments text is a JSON object, the object fits the
/// typed intake of the pinned ToolSpec, and no earlier member has its call identifier.
pub fn validate_tool_call_batch(
    state: StreamItemState,
    items: &[NativeStreamItem],
    exposure: &ExposureSnapshot,
    current: &[CurrentToolAuthority],
) -> ToolCallBatch {
    let mut calls: Vec<ValidatedToolCall> = Vec::new();
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
    earlier: &[ValidatedToolCall],
) -> Result<ValidatedToolCall, ToolCallBatchRefusal> {
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
    if earlier.iter().any(|call| call.call_id == call_id) {
        return Err(ToolCallBatchRefusal::DuplicateCallId { call_id });
    }
    let name = item.name.clone().unwrap_or_default();
    let Some(exposed) = exposure.tools.iter().find(|tool| tool.name == name) else {
        return Err(ToolCallBatchRefusal::Unexposed { call_id });
    };
    // A pinned revision without a current authority record is not enabled now.
    let Some(authority) = current.iter().find(|authority| {
        authority.registration_id == exposed.registration_id
            && authority.registration_revision == exposed.registration_revision
            && authority.enabled
    }) else {
        return Err(ToolCallBatchRefusal::NotEnabled { call_id });
    };
    if authority.lifecycle.admit_new_tool_call().is_err() {
        return Err(ToolCallBatchRefusal::RegistrationUnavailable { call_id });
    }
    let Some(spec) = [&WEB_SEARCH_TOOL_SPEC, &WEB_FETCH_TOOL_SPEC]
        .into_iter()
        .find(|spec| spec.revision == exposed.spec_revision)
        .filter(|spec| spec.operation.function_name() == exposed.name)
    else {
        return Err(ToolCallBatchRefusal::UnknownSpec { call_id });
    };
    let Ok(value) = serde_json::from_str::<Value>(arguments) else {
        return Err(ToolCallBatchRefusal::Malformed { call_id });
    };
    if !value.is_object() {
        return Err(ToolCallBatchRefusal::Malformed { call_id });
    }
    let intake = spec
        .intake(&value)
        .map_err(|refusal| ToolCallBatchRefusal::Intake {
            call_id: call_id.clone(),
            refusal,
        })?;
    Ok(ValidatedToolCall {
        call_id,
        name,
        registration_id: exposed.registration_id.clone(),
        registration_revision: exposed.registration_revision.clone(),
        spec_revision: exposed.spec_revision.clone(),
        intake,
    })
}

#[cfg(test)]
#[path = "tool_call_tests.rs"]
mod tests;
