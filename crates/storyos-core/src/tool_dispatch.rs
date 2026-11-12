//! The Tool Gateway decisions of one ToolCall dispatch (ADR 0034, ADR 0083, ADR 0084): the dispatch
//! plan from the pinned Registration and ToolSpec, the Tool Effect Request, its intersection with
//! the current grant and research binding at the claim, and the Tool Effect Outcome of one
//! observation. The result text is data: no instruction, permission, or authority comes from it.

use serde_json::{Map, Value, json};

use crate::{
    ContractDifference, ExternalRead, ExternalWrite, GrantScope, IntakeField, RegisteredTool,
    ToolCompatibilityDecision, ToolIntake, ToolRegistration, ToolRegistrationLifecycle, ToolSpec,
};

/// The state of one ToolCall row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolCallState {
    Pending,
    /// The dispatch claim committed, and no observation is recorded.
    Dispatched,
    /// The server answered with a terminal result.
    Settled,
    /// The exchange settled as a native error, a rejection, an invalid result, or not submitted.
    Failed,
    /// The outcome of the exchange is not known.
    Unknown,
    /// The Host refused the ToolCall before the dispatch claim.
    Refused,
    /// The effect is outside the grant; only a Tool Approval can admit it (ADR 0034).
    AwaitingApproval,
}

impl ToolCallState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Dispatched => "dispatched",
            Self::Settled => "settled",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
            Self::Refused => "refused",
            Self::AwaitingApproval => "awaiting_approval",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [
            Self::Pending,
            Self::Dispatched,
            Self::Settled,
            Self::Failed,
            Self::Unknown,
            Self::Refused,
            Self::AwaitingApproval,
        ]
        .into_iter()
        .find(|state| state.as_str() == value)
    }
}

/// The exact effects of one ToolCall, from the Effect Envelope of its pinned ToolSpec and its
/// validated arguments. The model chooses the arguments, never the effect labels.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolEffectRequest {
    pub external_read: ExternalRead,
    pub external_write: ExternalWrite,
    pub outbound_disclosure: IntakeField,
    /// The values of the disclosed field: the query, or the URLs.
    pub disclosed: Vec<String>,
}

impl ToolEffectRequest {
    /// `None` for a ToolSpec with no external effect.
    pub fn derive(spec: &ToolSpec, intake: &ToolIntake) -> Option<Self> {
        let envelope = spec.envelope?;
        let disclosed = match intake {
            ToolIntake::WebSearch { query, .. } => vec![query.clone()],
            ToolIntake::WebFetch { urls } => urls.clone(),
            ToolIntake::Finish => Vec::new(),
        };
        Some(Self {
            external_read: envelope.external_read,
            external_write: envelope.external_write,
            outbound_disclosure: envelope.outbound_disclosure,
            disclosed,
        })
    }

    /// The evidence record of the request.
    pub fn record(&self) -> Value {
        json!({
            "external_read": match self.external_read {
                ExternalRead::OneSearch => "one_search",
                ExternalRead::GivenUrls => "given_urls",
            },
            "external_write": match self.external_write {
                ExternalWrite::None => "none",
                ExternalWrite::Declared => "declared",
            },
            "outbound_disclosure": self.outbound_disclosure.source_class(),
            "disclosed": self.disclosed,
        })
    }
}

impl IntakeField {
    /// The source class of the Context contract that a disclosure of this field has (ADR 0083).
    pub fn source_class(self) -> &'static str {
        match self {
            Self::Query | Self::ResultCount => "agent_query",
            Self::UrlList => "agent_url_selection",
        }
    }
}

impl GrantScope {
    /// Whether the scope covers the effect. `research_tool_v1` covers one external read and the
    /// disclosure of the query or the URLs, and no external write.
    pub fn covers(self, effect: &ToolEffectRequest) -> bool {
        match self {
            Self::ResearchToolV1 => {
                effect.external_write == ExternalWrite::None
                    && matches!(
                        effect.outbound_disclosure,
                        IntakeField::Query | IntakeField::UrlList
                    )
            }
            Self::ProjectWritingAssistanceV1
            | Self::ProjectWritingAssistanceV2
            | Self::ProjectWritingAssistanceV3 => false,
        }
    }
}

