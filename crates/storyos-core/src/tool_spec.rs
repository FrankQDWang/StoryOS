//! The trusted StoryOS ToolSpecs of the research tools (ADR 0034, ADR 0084) and of the Host Tools
//! (ADR 0091). A ToolSpec does not name the service behind it; the Tool Registration binds the
//! service.
//!
//! The callable contract is a typed intake, not a JSON Schema validator. A search takes one query
//! and an optional result count. When the server of a Registration also requires a search goal,
//! the Registration derives it from the query with `SEARCH_GOAL_PREFIX`, so the model gives one
//! free-text field and the destination receives no other content.

use serde::Deserialize;
use serde_json::{Map, Value};

/// The bound of the total text of one research tool result.
pub const TOOL_RESULT_TEXT_MAX_BYTES: usize = 256 * 1024;
const QUERY_MAX_CHARS: usize = 512;
const RESULT_COUNT_MAX: u8 = 10;
const URLS_MAX: usize = 5;
const URL_MAX_CHARS: usize = 2048;
/// The fixed text before the query in the derived search goal.
const SEARCH_GOAL_PREFIX: &str = "Rank first the pages that answer this search: ";

/// The operation of a ToolSpec.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolOperation {
    WebSearch,
    WebFetch,
    /// The Host Tool with which the Agent says that the work of the Run is complete.
    Finish,
    /// The Host Tool with which the Agent proposes a change of Chapter blocks.
    ProposeEdit,
    /// The Host Tool with which the Agent gives one research synthesis (ADR 0091).
    SubmitResearch,
    /// The Host Tool with which the Agent reads a stored research result again (ADR 0093).
    ReadToolResult,
}

/// One field that a destination may receive. A destination receives no Ambient Context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntakeField {
    Query,
    ResultCount,
    UrlList,
}

/// The external read of one ToolCall.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalRead {
    OneSearch,
    GivenUrls,
}

/// The external write that a ToolSpec declares. No research ToolSpec declares one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalWrite {
    None,
    Declared,
}

/// The upper bound of the effects of one ToolCall: one external read, the declared external
/// write, and one Outbound Disclosure of one intake field. The type has no project read,
/// Artifact write, or Authoritative State mutation, and a server annotation never changes it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ToolEffectEnvelope {
    pub external_read: ExternalRead,
    pub external_write: ExternalWrite,
    pub outbound_disclosure: IntakeField,
}

/// One versioned StoryOS ToolSpec.
#[derive(Debug, Eq, PartialEq)]
pub struct ToolSpec {
    pub revision: &'static str,
    pub operation: ToolOperation,
    /// The Destination Context Intake Contract: the only fields the destination receives.
    pub intake_contract: &'static [IntakeField],
    /// `None` for a Host Tool: it has no external effect.
    pub envelope: Option<ToolEffectEnvelope>,
    /// The result is text content of at most this many bytes in total.
    pub result_text_max_bytes: usize,
}

pub const WEB_SEARCH_TOOL_SPEC: ToolSpec = ToolSpec {
    revision: "storyos.tool.web-search.v1",
    operation: ToolOperation::WebSearch,
    intake_contract: &[IntakeField::Query, IntakeField::ResultCount],
    envelope: Some(ToolEffectEnvelope {
        external_read: ExternalRead::OneSearch,
        external_write: ExternalWrite::None,
        outbound_disclosure: IntakeField::Query,
    }),
    result_text_max_bytes: TOOL_RESULT_TEXT_MAX_BYTES,
};

pub const WEB_FETCH_TOOL_SPEC: ToolSpec = ToolSpec {
    revision: "storyos.tool.web-fetch.v1",
    operation: ToolOperation::WebFetch,
    intake_contract: &[IntakeField::UrlList],
    envelope: Some(ToolEffectEnvelope {
        external_read: ExternalRead::GivenUrls,
        external_write: ExternalWrite::None,
        outbound_disclosure: IntakeField::UrlList,
    }),
    result_text_max_bytes: TOOL_RESULT_TEXT_MAX_BYTES,
};

/// The Host Tool `finish`: no arguments and an empty result (ADR 0091).
pub const FINISH_TOOL_SPEC: ToolSpec = ToolSpec {
    revision: "storyos.tool.finish.v1",
    operation: ToolOperation::Finish,
    intake_contract: &[],
    envelope: None,
    result_text_max_bytes: 0,
};

/// The Host Tool `propose_edit`: the content of one prose change reply (ADR 0091).
pub const PROPOSE_EDIT_TOOL_SPEC: ToolSpec = ToolSpec {
    revision: "storyos.tool.propose-edit.v1",
    operation: ToolOperation::ProposeEdit,
    intake_contract: &[],
    envelope: None,
    result_text_max_bytes: 512,
};

/// The ToolSpec of a pinned revision. `None` is a revision that this build does not know.
pub fn tool_spec(revision: &str) -> Option<&'static ToolSpec> {
    [
        &WEB_SEARCH_TOOL_SPEC,
        &WEB_FETCH_TOOL_SPEC,
        &FINISH_TOOL_SPEC,
        &PROPOSE_EDIT_TOOL_SPEC,
        &crate::SUBMIT_RESEARCH_TOOL_SPEC,
        &crate::READ_TOOL_RESULT_TOOL_SPEC,
    ]
    .into_iter()
    .find(|spec| spec.revision == revision)
}

/// The validated intake of one ToolCall.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolIntake {
    WebSearch {
        query: String,
        result_count: Option<u8>,
    },
    WebFetch {
        urls: Vec<String>,
    },
    Finish,
    ProposeEdit(crate::ProposedEdit),
    SubmitResearch {
        summary: String,
        claims: Vec<crate::ClaimCandidate>,
    },
    ReadToolResult {
        call_id: String,
    },
}

