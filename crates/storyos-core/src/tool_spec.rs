//! The trusted StoryOS ToolSpecs of the research tools (ADR 0034, ADR 0084). A ToolSpec does not
//! name the service behind it; the Tool Registration binds the service.
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

/// The upper bound of the effects of one ToolCall: one external read and one Outbound Disclosure
/// of one intake field. The type has no project read, Artifact write, external write, or
/// Authoritative State mutation, and a server annotation never changes it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ToolEffectEnvelope {
    pub external_read: ExternalRead,
    pub outbound_disclosure: IntakeField,
}

/// One versioned StoryOS ToolSpec.
#[derive(Debug, Eq, PartialEq)]
pub struct ToolSpec {
    pub revision: &'static str,
    pub operation: ToolOperation,
    /// The Destination Context Intake Contract: the only fields the destination receives.
    pub intake_contract: &'static [IntakeField],
    pub envelope: ToolEffectEnvelope,
    /// The result is text content of at most this many bytes in total.
    pub result_text_max_bytes: usize,
}

pub const WEB_SEARCH_TOOL_SPEC: ToolSpec = ToolSpec {
    revision: "storyos.tool.web-search.v1",
    operation: ToolOperation::WebSearch,
    intake_contract: &[IntakeField::Query, IntakeField::ResultCount],
    envelope: ToolEffectEnvelope {
        external_read: ExternalRead::OneSearch,
        outbound_disclosure: IntakeField::Query,
    },
    result_text_max_bytes: TOOL_RESULT_TEXT_MAX_BYTES,
};

pub const WEB_FETCH_TOOL_SPEC: ToolSpec = ToolSpec {
    revision: "storyos.tool.web-fetch.v1",
    operation: ToolOperation::WebFetch,
    intake_contract: &[IntakeField::UrlList],
    envelope: ToolEffectEnvelope {
        external_read: ExternalRead::GivenUrls,
        outbound_disclosure: IntakeField::UrlList,
    },
    result_text_max_bytes: TOOL_RESULT_TEXT_MAX_BYTES,
};

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

impl ToolSpec {
    /// Validates the complete arguments of one function call against the typed intake: a query
    /// of 1 to 512 characters and an optional result count of 1 to 10, or 1 to 5 absolute `http`
    /// or `https` URLs with no user information.
    pub fn intake(&self, arguments: &Value) -> Result<ToolIntake, IntakeRefusal> {
        match self.operation {
            ToolOperation::WebSearch => {
                let SearchArguments {
                    query,
                    result_count,
                } = SearchArguments::deserialize(arguments)
                    .map_err(|_| IntakeRefusal::Malformed)?;
                if query.trim().is_empty() || query.chars().count() > QUERY_MAX_CHARS {
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
                let FetchArguments { urls } = FetchArguments::deserialize(arguments)
                    .map_err(|_| IntakeRefusal::Malformed)?;
                if urls.is_empty() || urls.len() > URLS_MAX {
                    return Err(IntakeRefusal::UrlCount);
                }
                if !urls.iter().all(|url| absolute_web_url(url)) {
                    return Err(IntakeRefusal::Url);
                }
                Ok(ToolIntake::WebFetch { urls })
            }
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
            (Self::Search { .. }, ToolIntake::WebFetch { .. })
            | (Self::Fetch { .. }, ToolIntake::WebSearch { .. }) => return None,
        }
        Some(arguments)
    }
}

fn absolute_web_url(url: &str) -> bool {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"));
    rest.is_some_and(|rest| {
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        !authority.is_empty()
            && !authority.contains('@')
            && url.chars().count() <= URL_MAX_CHARS
            && !url.chars().any(|c| c.is_whitespace() || c.is_control())
    })
}

#[cfg(test)]
#[path = "tool_spec_tests.rs"]
mod tests;