/// The dispatch plan of one pending ToolCall: its pinned Registration, the server tool, the
/// validated intake, the Tool Effect Request, and the server arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolCallPlan {
    pub registration: &'static ToolRegistration,
    pub tool: &'static RegisteredTool,
    pub intake: ToolIntake,
    pub effect: ToolEffectRequest,
    pub server_arguments: Map<String, Value>,
}

impl ToolCallPlan {
    /// The plan from the stored pins and typed arguments. `None` is a damaged row: an unknown
    /// revision, a ToolSpec that the Registration does not map, or arguments outside the intake.
    pub fn derive(
        registration_revision: &str,
        spec_revision: &str,
        arguments: &Value,
    ) -> Option<Self> {
        let registration = crate::tool_registration(registration_revision)?;
        let tool = registration
            .tools
            .iter()
            .find(|tool| tool.spec.revision == spec_revision)?;
        // The stored form has `result_count: null` for no count; the intake takes it as absent.
        let intake = tool.spec.intake(arguments).ok()?;
        Some(Self {
            registration,
            tool,
            effect: ToolEffectRequest::derive(tool.spec, &intake)?,
            server_arguments: tool.arguments.map(&intake)?,
            intake,
        })
    }
}

/// The current authority of one ToolCall, as the claim transaction reads it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ToolDispatchFacts<'a> {
    /// Whether the ToolCall row and the claimed AgentRun have one Project Scope.
    pub same_scope: bool,
    /// Whether the Run Capability Grant names the Registration revision and the function of the
    /// ToolCall. The Run Capability Grant of a ToolCall is the Tool Exposure that the Step
    /// Snapshot of its Model Attempt pinned: the Host gave the Run only those Tools.
    pub run_grant_names_tool: bool,
    pub lifecycle: ToolRegistrationLifecycle,
    pub compatibility: Option<ToolCompatibilityDecision>,
    /// Whether the use binding of the current research Decision pins the revision of the
    /// ToolCall.
    pub binding_pins_revision: bool,
    pub credential_reference: Option<&'a str>,
    /// The recorded scope of the grant of the use binding.
    pub grant_scope: Option<GrantScope>,
    pub effect: &'a ToolEffectRequest,
}

/// A refusal before the dispatch claim. It makes no Destination Attempt and no Disclosure Event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolDispatchRefusal {
    DisclosureOutsideGrant,
    RegistrationUnavailable,
    RegistrationIncompatible,
    CredentialUnbound,
    ToolApprovalRequired,
    ToolOutsideRunGrant,
}

impl ToolDispatchRefusal {
    pub fn code(self) -> &'static str {
        match self {
            Self::DisclosureOutsideGrant => "disclosure_outside_grant",
            Self::RegistrationUnavailable => "tool_registration_unavailable",
            Self::RegistrationIncompatible => "tool_registration_incompatible",
            Self::CredentialUnbound => "credential_unbound",
            Self::ToolApprovalRequired => "tool_approval_required",
            Self::ToolOutsideRunGrant => "tool_outside_run_grant",
        }
    }

    /// The ToolCall state that the refusal records.
    pub fn state(self) -> ToolCallState {
        match self {
            Self::ToolApprovalRequired => ToolCallState::AwaitingApproval,
            Self::DisclosureOutsideGrant
            | Self::RegistrationUnavailable
            | Self::RegistrationIncompatible
            | Self::CredentialUnbound
            | Self::ToolOutsideRunGrant => ToolCallState::Refused,
        }
    }
}