/// Why the arguments of a function call do not fit the typed intake.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntakeRefusal {
    /// Not an object of the declared fields, or a field of the wrong type.
    Malformed,
    Query,
    ResultCount,
    UrlCount,
    Url,
    /// A text of the arguments has U+0000, which the store cannot keep in a `jsonb` value.
    UnstorableText,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchArguments {
    query: String,
    #[serde(default)]
    result_count: Option<u8>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FetchArguments {
    urls: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoArguments {}

impl ToolSpec {
    /// Validates the complete arguments of one function call against the typed intake. A search
    /// takes a query of 1 to 512 characters and an optional result count of 1 to 10. A fetch
    /// takes 1 to 5 absolute `http` or `https` URLs with a host and no user information. A query
    /// or a URL with a control character (Unicode category Cc: U+0000 to U+001F, U+007F, and
    /// U+0080 to U+009F) does not fit: the store cannot keep U+0000 in a `jsonb` value, and a
    /// query or a URL is one line of text.
    pub fn intake(&self, arguments: &Value) -> Result<ToolIntake, IntakeRefusal> {
        match self.operation {
            ToolOperation::WebSearch => {
                let SearchArguments {
                    query,
                    result_count,
                } = SearchArguments::deserialize(arguments)
                    .map_err(|_| IntakeRefusal::Malformed)?;
                if query.trim().is_empty()
                    || query.chars().count() > QUERY_MAX_CHARS
                    || query.chars().any(char::is_control)
                {
                    return Err(IntakeRefusal::Query);
                }
                if result_count.is_some_and(|count| count == 0 || count > RESULT_COUNT_MAX) {
                    return Err(IntakeRefusal::ResultCount);
                }
                Ok(ToolIntake::WebSearch {
                    query,
                    result_count,
                })
            }
            ToolOperation::WebFetch => {
                let FetchArguments { urls } =
                    FetchArguments::deserialize(arguments).map_err(|_| IntakeRefusal::Malformed)?;
                if urls.is_empty() || urls.len() > URLS_MAX {
                    return Err(IntakeRefusal::UrlCount);
                }
                if !urls.iter().all(|url| absolute_web_url(url)) {
                    return Err(IntakeRefusal::Url);
                }
                Ok(ToolIntake::WebFetch { urls })
            }
            ToolOperation::Finish => {
                NoArguments::deserialize(arguments).map_err(|_| IntakeRefusal::Malformed)?;
                Ok(ToolIntake::Finish)
            }
            ToolOperation::ProposeEdit => crate::ProposedEdit::deserialize(arguments)
                .map(ToolIntake::ProposeEdit)
                .map_err(|_| IntakeRefusal::Malformed),
            ToolOperation::SubmitResearch => crate::submit_research::intake(arguments),
            ToolOperation::ReadToolResult => crate::read_tool_result::intake(arguments),
        }
    }
}

/// The argument names that one server tool takes for the fields of a typed intake. This is a
/// trusted adapter rule of the Tool Registration.
#[derive(Debug, Eq, PartialEq)]
pub enum ServerArguments {
    Search {
        query: &'static str,
        /// The required goal field, which receives `SEARCH_GOAL_PREFIX` and the query.
        goal: Option<&'static str>,
        result_count: &'static str,
    },
    Fetch {
        urls: &'static str,
    },
}

impl ServerArguments {
    /// The server arguments of an intake. `None` when the intake is of another operation.
    pub fn map(&self, intake: &ToolIntake) -> Option<Map<String, Value>> {
        let mut arguments = Map::new();
        match (self, intake) {
            (
                Self::Search {
                    query: query_name,
                    goal,
                    result_count: count_name,
                },
                ToolIntake::WebSearch {
                    query,
                    result_count,
                },
            ) => {
                arguments.insert((*query_name).to_owned(), Value::from(query.as_str()));
                if let Some(goal) = goal {
                    arguments.insert(
                        (*goal).to_owned(),
                        Value::from(format!("{SEARCH_GOAL_PREFIX}{query}")),
                    );
                }
                if let Some(count) = result_count {
                    arguments.insert((*count_name).to_owned(), Value::from(*count));
                }
            }
            (Self::Fetch { urls: urls_name }, ToolIntake::WebFetch { urls }) => {
                arguments.insert((*urls_name).to_owned(), Value::from(urls.clone()));
            }
            (
                Self::Search { .. },
                ToolIntake::WebFetch { .. }
                | ToolIntake::Finish
                | ToolIntake::ProposeEdit(_)
                | ToolIntake::SubmitResearch { .. }
                | ToolIntake::ReadToolResult { .. },
            )
            | (
                Self::Fetch { .. },
                ToolIntake::WebSearch { .. }
                | ToolIntake::Finish
                | ToolIntake::ProposeEdit(_)
                | ToolIntake::SubmitResearch { .. }
                | ToolIntake::ReadToolResult { .. },
            ) => {
                return None;
            }
        }
        Some(arguments)
    }
}

fn absolute_web_url(text: &str) -> bool {
    text.chars().count() <= URL_MAX_CHARS
        && !text.chars().any(|c| c.is_whitespace() || c.is_control())
        && url::Url::parse(text).is_ok_and(|url| {
            matches!(url.scheme(), "https" | "http")
                && url.host_str().is_some_and(|host| !host.is_empty())
                && url.username().is_empty()
                && url.password().is_none()
        })
}

#[cfg(test)]
#[path = "tool_spec_tests.rs"]
mod tests;
