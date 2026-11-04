//! The Tool Exposure of one RunStep (ADR 0034, ADR 0084): the disposable projection of the
//! enabled Tools that one Agent Plan request declares to the model. The Worker computes it from
//! the enabled `active` Tool Registrations of the Project, their `compatible` compatibility
//! Decisions, and the qualified function calling entry of the pinned Model Registration. The
//! projection grants nothing, changes no Registration, and names no service: the model sees the
//! operation of a ToolSpec, and the Step Snapshot pins the Registration revision behind it.

use serde_json::{Value, json};

use crate::{
    CapabilityEvidence, IntakeField, NativeStreamItem, StreamItemRole, StreamItemState,
    ToolOperation, ToolRegistrationLifecycle, ToolSpec, model_registration, tool_registration,
};

/// The native type of the function call item of the Agent Plan route.
pub const FUNCTION_CALL_NATIVE_TYPE: &str = "function_call";

/// The compatibility Decision of one Project Tool Enablement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolCompatibilityDecision {
    Compatible,
}

impl ToolCompatibilityDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Compatible => "tool_registration_compatible",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [Self::Compatible]
            .into_iter()
            .find(|decision| decision.as_str() == value)
    }
}

/// One enabled Tool Registration of the Project, as the Worker reads it at the RunStep.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnabledTool {
    /// The Registration revision that the enablement pinned.
    pub registration_revision: String,
    pub lifecycle: ToolRegistrationLifecycle,
    /// `None` when the enablement has no compatibility Decision of a known kind.
    pub compatibility: Option<ToolCompatibilityDecision>,
}

/// The facts of one RunStep that the Exposure reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExposureFacts<'a> {
    pub enabled: &'a [EnabledTool],
    /// The Model Registration revision that the AgentRun pinned.
    pub model_registration_revision: &'a str,
}

/// One Tool that the request declares.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExposedTool {
    /// The function name that the model sees: the operation of the ToolSpec.
    pub name: &'static str,
    pub registration_id: &'static str,
    pub registration_revision: &'static str,
    pub spec: &'static ToolSpec,
}

/// The function declaration of one exposed Tool: provider-neutral data that the adapter
/// serializes. The parameters are the JSON Schema of the typed intake of the ToolSpec.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionDeclaration {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}

/// The Tool Exposure of one RunStep. An empty Exposure declares no Tool.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolExposure {
    pub tools: Vec<ExposedTool>,
}

impl ToolOperation {
    /// The function name that the model sees. It names no service.
    pub fn function_name(self) -> &'static str {
        match self {
            Self::WebSearch => "web_search",
            Self::WebFetch => "web_fetch",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::WebSearch => {
                "Searches the web for pages that answer one query. Use it when the author request \
                 needs facts that the supplied text does not give."
            }
            Self::WebFetch => "Reads the text of up to 5 web pages that you give by URL.",
        }
    }
}

impl IntakeField {
    fn name(self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::ResultCount => "result_count",
            Self::UrlList => "urls",
        }
    }

    /// The JSON Schema of the field. Each field is required, and an optional field takes null,
    /// because a strict function schema requires each property.
    fn schema(self) -> Value {
        match self {
            Self::Query => json!({
                "type": "string",
                "description": "The search query, 1 to 512 characters."
            }),
            Self::ResultCount => json!({
                "type": ["integer", "null"],
                "description": "The number of results, 1 to 10. Null takes the default."
            }),
            Self::UrlList => json!({
                "type": "array",
                "items": {"type": "string"},
                "description": "1 to 5 absolute http or https URLs."
            }),
        }
    }
}

impl ExposedTool {
    pub fn declaration(&self) -> FunctionDeclaration {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();
        for field in self.spec.intake_contract {
            properties.insert(field.name().to_owned(), field.schema());
            required.push(field.name());
        }
        FunctionDeclaration {
            name: self.name,
            description: self.spec.operation.description(),
            parameters: json!({
                "type": "object",
                "properties": properties,
                "required": required,
                "additionalProperties": false,
            }),
        }
    }

    /// The identity of the declaration and of the Registration behind it.
    fn digest_value(&self) -> Value {
        let declaration = self.declaration();
        json!({
            "name": declaration.name,
            "description": declaration.description,
            "parameters": declaration.parameters,
            "registration_id": self.registration_id,
            "registration_revision": self.registration_revision,
            "spec_revision": self.spec.revision,
        })
    }
}

impl ToolExposure {
    pub fn empty() -> Self {
        Self { tools: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// The Exposure of the facts. An enablement exposes its Tools only when the Registration
    /// revision is active and known, the Decision is compatible, and the pinned Model
    /// Registration has qualified function calling. Each other enablement exposes nothing, and
    /// a second Tool of one function name stays out.
    pub fn compute(facts: &ExposureFacts<'_>) -> Self {
        let qualified =
            model_registration(facts.model_registration_revision).is_some_and(|registration| {
                matches!(
                    registration.capability_profile.entries.function_calling,
                    CapabilityEvidence::Qualified { .. }
                )
            });
        if !qualified {
            return Self::empty();
        }
        let mut tools: Vec<ExposedTool> = Vec::new();
        for enabled in facts.enabled {
            if enabled.lifecycle != ToolRegistrationLifecycle::Active
                || enabled.compatibility != Some(ToolCompatibilityDecision::Compatible)
            {
                continue;
            }
            let Some(registration) = tool_registration(&enabled.registration_revision) else {
                continue;
            };
            for tool in registration.tools {
                let name = tool.spec.operation.function_name();
                if tools.iter().any(|exposed| exposed.name == name) {
                    continue;
                }
                tools.push(ExposedTool {
                    name,
                    registration_id: registration.id,
                    registration_revision: registration.revision,
                    spec: tool.spec,
                });
            }
        }
        Self { tools }
    }

    /// The function declarations, in order.
    pub fn declarations(&self) -> Vec<FunctionDeclaration> {
        self.tools.iter().map(ExposedTool::declaration).collect()
    }

    /// One digest over the complete projection. Two Attempts with different digests are two
    /// transport continuations.
    pub fn digest(&self) -> String {
        let tools: Vec<Value> = self.tools.iter().map(ExposedTool::digest_value).collect();
        let text = crate::project_archive::canonical_json(&json!({ "tools": tools }));
        format!(
            "sha256:{}",
            crate::project_archive::hex_sha256(text.as_bytes())
        )
    }

    /// The record of the Step Snapshot: each exposed name with its Registration revision and
    /// ToolSpec revision, and the digest. A later validation reads it, never the current
    /// Registration.
    pub fn snapshot(&self) -> Value {
        let tools: Vec<Value> = self
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "registration_id": tool.registration_id,
                    "registration_revision": tool.registration_revision,
                    "spec_revision": tool.spec.revision,
                })
            })
            .collect();
        json!({ "digest": self.digest(), "tools": tools })
    }
}

/// Whether the native items of one response hold a complete function call. Such a response makes
/// no Agent Decision, and no ToolCall exists yet: the Run settles with the call as evidence.
pub fn requests_function(items: &[NativeStreamItem]) -> bool {
    items.iter().any(|item| {
        item.role == StreamItemRole::Tool
            && item.state == StreamItemState::Complete
            && item.native_type.as_deref() == Some(FUNCTION_CALL_NATIVE_TYPE)
    })
}

#[cfg(test)]
#[path = "tool_exposure_tests.rs"]
mod tests;
