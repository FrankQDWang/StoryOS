//! The shared test support from `passage_ranking_tests.rs`.

use crate::PostgresProjectReader;
use storyos_application::{ProjectScope, QuestionVector};
use storyos_core::{PassageTerms, ToolResultText};

/// The ranked passages of `terms` and `question` in one read-only transaction of `scope`, as the
/// result text of the native call `call_id`, or `None` when the Project is not visible. With no
/// question vector, the result is the exact-term list.
pub(crate) async fn rank_passages(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    terms: &PassageTerms,
    question: Option<QuestionVector<'_>>,
    call_id: &str,
) -> Option<ToolResultText> {
    let client = store.connect().await.unwrap();
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .unwrap();
    let facts = crate::passage_ranking::read_facts(&*client, scope, question)
        .await
        .unwrap();
    client.batch_execute("COMMIT").await.unwrap();
    facts.map(|facts| {
        storyos_core::hybrid_search_result(&facts.chapters, Some(&facts.nearest), terms, call_id)
    })
}
