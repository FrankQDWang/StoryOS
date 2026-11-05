//! The ToolCall batch of one terminal Model Attempt (ADR 0033, ADR 0034, ADR 0084). The Host
//! validates the complete requested business-tool batch against the Tool Exposure that the Step
//! Snapshot pinned, never against the current Registration, and against the typed intake of each
//! named ToolSpec. One invalid member rejects the whole batch, and an invalid batch makes the
//! Agent Decision invalid. A valid batch is part of the Decision: only a durable Decision derives
//! one ToolCall for each member, in item order.

use serde_json::{Value, json};

use crate::{
    FUNCTION_CALL_NATIVE_TYPE, IntakeRefusal, NativeStreamItem, StreamItemRole, StreamItemState,
    ToolIntake, ToolSpec, WEB_FETCH_TOOL_SPEC, WEB_SEARCH_TOOL_SPEC,
};

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
    /// A Tool item of a native type that the request did not ask for.
    UnexpectedItem {
        item_id: String,
    },
    /// A function call item without a complete arguments text or without a call identifier.
    Incomplete {
        item_id: String,
    },
    /// The function name is not a Tool of the pinned Exposure.
    Unexposed {
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
            Self::UnexpectedItem { .. } => "unexpected_item",
            Self::Incomplete { .. } => "incomplete_arguments",
            Self::Unexposed { .. } => "unexposed_name",
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

/// The ToolSpec of a pinned revision. The validation reads the revision of the Snapshot.
fn tool_spec(revision: &str) -> Option<&'static ToolSpec> {
    [&WEB_SEARCH_TOOL_SPEC, &WEB_FETCH_TOOL_SPEC]
        .into_iter()
        .find(|spec| spec.revision == revision)
}

/// Validates the Tool items of `items` against the pinned `exposure`. Each Tool item is a
/// member of the batch. A member is valid when it is a complete function call with a call
/// identifier, its name is exposed, its arguments text is a JSON object, the object fits the
/// typed intake of the pinned ToolSpec, and no earlier member has its call identifier.
pub fn validate_tool_call_batch(
    items: &[NativeStreamItem],
    exposure: &ExposureSnapshot,
) -> ToolCallBatch {
    let mut calls: Vec<ValidatedToolCall> = Vec::new();
    for item in items
        .iter()
        .filter(|item| item.role == StreamItemRole::Tool)
    {
        match validate_member(item, exposure, &calls) {
            Ok(call) => calls.push(call),
            Err(refusal) => return ToolCallBatch::Invalid(refusal),
        }
    }
    ToolCallBatch::Valid(calls)
}

fn validate_member(
    item: &NativeStreamItem,
    exposure: &ExposureSnapshot,
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
    let call_id = call_id.clone();
    if earlier.iter().any(|call| call.call_id == call_id) {
        return Err(ToolCallBatchRefusal::DuplicateCallId { call_id });
    }
    let name = item.name.clone().unwrap_or_default();
    let Some(exposed) = exposure.tools.iter().find(|tool| tool.name == name) else {
        return Err(ToolCallBatchRefusal::Unexposed { call_id });
    };
    let Some(spec) = tool_spec(&exposed.spec_revision)
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