/// Admits the dispatch of one ToolCall at the final local dispatch boundary: the non-expanding
/// intersection of the exact Project Scope, the Run Capability Grant, the Registration
/// lifecycle, the current research compatibility Decision, the Credential Reference of the use binding, and the
/// grant scope with the Tool Effect Request (ADR 0034). A covered effect needs no confirmation.
pub fn admit_tool_dispatch(facts: &ToolDispatchFacts<'_>) -> Result<(), ToolDispatchRefusal> {
    if !facts.same_scope {
        return Err(ToolDispatchRefusal::DisclosureOutsideGrant);
    }
    if !facts.run_grant_names_tool {
        return Err(ToolDispatchRefusal::ToolOutsideRunGrant);
    }
    if facts.lifecycle.admit_new_tool_call().is_err() {
        return Err(ToolDispatchRefusal::RegistrationUnavailable);
    }
    if facts.compatibility != Some(ToolCompatibilityDecision::Compatible)
        || !facts.binding_pins_revision
    {
        return Err(ToolDispatchRefusal::RegistrationIncompatible);
    }
    if facts.credential_reference.is_none() {
        return Err(ToolDispatchRefusal::CredentialUnbound);
    }
    let Some(scope) = facts
        .grant_scope
        .filter(|scope| *scope == GrantScope::ResearchToolV1)
    else {
        return Err(ToolDispatchRefusal::DisclosureOutsideGrant);
    };
    if !scope.covers(facts.effect) {
        return Err(ToolDispatchRefusal::ToolApprovalRequired);
    }
    Ok(())
}

/// The complete result of one Tool exchange. It has no error channel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolObservation {
    /// The operation request did not leave this process.
    NotSubmitted,
    /// The fresh Discovery of the session differs from the pinned contract. The client did not
    /// send the operation request.
    Drift(Vec<ContractDifference>),
    /// The server refused an HTTP request, with the status and no body text.
    Rejected {
        reason: String,
    },
    JsonRpcError {
        code: i64,
    },
    /// The server answered, and the answer failed the shape check.
    InvalidResult {
        reason: &'static str,
    },
    Called {
        text: Vec<String>,
        is_error: bool,
    },
    OutcomeUnknown,
    /// The server answered the operation request with no result: an HTTP 5xx status, or a
    /// response body that ended or timed out after the status arrived. The disclosure is
    /// confirmed, and the external read is not known.
    AnsweredWithoutResult {
        reason: String,
    },
}

/// The outcome of one requested effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectOutcome {
    NotAttempted,
    Confirmed,
    Unknown,
}

impl EffectOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::NotAttempted => "not_attempted",
            Self::Confirmed => "confirmed",
            Self::Unknown => "unknown",
        }
    }
}

/// The Tool Effect Outcome of one ToolCall attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ToolEffectOutcome {
    pub external_read: EffectOutcome,
    pub outbound_disclosure: EffectOutcome,
}

impl ToolEffectOutcome {
    pub fn record(&self) -> Value {
        json!({
            "external_read": self.external_read.as_str(),
            "outbound_disclosure": self.outbound_disclosure.as_str(),
        })
    }
}

/// How the observation settles the Destination Attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolAttemptSettlement {
    NotSubmitted,
    /// ConfirmedSubmitted with a terminal result.
    TerminalResult,
    /// ConfirmedSubmitted with a native error, a rejection, or an invalid result.
    NativeError,
    OutcomeUnknown,
}

impl ToolAttemptSettlement {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotSubmitted => "not_submitted",
            Self::TerminalResult => "terminal_result",
            Self::NativeError => "native_error",
            Self::OutcomeUnknown => "outcome_unknown",
        }
    }
}

/// The result text of one settled ToolCall, kept as data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolResultText {
    /// The text blocks, joined with one line feed.
    pub text: String,
    pub digest: String,
    pub bytes: usize,
}

/// What one observation records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolCallSettlement {
    pub state: ToolCallState,
    pub attempt: ToolAttemptSettlement,
    pub outcome: ToolEffectOutcome,
    pub result: Option<ToolResultText>,
    /// The refusal code of a drift, which quarantines the revision.
    pub refusal: Option<&'static str>,
}

