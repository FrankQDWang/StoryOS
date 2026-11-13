//! The Host Tool `propose_edit` (ADR 0091 sections 2 and 5): its arguments, the prose change
//! validation of one call (ADR 0061, ADR 0062, ADR 0071), and the screen of the calls of one
//! batch against the earlier calls of the Run.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    CallValidationRefusal, DeclaredLocation, NativeStreamItem, ProseChangeCandidate,
    ProseChangeRefusal, RefusedToolCall, RequestedToolCall, SnapshotTool, ToolCallBatch,
    ToolIntake, prose_change_refusal,
};

/// The validation can refuse this many `propose_edit` calls of one Run. The Host refuses each
/// later call.
pub const MAX_REFUSED_PROPOSE_EDIT_CALLS: usize = 3;

/// The result of a call that opened a Proposal. It holds no manuscript text.
pub const PROPOSAL_WAITS_RESULT: &str = "StoryOS opened the Proposal of this change, and the Proposal waits for the decision of the \
     author.";

/// One block change of a `propose_edit` call.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BlockChange {
    pub block_id: String,
    pub candidate_text: String,
    pub explanation: String,
}

/// The arguments of one `propose_edit` call: the content of the prose change reply.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProposedEdit {
    pub summary: String,
    pub changes: Vec<BlockChange>,
}

impl ProposedEdit {
    /// The prose changes of the call against the declared targets, when the prose change
    /// validation accepts them. A block that is not a target keeps an empty base, so the
    /// validation refuses it.
    pub fn validate(
        &self,
        targets: &[DeclaredLocation],
    ) -> Result<Vec<ProseChangeCandidate>, ProposeEditRefusal> {
        let candidates: Vec<ProseChangeCandidate> = self
            .changes
            .iter()
            .map(|change| {
                let target = targets
                    .iter()
                    .find(|target| target.block_id == change.block_id);
                ProseChangeCandidate {
                    chapter_id: target
                        .map(|target| target.chapter_id.clone())
                        .unwrap_or_default(),
                    manuscript_block_id: change.block_id.clone(),
                    base_authoritative_revision_id: target
                        .map(|target| target.base_revision_id.clone())
                        .unwrap_or_default(),
                    candidate_text: change.candidate_text.clone(),
                    explanation: change.explanation.clone(),
                }
            })
            .collect();
        // PostgreSQL refuses U+0000 in `text` and `jsonb`.
        if std::iter::once(&self.summary)
            .chain(self.changes.iter().flat_map(|change| {
                [
                    &change.block_id,
                    &change.candidate_text,
                    &change.explanation,
                ]
            }))
            .any(|text| text.contains('\0'))
        {
            return Err(ProposeEditRefusal::NulCharacter);
        }
        match prose_change_refusal(targets, &candidates) {
            Some(refusal) => Err(ProposeEditRefusal::Validation(refusal)),
            None => Ok(candidates),
        }
    }

    pub fn arguments(&self) -> Value {
        json!({ "summary": self.summary, "changes": self.changes })
    }
}

/// The strict JSON Schema of the arguments.
pub fn propose_edit_parameters() -> Value {
    let text = |description: &str| json!({ "type": "string", "description": description });
    json!({
        "type": "object",
        "properties": {
            "summary": text("One sentence that tells the author what the change does."),
            "changes": {
                "type": "array",
                "description": "One item for each changed target block of the request.",
                "items": {
                    "type": "object",
                    "properties": {
                        "block_id": text("The identity of one target block of the request."),
                        "candidate_text": text("The complete new text of the block."),
                        "explanation": text("Why the change answers the author request."),
                    },
                    "required": ["block_id", "candidate_text", "explanation"],
                    "additionalProperties": false,
                },
            },
        },
        "required": ["summary", "changes"],
        "additionalProperties": false,
    })
}

/// Why the Host refused one `propose_edit` call. The model reads the reason as the call result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProposeEditRefusal {
    Validation(ProseChangeRefusal),
    NulCharacter,
    /// An earlier `propose_edit` call of the same response.
    SecondInResponse,
    /// The Run already opened a Proposal.
    AfterProposal,
    /// The validation refused `MAX_REFUSED_PROPOSE_EDIT_CALLS` calls of the Run.
    Limit,
    /// A target block changed or a Proposal holds it, so the opening opened no Proposal.
    NotOpened,
    /// The referenced candidate changed after the request read it. The next RunStep reads the
    /// current candidate when it is still pending (ADR 0085).
    StaleCandidate,
}

