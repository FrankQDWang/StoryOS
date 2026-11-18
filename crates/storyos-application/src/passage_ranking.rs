//! The ranked passages of the whole manuscript for a question (ADR 0095 sections 1 and 2). The
//! function is read-only: it writes nothing, and its result gives no authority.

use std::future::Future;

use storyos_core::{
    ChapterPassages, NearestPassage, PassageTerms, ToolResultText, admit_passage_ranking,
    exact_term_list, fuse_passage_lists, vector_list,
};

use crate::{ProjectReadError, ProjectScope};

/// The vector of the question and the Embedding Registration that made it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuestionVector<'a> {
    pub registration_revision: &'a str,
    pub values: &'a [f32],
}

/// What the store reads in one transaction: the current passages of each live Chapter in
/// manuscript order, and the nearest vector rows of the current passages in distance order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassageRankingFacts {
    pub chapters: Vec<ChapterPassages>,
    pub nearest: Vec<NearestPassage>,
}

/// The store side of a ranking of the manuscript passages for `hybrid_search_manuscript`.
pub trait PassageRankingStore: Sync {
    /// Reads the facts of one ranking in one consistent state of the Project Scope, or `None`
    /// when the Project is not visible. With no question vector, or with no vector store,
    /// `nearest` is empty. Each nearest row has the Registration of `question`, the current cut
    /// revision, and the digest of a current passage of its Chapter.
    fn read_passage_ranking_facts(
        &self,
        scope: &ProjectScope,
        question: Option<QuestionVector<'_>>,
    ) -> impl Future<Output = Result<Option<PassageRankingFacts>, ProjectReadError>> + Send;
}

/// The ranked passages of the manuscript of `scope` for `terms` and `question`, as the result
/// text of the native call `call_id`, or `None` when the Project is not visible. The exact-term
/// list and the vector list are joined with Reciprocal Rank Fusion. With no question vector, the
/// result is the exact-term list.
pub async fn rank_manuscript_passages(
    store: &impl PassageRankingStore,
    scope: &ProjectScope,
    terms: &PassageTerms,
    question: Option<QuestionVector<'_>>,
    call_id: &str,
) -> Result<Option<ToolResultText>, ProjectReadError> {
    let Some(facts) = store.read_passage_ranking_facts(scope, question).await? else {
        return Ok(None);
    };
    let exact = exact_term_list(&facts.chapters, terms);
    let vector = vector_list(&facts.chapters, &facts.nearest);
    let ranked = fuse_passage_lists([&exact, &vector]);
    Ok(Some(admit_passage_ranking(
        &facts.chapters,
        &ranked,
        terms,
        None,
        call_id,
    )))
}