/// Validates the observation against the result contract of the ToolSpec: text blocks, the size
/// bound, no U+0000, and `isError`. The external read is confirmed only with a terminal result; the
/// disclosure is confirmed when the server answered.
pub fn settle_tool_call(spec: &ToolSpec, observation: &ToolObservation) -> ToolCallSettlement {
    use EffectOutcome::{Confirmed, NotAttempted, Unknown};
    let settlement =
        |state, attempt, external_read, outbound_disclosure, result| ToolCallSettlement {
            state,
            attempt,
            outcome: ToolEffectOutcome {
                external_read,
                outbound_disclosure,
            },
            result,
            refusal: None,
        };
    let native_error = || {
        settlement(
            ToolCallState::Failed,
            ToolAttemptSettlement::NativeError,
            Unknown,
            Confirmed,
            None,
        )
    };
    match observation {
        ToolObservation::NotSubmitted => settlement(
            ToolCallState::Failed,
            ToolAttemptSettlement::NotSubmitted,
            NotAttempted,
            NotAttempted,
            None,
        ),
        ToolObservation::Drift(_) => ToolCallSettlement {
            refusal: Some(ToolDispatchRefusal::RegistrationUnavailable.code()),
            ..settlement(
                ToolCallState::Refused,
                ToolAttemptSettlement::NotSubmitted,
                NotAttempted,
                NotAttempted,
                None,
            )
        },
        ToolObservation::OutcomeUnknown => settlement(
            ToolCallState::Unknown,
            ToolAttemptSettlement::OutcomeUnknown,
            Unknown,
            Unknown,
            None,
        ),
        ToolObservation::AnsweredWithoutResult { .. } => settlement(
            ToolCallState::Unknown,
            ToolAttemptSettlement::OutcomeUnknown,
            Unknown,
            Confirmed,
            None,
        ),
        ToolObservation::Rejected { .. }
        | ToolObservation::JsonRpcError { .. }
        | ToolObservation::InvalidResult { .. }
        | ToolObservation::Called { is_error: true, .. } => native_error(),
        ToolObservation::Called {
            text,
            is_error: false,
        } => {
            let text = text.join("\n");
            // The store cannot keep U+0000 in a text value.
            if text.len() > spec.result_text_max_bytes || text.contains('\0') {
                return native_error();
            }
            let digest = format!(
                "sha256:{}",
                crate::project_archive::hex_sha256(text.as_bytes())
            );
            let bytes = text.len();
            settlement(
                ToolCallState::Settled,
                ToolAttemptSettlement::TerminalResult,
                Confirmed,
                Confirmed,
                Some(ToolResultText {
                    text,
                    digest,
                    bytes,
                }),
            )
        }
    }
}

/// The next step of a Run whose response asked for a function (ADR 0084).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionRequestStep {
    /// The durable Decision has an unresolved ToolCall: the Tool Gateway handles it.
    ToolGateway,
    /// The function request has no validated ToolCall: the Run refuses with one sentence.
    Refuse,
    /// Each ToolCall of the durable Decision is resolved: the Run opens the continuation RunStep
    /// with the results (S5-05).
    Continue,
    /// No function request: the Run completes.
    Complete,
}

/// Decides the next step from the facts of the response and of its ToolCalls.
pub fn function_request_step(
    requests_function: bool,
    supplies_tool_call: bool,
    unresolved_tool_call: bool,
) -> FunctionRequestStep {
    match (requests_function, supplies_tool_call, unresolved_tool_call) {
        (_, true, true) => FunctionRequestStep::ToolGateway,
        (_, true, false) => FunctionRequestStep::Continue,
        (true, false, _) => FunctionRequestStep::Refuse,
        (false, false, _) => FunctionRequestStep::Complete,
    }
}

#[cfg(test)]
#[path = "tool_dispatch_tests.rs"]
mod tests;