impl ProposeEditRefusal {
    /// The stable label for the `refusal` of the ToolCall row, and the reason for the result.
    fn parts(self) -> (&'static str, &'static str) {
        use ProseChangeRefusal as Rule;
        match self {
            Self::Validation(Rule::NoChange) => ("no_change", "it has no block change"),
            Self::Validation(Rule::Shape) => (
                "change_shape",
                "it changes a pending candidate together with blocks that cannot join its Proposal",
            ),
            Self::Validation(Rule::Targets) => (
                "targets",
                "the targets of this request cannot take a change",
            ),
            Self::Validation(Rule::RepeatedBlock) => {
                ("repeated_block", "it changes one block more than one time")
            }
            Self::Validation(Rule::UndeclaredBlock) => (
                "undeclared_block",
                "it names a block that is not a target block of this request",
            ),
            Self::Validation(Rule::EmptyCandidate) => (
                "empty_candidate",
                "a block change has an empty candidate text",
            ),
            Self::Validation(Rule::EmptyExplanation) => {
                ("empty_explanation", "a block change has no explanation")
            }
            Self::Validation(Rule::OversizedText) => (
                "oversized_text",
                "a text is longer than the limit of one context item",
            ),
            Self::NulCharacter => ("nul_character", "one of its texts has the character U+0000"),
            Self::SecondInResponse => (
                "second_in_response",
                "one response can make only one propose_edit call",
            ),
            Self::AfterProposal => (
                "after_proposal",
                "this Run already opened a Proposal that waits for the decision of the author",
            ),
            Self::Limit => ("refusal_limit", ""),
            Self::NotOpened => (
                "not_opened",
                "a target block changed or another Proposal holds it",
            ),
            Self::StaleCandidate => (
                "stale_candidate",
                "the candidate changed after this request read it",
            ),
        }
    }

    pub fn label(self) -> &'static str {
        self.parts().0
    }

    /// The result of the refused call in one sentence.
    pub fn result_text(self) -> String {
        match self {
            Self::Limit => format!(
                "The Host refused this call because this Run already has \
                 {MAX_REFUSED_PROPOSE_EDIT_CALLS} refused propose_edit calls, so call finish and \
                 tell the author in the final answer that you could not make the change."
            ),
            Self::Validation(_)
            | Self::NulCharacter
            | Self::SecondInResponse
            | Self::AfterProposal
            | Self::NotOpened
            | Self::StaleCandidate => format!(
                "The Host refused this call because {}, so no Proposal opened.",
                self.parts().1
            ),
        }
    }
}

/// The earlier `propose_edit` calls of the Run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProposeEditHistory {
    pub refused: usize,
    /// A call of the Run opened a Proposal.
    pub proposal_open: bool,
}

/// Refuses each `propose_edit` call of the batch that the limits or the prose change validation
/// against `targets` refuse. Each refused call adds to the refusals of the Run in response order,
/// and above the limit each call gets the refusal of the limit. A refused call keeps the
/// arguments text of its item in `items`. The other members stay as they are.
pub fn screen_proposed_edits(
    batch: ToolCallBatch,
    items: &[NativeStreamItem],
    targets: &[DeclaredLocation],
    history: ProposeEditHistory,
) -> ToolCallBatch {
    let ToolCallBatch::Valid(calls) = batch else {
        return batch;
    };
    let mut refused = history.refused;
    let mut earlier = false;
    let calls = calls
        .into_iter()
        .map(|call| {
            let (refusal, tool, arguments_text) = match call {
                RequestedToolCall::Refused(mut call)
                    if call.tool.as_ref().is_some_and(|tool| {
                        tool.spec_revision == crate::PROPOSE_EDIT_TOOL_SPEC.revision
                    }) =>
                {
                    earlier = true;
                    if refused >= MAX_REFUSED_PROPOSE_EDIT_CALLS {
                        call.refusal =
                            CallValidationRefusal::ProposeEdit(ProposeEditRefusal::Limit);
                    }
                    refused += 1;
                    return RequestedToolCall::Refused(call);
                }
                RequestedToolCall::Validated(validated) => {
                    let ToolIntake::ProposeEdit(edit) = &validated.intake else {
                        return RequestedToolCall::Validated(validated);
                    };
                    let refusal = if refused >= MAX_REFUSED_PROPOSE_EDIT_CALLS {
                        Some(ProposeEditRefusal::Limit)
                    } else if history.proposal_open {
                        Some(ProposeEditRefusal::AfterProposal)
                    } else if earlier {
                        Some(ProposeEditRefusal::SecondInResponse)
                    } else {
                        edit.validate(targets).err()
                    };
                    earlier = true;
                    let Some(refusal) = refusal else {
                        return RequestedToolCall::Validated(validated);
                    };
                    refused += 1;
                    let arguments_text = items
                        .iter()
                        .find(|item| item.call_id.as_deref() == Some(&validated.call_id))
                        .and_then(|item| item.arguments.clone())
                        .unwrap_or_default();
                    (refusal, validated, arguments_text)
                }
                other @ RequestedToolCall::Refused(_) => return other,
            };
            RequestedToolCall::Refused(RefusedToolCall {
                arguments_text,
                tool: Some(SnapshotTool {
                    name: tool.name.clone(),
                    registration_id: tool.registration_id,
                    registration_revision: tool.registration_revision,
                    spec_revision: tool.spec_revision,
                }),
                call_id: tool.call_id,
                name: tool.name,
                refusal: CallValidationRefusal::ProposeEdit(refusal),
            })
        })
        .collect();
    ToolCallBatch::Valid(calls)
}
