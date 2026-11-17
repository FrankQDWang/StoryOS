//! The manuscript Tool uses of a Run, in the Run read (ADR 0095 section 1). Test prefix leads
//! `ac`, and `feb` to `fef`.

use storyos_application::{
    AgentRunReadSelection, AgentRunReadStore, ConversationSelection, ProjectScope,
};
use storyos_core::{AgentRunStatus, ManuscriptUse, NO_SEARCH_BY_MEANING};

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::stores;
use crate::agent_run_finish_rule::tests::complete;
use crate::agent_run_read_chapter::tests::project;
use crate::create_agent_run::context::chapter_list_tests::admitted;
use crate::project_embedding_binding::tests::{FAKE, embedding_project};

fn chapter_read(chapter_id: &str, title: &str) -> ManuscriptUse {
    ManuscriptUse::ChapterRead {
        chapter_id: chapter_id.to_owned(),
        title: title.to_owned(),
    }
}

fn keyword_search(query: &str) -> ManuscriptUse {
    ManuscriptUse::KeywordSearch {
        query: query.to_owned(),
    }
}

fn hybrid_search(question: &str) -> ManuscriptUse {
    ManuscriptUse::HybridSearch {
        question: question.to_owned(),
    }
}

/// The manuscript Tool uses of the Run read of `run_id` in the Project Scope `scope`, or `None`
/// when the read in that Scope has no Run.
async fn uses(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    run_id: &str,
) -> Option<Option<Vec<ManuscriptUse>>> {
    store
        .read_agent_run(scope, run_id, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .map(|run| run.manuscript_uses)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_run_read_names_each_chapter_and_each_query_once_in_the_order_of_the_first_use() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let blocks = ["Lin ran.", "The bell."];
    let (scope, chapter_a, fog, _) = project(&store, &admin, "ac1", 0xbd10, &blocks).await;
    // One search, two reads of Chapter A, one read of Fog, and one read of a number that no
    // Chapter has, in one response.
    let message = "SCRIPT:search_manuscript Lin\nSCRIPT:read_chapter 1\n\
                   SCRIPT:read_chapter 1:2-null\nSCRIPT:read_chapter 2\nSCRIPT:read_chapter 3";
    let new = ConversationSelection::New;
    let run_id = admitted(&store, &scope, &chapter_a, "ac2", message, new).await;
    complete(&store, &admin, &scope, &run_id).await;
    let run = store
        .read_agent_run(&scope, &run_id, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    let attempt_id = run.model.as_ref().unwrap().model_attempt_id.clone();
    let attempt = store
        .read_agent_run(
            &scope,
            &run_id,
            &AgentRunReadSelection::ModelAttempt(attempt_id),
        )
        .await
        .unwrap()
        .unwrap();
    let states: Vec<String> = admin
        .query(
            "SELECT state FROM storyos.tool_calls
              WHERE run_id = $1::text::uuid AND function_name <> 'finish'
              ORDER BY decision_position, call_position",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(/*idx*/ 0))
        .collect();

    // The refused read gives no entry. A read of one Model Attempt has no such field.
    assert_eq!(
        (
            run.status,
            states,
            run.manuscript_uses,
            attempt.manuscript_uses
        ),
        (
            AgentRunStatus::Completed,
            ["settled", "settled", "settled", "settled", "refused"]
                .map(str::to_owned)
                .to_vec(),
            Some(vec![
                keyword_search("Lin"),
                chapter_read(&chapter_a, "Chapter A"),
                chapter_read(&fog, "Fog"),
            ]),
            None,
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_run_read_names_only_the_uses_of_its_own_run() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut runs = Vec::new();
    for (lead, suffix, prefix, query) in
        [("ac3", 0xbd20, "ac4", "Lin"), ("ac5", 0xbd30, "ac6", "Mei")]
    {
        let (scope, chapter_a, fog, _) =
            project(&store, &admin, lead, suffix, &["Lin met Mei."]).await;
        let message = format!("SCRIPT:search_manuscript {query}\nSCRIPT:read_chapter 2");
        let new = ConversationSelection::New;
        let run_id = admitted(&store, &scope, &chapter_a, prefix, &message, new).await;
        complete(&store, &admin, &scope, &run_id).await;
        runs.push((scope, run_id, fog, query));
    }
    let mut got = Vec::new();
    for (scope, run_id, _, _) in &runs {
        got.push(uses(&store, scope, run_id).await);
    }

    // Each Run read has only the entries of its own Run, and the read of a Run in the Scope of
    // the other Project has no Run.
    assert_eq!(
        got,
        runs.iter()
            .map(|(_, _, fog, query)| Some(Some(vec![
                keyword_search(query),
                chapter_read(fog, "Fog")
            ])))
            .collect::<Vec<_>>()
    );
    assert_eq!(uses(&store, &runs[1].0, &runs[0].1).await, None);
}

/// ADR 0095 section 1: each question of a settled `hybrid_search_manuscript` call is one entry.
/// A call that settled with the exact-term list only, because no vector was available, gives the
/// entry. A refused call gives no entry. A Run of another Project has only its own entries: its
/// call is refused because the Project has no embedding use binding.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_run_read_names_each_question_of_a_hybrid_search_only_in_its_own_run() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (plain, admin) = stores().await;
    let store = plain.clone().with_embedding_registration(FAKE);
    let (bound, chapter_id, first) = embedding_project(&store, &admin, ["feb", "fec"], FAKE).await;
    complete(&store, &admin, &bound, &first).await;
    // A hybrid search, a keyword search, the same question again with no term, an empty
    // question, and a second question.
    let message = "SCRIPT:hybrid_search_manuscript 谁在渡口等船？|Lin\nSCRIPT:search_manuscript Lin\n\
                   SCRIPT:hybrid_search_manuscript 谁在渡口等船？\n\
                   SCRIPT:hybrid_search_manuscript |Lin\nSCRIPT:hybrid_search_manuscript 钟声从哪里来？";
    let new = ConversationSelection::New;
    let used = admitted(&store, &bound, &chapter_id, "fed", message, new).await;
    complete(&store, &admin, &bound, &used).await;
    let (other, other_chapter, _, _) = project(&plain, &admin, "fee", 0xfef8, &["Lin ran."]).await;
    let message =
        "SCRIPT:hybrid_search_manuscript 谁在渡口等船？\nSCRIPT:search_manuscript 谁在渡口等船？";
    let new = ConversationSelection::New;
    let unbound = admitted(&plain, &other, &other_chapter, "fef", message, new).await;
    complete(&plain, &admin, &other, &unbound).await;
    // The Run of each hybrid search call (true for the Run of the bound Project), its state, and
    // whether its result is the exact-term list only.
    let states: Vec<(bool, String, bool)> = admin
        .query(
            "SELECT run_id = $2::text::uuid, state,
                    COALESCE(starts_with(result_text, $1), false)
               FROM storyos.tool_calls
              WHERE run_id IN ($2::text::uuid, $3::text::uuid)
                AND function_name = 'hybrid_search_manuscript'
              ORDER BY run_id = $2::text::uuid DESC, decision_position, call_position",
            &[&NO_SEARCH_BY_MEANING, &used, &unbound],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| (row.get(/*idx*/ 0), row.get(/*idx*/ 1), row.get(/*idx*/ 2)))
        .collect();

    // Each settled call has the exact-term list only, and the Run of the other Project gets no
    // entry for its refused call.
    assert_eq!(
        (
            states,
            uses(&store, &bound, &used).await,
            uses(&store, &bound, &first).await,
            uses(&plain, &other, &unbound).await,
            uses(&plain, &other, &used).await,
        ),
        (
            [
                (true, "settled", true),
                (true, "settled", true),
                (true, "refused", false),
                (true, "settled", true),
                (false, "refused", false),
            ]
            .map(|(bound, state, exact)| (bound, state.to_owned(), exact))
            .to_vec(),
            Some(Some(vec![
                hybrid_search("谁在渡口等船？"),
                keyword_search("Lin"),
                hybrid_search("钟声从哪里来？"),
            ])),
            Some(Some(Vec::new())),
            Some(Some(vec![keyword_search("谁在渡口等船？")])),
            None,
        )
    );
}
