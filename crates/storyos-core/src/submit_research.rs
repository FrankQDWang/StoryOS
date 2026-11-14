//! The Host Tool `submit_research` (ADR 0091 section 2): the model gives the summary and the cited
//! claims of one research synthesis as arguments. The Host runs the validation of ADR 0087
//! section 3 against the sources of the Run and records one Research Artifact. One response and
//! one Run accept one call that records an Artifact.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    ClaimCandidate, IntakeRefusal, SynthesisRefusal, ToolIntake, ToolOperation, ToolResultText,
    ToolSpec,
};

pub const SUBMIT_RESEARCH_TOOL_SPEC: ToolSpec = ToolSpec {
    revision: "storyos.tool.submit-research.v1",
    operation: ToolOperation::SubmitResearch,
    intake_contract: &[],
    envelope: None,
    result_text_max_bytes: 1024,
};

/// The description of the Tool. It holds the cite rules of ADR 0087.
pub(crate) const DESCRIPTION: &str = "Records the result of web research for the author: a \
summary and the claims, each with its citations. A tool_sources tag after a tool_result lists the \
id of each source and each gap of that result. A gap is a requested page that is not available. \
Give at least one claim. Each claim cites at least one id from a tool_sources tag: a source that \
the claim uses, or a gap that the claim reports. Copy each id exactly. When the results do not \
answer the request, say so in the summary, and cite the gaps. A Run records one research result.";

/// The strict JSON Schema of the arguments.
pub(crate) fn parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "summary": {
                "type": "string",
                "description": "What the research found, and what it did not find."
            },
            "claims": {
                "type": "array",
                "description": "The claims, in order.",
                "items": {
                    "type": "object",
                    "properties": {
                        "text": {"type": "string", "description": "One claim."},
                        "citations": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "The id of each source or gap that the claim cites."
                        }
                    },
                    "required": ["text", "citations"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["summary", "claims"],
        "additionalProperties": false
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    summary: String,
    claims: Vec<ClaimArguments>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimArguments {
    text: String,
    citations: Vec<String>,
}

/// The typed intake. A summary, a claim text, or a citation with U+0000 does not fit, because
/// the ToolCall row keeps the arguments as `jsonb`.
pub(crate) fn intake(arguments: &Value) -> Result<ToolIntake, IntakeRefusal> {
    let Arguments { summary, claims } =
        Arguments::deserialize(arguments).map_err(|_| IntakeRefusal::Malformed)?;
    let texts = std::iter::once(&summary).chain(
        claims
            .iter()
            .flat_map(|claim| std::iter::once(&claim.text).chain(&claim.citations)),
    );
    if texts.into_iter().any(|text| text.contains('\0')) {
        return Err(IntakeRefusal::UnstorableText);
    }
    Ok(ToolIntake::SubmitResearch {
        summary,
        claims: claims
            .into_iter()
            .map(|claim| ClaimCandidate {
                text: claim.text,
                citations: claim.citations,
            })
            .collect(),
    })
}

/// The validated arguments of the intake, in their typed form.
pub(crate) fn arguments(summary: &str, claims: &[ClaimCandidate]) -> Value {
    let claims: Vec<Value> = claims
        .iter()
        .map(|claim| json!({"text": claim.text, "citations": claim.citations}))
        .collect();
    json!({"summary": summary, "claims": claims})
}

/// The result of a call that recorded the Research Artifact.
pub const RESEARCH_RECORDED_RESULT: &str = "StoryOS recorded the research result, and the author \
sees it with its sources. Do not call submit_research again in this Run.";

/// Why the Host refused one `submit_research` call. The result gives the reason, and no Artifact
/// is written.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubmitResearchRefusal {
    Synthesis(SynthesisRefusal),
    /// An earlier call of the same response is a `submit_research` call.
    SecondInResponse,
    /// The Run already recorded its Research Artifact.
    ArtifactRecorded,
}

impl SubmitResearchRefusal {
    /// The stable label of the refusal, for the `refusal` of the ToolCall row.
    pub fn label(self) -> &'static str {
        match self {
            Self::Synthesis(refusal) => refusal.as_str(),
            Self::SecondInResponse => "second_submit_research_in_response",
            Self::ArtifactRecorded => "research_artifact_recorded",
        }
    }

    /// The result of the refused call: the reason in one sentence.
    pub fn result(self) -> ToolResultText {
        let reason = match self {
            Self::Synthesis(SynthesisRefusal::NoClaim) => "the research result has no claim",
            Self::Synthesis(SynthesisRefusal::EmptyText) => "the summary or a claim text is blank",
            Self::Synthesis(SynthesisRefusal::UnstorableText) => {
                "the summary or a claim text has the character U+0000"
            }
            Self::Synthesis(SynthesisRefusal::UncitedClaim) => "a claim has no citation",
            Self::Synthesis(SynthesisRefusal::UncapturedCitation) => {
                "a citation is not the id of a source or a gap in a tool_sources tag of this Run"
            }
            Self::Synthesis(SynthesisRefusal::OtherScopeCitation) => {
                "a citation names a source of another Project or another Run"
            }
            Self::SecondInResponse => {
                "an earlier call of this response is a submit_research call, and one response \
                 accepts one"
            }
            Self::ArtifactRecorded => {
                "this Run already recorded its research result, and one Run records one"
            }
        };
        crate::host_tool_result(&format!(
            "The Host refused this call because {reason}, so StoryOS recorded no research result."
        ))
    }
}

#[cfg(test)]
#[path = "submit_research_tests.rs"]
mod tests;
