//! The Tool Exposure of one RunStep (ADR 0034, ADR 0084, ADR 0090): the disposable projection of
//! the bound Tools that one Agent Plan request declares to the model. The Worker computes it from
//! the `active` Tool Registration of the current research binding of the Project, its
//! `compatible` compatibility Decision, and the qualified function calling entry of the pinned Model Registration. The
//! projection grants nothing, changes no Registration, and names no service: the model sees the
//! operation of a ToolSpec, and the Step Snapshot pins the Registration revision behind it. A
//! Model Registration of the tool loop profile also gets the Host Tools (ADR 0091).

use serde_json::{Value, json};

use crate::{
    AgentDecisionOutcome, CapabilityEvidence, HOST_TOOL_REGISTRATION, IntakeField,
    NativeStreamItem, StreamItemRole, StreamItemState, ToolOperation, ToolRegistrationLifecycle,
    ToolSpec, model_registration, tool_registration,
};

/// The native type of the function call item of the Agent Plan route.
pub const FUNCTION_CALL_NATIVE_TYPE: &str = "function_call";

/// The compatibility Decision of one research binding.
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

/// The Tool Registration of the current research binding of the Project, as the Worker reads it
/// at the RunStep.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundTool {
    /// The Registration revision that the use binding pins.
    pub registration_revision: String,
    pub lifecycle: ToolRegistrationLifecycle,
    /// `None` when the Decision is not of a known kind.
    pub compatibility: Option<ToolCompatibilityDecision>,
}

/// The facts of one RunStep that the Exposure reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExposureFacts<'a> {
    pub bound: &'a [BoundTool],
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
            Self::Finish => "finish",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::WebSearch => {
                "Searches the web for pages that answer one query. Use it when the author request \
                 needs facts that the supplied text does not give."
            }
            Self::WebFetch => "Reads the text of up to 5 web pages that you give by URL.",
            Self::Finish => {
                "Says that your work on the author request is complete. After its result, write \
                 the final answer to the author."
            }
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

    /// The Exposure of the facts. A binding exposes its Tools only when the Registration
    /// revision is active and known, the Decision is compatible, and the pinned Model
    /// Registration has qualified function calling. Each other binding exposes nothing, and a
    /// second Tool of one function name stays out. The Host Tools follow when the Model
    /// Registration has the tool loop profile.
    pub fn compute(facts: &ExposureFacts<'_>) -> Self {
        let Some(registration) =
            model_registration(facts.model_registration_revision).filter(|registration| {
                matches!(
                    registration.capability_profile.entries.function_calling,
                    CapabilityEvidence::Qualified { .. }
                )
            })
        else {
            return Self::empty();
        };
        let mut tools: Vec<ExposedTool> = Vec::new();
        for bound in facts.bound {
            if bound.lifecycle != ToolRegistrationLifecycle::Active
                || bound.compatibility != Some(ToolCompatibilityDecision::Compatible)
            {
                continue;
            }
            let Some(registration) = tool_registration(&bound.registration_revision) else {
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
        if registration.declares_host_tools() {
            tools.extend(HOST_TOOL_REGISTRATION.tools.iter().map(|spec| ExposedTool {
                name: spec.operation.function_name(),
                registration_id: HOST_TOOL_REGISTRATION.id,
                registration_revision: HOST_TOOL_REGISTRATION.revision,
                spec,
            }));
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

/// Whether the native items of one response hold a complete function call. When such a response
/// makes no Agent Decision, its batch did not validate and derives no ToolCall: the Run settles
/// with the call as evidence. A validated batch is an Agent Decision that derives its ToolCalls.
pub fn requests_function(items: &[NativeStreamItem]) -> bool {
    items.iter().any(|item| {
        item.role == StreamItemRole::Tool
            && item.state == StreamItemState::Complete
            && item.native_type.as_deref() == Some(FUNCTION_CALL_NATIVE_TYPE)
    })
}

/// The refusal capability of a Run whose function request batch did not validate because no Tool
/// can execute the request now (ADR 0084): its pinned revision does not admit a new ToolCall now,
/// or this build does not have its pinned ToolSpec revision. The author sees one result sentence.
pub const RESEARCH_TOOL_UNAVAILABLE_CAPABILITY: &str = "research_tool_unavailable";

/// The refusal capability of a Run whose function request batch did not validate for each other
/// refusal (ADR 0084): StoryOS could not accept the request of the model. The Attempt keeps the
/// refusal as evidence, and the author sees one result sentence.
pub const TOOL_REQUEST_INVALID_CAPABILITY: &str = "tool_request_invalid";

/// The refusal capability of a Run whose function request batch did not validate, from the
/// `label` of its `ToolCallBatchRefusal` on the Attempt evidence.
pub fn function_request_refusal_capability(label: &str) -> &'static str {
    match label {
        "tool_registration_unavailable" | "unknown_spec" => RESEARCH_TOOL_UNAVAILABLE_CAPABILITY,
        _ => TOOL_REQUEST_INVALID_CAPABILITY,
    }
}

/// What the Host does with one terminal response after core validation (ADR 0084).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseDisposition {
    /// Core validation made an Agent Decision. The response settles as that decision.
    Decision,
    /// Core validation made an Agent Decision of a validated function request batch. The store
    /// derives one ToolCall for each member with the durable Decision, in state `pending`, and
    /// the Run continues to the Tool Gateway.
    ToolCallsPending,
    /// The response of an ordinary request made no Agent Decision and asks for a function: the
    /// batch did not validate. No ToolCall exists: the items and the usage stay as evidence, and
    /// the Run refuses with the capability of `function_request_refusal_capability`. A probe
    /// never gets this disposition.
    FunctionRequested,
    /// No Agent Decision and no function request: a probe response, or a reply that validation
    /// did not accept.
    NoDecision,
}

/// The facts of one terminal response that decide its disposition.
#[derive(Clone, Copy, Debug)]
pub struct ResponseFacts<'a> {
    /// The request was a Tool qualification probe (ADR 0049).
    pub probe: bool,
    pub outcome: &'a AgentDecisionOutcome,
    pub items: &'a [NativeStreamItem],
}

/// Decides the disposition of one terminal response. Each Create path uses it: the direct
/// Create, the original-result retrieval, and the successor of an unknown Create.
pub fn response_disposition(facts: &ResponseFacts<'_>) -> ResponseDisposition {
    match facts.outcome {
        AgentDecisionOutcome::Decision {
            kind: crate::AgentDecisionKind::ToolRequest { .. },
            ..
        } => ResponseDisposition::ToolCallsPending,
        AgentDecisionOutcome::Decision { .. } => ResponseDisposition::Decision,
        AgentDecisionOutcome::NoDecision if !facts.probe && requests_function(facts.items) => {
            ResponseDisposition::FunctionRequested
        }
        AgentDecisionOutcome::NoDecision => ResponseDisposition::NoDecision,
    }
}

#[cfg(test)]
#[path = "tool_exposure_tests.rs"]
mod tests;
