//! The Host Tool `hybrid_search_manuscript` (ADR 0095 section 1): the Agent gives a question in
//! ordinary words and the exact terms that it knows, and gets the ranked passages of S7-C3a. The
//! call has no external effect and gives the Agent no target.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    ChapterPassages, EmbeddingRefusal, IntakeRefusal, NearestPassage, PassageTerms,
    TOOL_RESULT_TEXT_MAX_BYTES, ToolIntake, ToolOperation, ToolResultText, ToolSpec,
    admit_passage_ranking, exact_term_list, fuse_passage_lists, vector_list,
};

pub const HYBRID_SEARCH_MANUSCRIPT_TOOL_SPEC: ToolSpec = ToolSpec {
    revision: "storyos.tool.hybrid-search-manuscript.v1",
    operation: ToolOperation::HybridSearchManuscript,
    intake_contract: &[],
    envelope: None,
    // The Context item of a result is at or below the item limit (ADR 0052).
    result_text_max_bytes: TOOL_RESULT_TEXT_MAX_BYTES,
};

pub(crate) const DESCRIPTION: &str = "Finds the passages of the manuscript that answer a question \
in ordinary words, also when the passages do not have the words of the question. Use \
search_manuscript first. Use this Tool when search_manuscript finds nothing, or when the \
question has no exact term. Give the exact terms that you know, for example names, in terms. \
Then read the Blocks near a passage with read_chapter.";

/// The parameter schema of the Tool: a question and a list of exact terms, which can be empty.
pub(crate) fn parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "question": {
                "type": "string",
                "description": "The question in ordinary words."
            },
            "terms": {
                "type": "array",
                "items": {"type": "string"},
                "description": "The exact words or phrases that the passages can have, with the \
                                same letter case. The list can be empty."
            }
        },
        "required": ["question", "terms"],
        "additionalProperties": false
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    question: String,
    terms: Vec<String>,
}

/// The typed intake. The question has the query rule of `web_search`. The terms have the bounds
/// of `PassageTerms`, and a repeated term is removed.
pub(crate) fn intake(arguments: &Value) -> Result<ToolIntake, IntakeRefusal> {
    let Arguments { question, terms } =
        Arguments::deserialize(arguments).map_err(|_| IntakeRefusal::Malformed)?;
    if !crate::tool_spec::query_fits(&question) {
        return Err(IntakeRefusal::Query);
    }
    let terms = PassageTerms::new(&terms).map_err(|_| IntakeRefusal::Terms)?;
    Ok(ToolIntake::HybridSearchManuscript {
        question,
        terms: terms.as_slice().to_vec(),
    })
}

/// Why the Host refuses a valid call of the Tool.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HybridSearchRefusal {
    /// At the call, the deployment has no embedding configuration or the database has no vector
    /// store, or the admission refuses the embedding request of the question (ADR 0095 section
    /// 3).
    Unavailable,
}

impl HybridSearchRefusal {
    /// The refusal of a call whose embedding request the admission refused. Each admission
    /// refusal gives `Unavailable`, and the Embedding Attempt is not written.
    pub fn of_embedding(_refusal: EmbeddingRefusal) -> Self {
        Self::Unavailable
    }

    /// The stable label of the refusal, for the `refusal` of the ToolCall row.
    pub fn label(self) -> &'static str {
        match self {
            Self::Unavailable => "hybrid_search_unavailable",
        }
    }

    /// The result of the refused call: the reason in one sentence.
    pub fn result(self) -> ToolResultText {
        crate::host_tool_result(
            "The Host refused this call because the search by meaning is not available now: use \
             search_manuscript, so no result is available, and this is not an empty result.",
        )
    }
}

/// The admission of a valid call: the Host refuses it when the deployment has no embedding
/// configuration or the database has no vector store at the call.
pub fn admit_hybrid_search(
    embedding_configured: bool,
    vector_store: bool,
) -> Result<(), HybridSearchRefusal> {
    match embedding_configured && vector_store {
        true => Ok(()),
        false => Err(HybridSearchRefusal::Unavailable),
    }
}

/// The first line of a result with no vector of the question.
pub const NO_SEARCH_BY_MEANING: &str = "The search by meaning was not available for this call, so \
this result has only the passages that have the exact terms.";

/// The result of a call with the terms `terms` for the current passages `chapters` (ADR 0095
/// section 1). With the nearest vector rows of the question, the exact-term list and the vector
/// list are joined with Reciprocal Rank Fusion. With no vector of the question, because the
/// embedding request failed, the result is the exact-term list with `NO_SEARCH_BY_MEANING` as its
/// first line.
pub fn hybrid_search_result(
    chapters: &[ChapterPassages],
    nearest: Option<&[NearestPassage]>,
    terms: &PassageTerms,
    call_id: &str,
) -> ToolResultText {
    let exact = exact_term_list(chapters, terms);
    let (vector, notice) = match nearest {
        Some(nearest) => (vector_list(chapters, nearest), None),
        None => (Vec::new(), Some(NO_SEARCH_BY_MEANING)),
    };
    let ranked = fuse_passage_lists(&[&exact, &vector]);
    admit_passage_ranking(chapters, &ranked, terms, notice, call_id)
}

#[cfg(test)]
#[path = "hybrid_search_manuscript_tests.rs"]
mod tests;
