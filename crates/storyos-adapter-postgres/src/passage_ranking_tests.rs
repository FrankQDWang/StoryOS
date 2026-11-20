//! The ranked passages of the manuscript (ADR 0095 sections 1 and 2) with real PostgreSQL and the
//! vectors of the fake embedding destination. Test prefix leads `95` and `69`.

use std::time::Duration;

use storyos_application::{ProjectId, ProjectScope, QuestionVector, UserId};
use storyos_core::{
    AuthorEditPrimitive, HOST_FAKE_EMBEDDING_REGISTRATION, PASSAGE_CUT_REVISION, PassageBlock,
    PassageTerms, ToolResultText, TransitionOutcome, cut_passages,
};
use tokio_postgres::Client;

use super::NEAREST_PASSAGES_SQL;
use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::stores;
use crate::command_sequence::tests::titled_chapter;
use crate::delete_structure_tests::settle_delete_chapter;
use crate::passage_vectors::tests::{
    BOATS, Chapter, SHORE, TIDE, WIND, chapter, edit_at, head, index, prepend, texts,
    vector_table_visible,
};

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
    let facts = super::read_facts(&*client, scope, question).await.unwrap();
    client.batch_execute("COMMIT").await.unwrap();
    facts.map(|facts| {
        storyos_core::hybrid_search_result(&facts.chapters, Some(&facts.nearest), terms, call_id)
    })
}

/// The result text of the ranking of `terms` and `question` in `scope`, or `None` when the Project
/// is not visible.
async fn rank(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    terms: &[&str],
    question: Option<&Vec<f32>>,
) -> Option<String> {
    let terms: Vec<String> = terms.iter().map(|term| (*term).to_owned()).collect();
    let question = question.map(|values| QuestionVector {
        registration_revision: HOST_FAKE_EMBEDDING_REGISTRATION.revision,
        values,
    });
    rank_passages(
        store,
        scope,
        &PassageTerms::new(&terms).unwrap(),
        question,
        "call-1",
    )
    .await
    .map(|result| result.text)
}

/// The lines with the numbers of the Blocks of each passage of `text`, in rank order.
fn places(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| line.starts_with("[block"))
        .collect()
}

/// The digest of each passage of `texts`, in passage order.
fn passage_digests(texts: &[String]) -> Vec<String> {
    let blocks: Vec<PassageBlock<'_>> = texts
        .iter()
        .map(|text| PassageBlock {
            block_id: "b",
            text,
        })
        .collect();
    cut_passages(&blocks)
        .into_iter()
        .map(|passage| passage.digest)
        .collect()
}

/// The stored vector of the passage `digest` of the Project of `scope`, as the vector of a
/// question.
async fn stored_vector(admin: &Client, scope: &ProjectScope, digest: &str) -> Vec<f32> {
    let text: String = admin
        .query_one(
            "SELECT embedding::text FROM storyos.passage_vectors
              WHERE project_id = $1::text::uuid AND passage_digest = $2",
            &[&scope.project_id.as_ref(), &digest],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    text.trim_matches(['[', ']'])
        .split(',')
        .map(|value| value.parse().unwrap())
        .collect()
}

async fn title(admin: &Client, chapter_id: &str) -> String {
    admin
        .query_one(
            "SELECT title FROM storyos.manuscript_objects WHERE manuscript_object_id = $1::text::uuid",
            &[&chapter_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// Applies `command` and moves the head of `chapter`.
async fn apply(
    store: &PostgresProjectReader,
    admin: &Client,
    chapter: &mut Chapter,
    command: storyos_application::ApplyAuthorEditCommand,
) {
    store.apply_author_edit(&command).await.unwrap();
    let before = std::mem::replace(&mut chapter.head, head(admin, &chapter.chapter_id).await);
    assert_ne!(chapter.head, before, "the Author Edit must apply");
}

/// A passage with no vector returns when it holds a term. After the index work, the result joins
/// the exact-term list and the vector list: the passage in the two lists goes first, and each
/// result has the line of its Chapter, the numbers of its Blocks, and its current text. With no
/// question vector, the result is the exact-term list.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_result_joins_the_exact_term_list_and_the_vector_list_of_the_current_passages() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let chapter = chapter(&store, &admin, ["951", "952"], "953", &texts()).await;
    let no_vector = vec![0.25; 1_024];
    let before = rank(&store, &chapter.scope, &["渔船"], Some(&no_vector)).await;

    index(&store, &admin, &chapter).await;
    let question = stored_vector(&admin, &chapter.scope, &passage_digests(&texts())[0]).await;
    let joined = rank(&store, &chapter.scope, &["渔船"], Some(&question)).await;
    let exact_only = rank(&store, &chapter.scope, &["渔船"], None).await;
    let nothing = rank(&store, &chapter.scope, &[], None).await;

    let line = format!(
        "{{\"chapter\":1,\"chapter_id\":\"{}\",\"title\":\"{}\"}}",
        chapter.chapter_id,
        title(&admin, &chapter.chapter_id).await
    );
    let boats = BOATS.repeat(60);
    assert_eq!(
        before.as_deref(),
        Some(
            format!(
                "{{\"passages\":1,\"passages_returned\":1,\"terms\":[\"渔船\"]}}\n{line}\n\
                 [block 5]\n{boats}"
            )
            .as_str()
        )
    );
    assert_eq!(
        joined,
        Some(format!(
            "{{\"passages\":3,\"passages_returned\":3,\"terms\":[\"渔船\"]}}\n{line}\n\
             [block 5]\n{boats}\n{line}\n[blocks 1-2]\n{}\n{}\n{line}\n[block 4]\n{}",
            TIDE.repeat(30),
            SHORE.repeat(30),
            WIND.repeat(20)
        ))
    );
    assert_eq!(exact_only, before);
    assert_eq!(
        nothing.as_deref().map(|text| text.lines().next()),
        Some(Some(
            "{\"passages\":0,\"passages_returned\":0,\"terms\":[]}"
        ))
    );
}

/// A vector row is stale when the text of its passage changes, when a Block of its passage is
/// removed, and when its Chapter is removed after the vectors were made. A stale row does not
/// return from the vector list.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_stale_vector_row_does_not_return_from_the_vector_list() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut chapter = chapter(&store, &admin, ["954", "955"], "956", &texts()).await;
    index(&store, &admin, &chapter).await;
    let digests = passage_digests(&texts());
    let first = stored_vector(&admin, &chapter.scope, &digests[0]).await;
    let second = stored_vector(&admin, &chapter.scope, &digests[1]).await;
    let all = rank(&store, &chapter.scope, &[], Some(&first))
        .await
        .unwrap();

    let command = prepend(&store, &chapter, ("9562", 2), 0, "夜里，").await;
    apply(&store, &admin, &mut chapter, command).await;
    let changed = rank(&store, &chapter.scope, &[], Some(&first))
        .await
        .unwrap();
    let join = AuthorEditPrimitive::JoinBlocks {
        left_manuscript_block_id: chapter.blocks[3].clone(),
        right_manuscript_block_id: chapter.blocks[4].clone(),
    };
    let end = WIND.repeat(20).encode_utf16().count() as u32;
    let command = edit_at(&store, &chapter, ("9563", 3), vec![join], end).await;
    apply(&store, &admin, &mut chapter, command).await;
    let joined = rank(&store, &chapter.scope, &[], Some(&second))
        .await
        .unwrap();
    let volume: String = admin
        .query_one(
            "SELECT parent_volume_id::text FROM storyos.manuscript_objects
              WHERE manuscript_object_id = $1::text::uuid",
            &[&chapter.chapter_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    titled_chapter(&store, &chapter.scope, "9564", &volume, "Chapter B", 3).await;
    let deleted = settle_delete_chapter(
        &store,
        &chapter.scope,
        "9565",
        &chapter.chapter_id,
        /*expected_tree_revision*/ 4,
    )
    .await;
    assert!(matches!(deleted.outcome, TransitionOutcome::Applied(_)));
    let removed = rank(&store, &chapter.scope, &["灯塔"], Some(&first))
        .await
        .unwrap();

    let mut every = places(&all);
    every.sort();
    assert_eq!(
        (places(&all)[0], every),
        (
            "[blocks 1-2]",
            vec!["[block 4]", "[block 5]", "[blocks 1-2]"]
        )
    );
    let mut kept = places(&changed);
    kept.sort();
    assert_eq!(kept, ["[block 4]", "[block 5]"]);
    assert_eq!(
        [&joined, &removed].map(|text| text.lines().next().unwrap().to_owned()),
        [
            "{\"passages\":0,\"passages_returned\":0,\"terms\":[]}".to_owned(),
            "{\"passages\":0,\"passages_returned\":0,\"terms\":[\"灯塔\"]}".to_owned(),
        ]
    );
}

/// The lists, the stale check, and the result text come from one transaction. The read holds on
/// a lock of the vector table after it read the Chapters. An edit that commits while it holds is
/// not in its result, and the next read has the edit.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_result_is_one_consistent_state_of_the_manuscript() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut chapter = chapter(&store, &admin, ["95a", "95b"], "95c", &texts()).await;
    index(&store, &admin, &chapter).await;
    let last = stored_vector(&admin, &chapter.scope, &passage_digests(&texts())[2]).await;
    let command = prepend(&store, &chapter, ("95c2", 2), 4, "夜里，").await;

    admin
        .batch_execute("BEGIN; LOCK TABLE storyos.passage_vectors IN ACCESS EXCLUSIVE MODE")
        .await
        .unwrap();
    let (held, ()) = tokio::join!(rank(&store, &chapter.scope, &[], Some(&last)), async {
        tokio::time::timeout(Duration::from_secs(/*secs*/ 10), async {
            loop {
                let waiting: bool = admin
                    .query_one(
                        "SELECT EXISTS (SELECT 1 FROM pg_catalog.pg_locks
                                         WHERE relation = 'storyos.passage_vectors'::regclass
                                           AND NOT granted)",
                        &[],
                    )
                    .await
                    .unwrap()
                    .get(/*idx*/ 0);
                if waiting {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(/*millis*/ 10)).await;
            }
        })
        .await
        .expect("the read must wait on the lock of the vector table");
        tokio::time::timeout(
            Duration::from_secs(/*secs*/ 10),
            store.apply_author_edit(&command),
        )
        .await
        .expect("the edit must not wait on the lock of the vector table")
        .unwrap();
        admin.batch_execute("COMMIT").await.unwrap();
    });
    chapter.head = head(&admin, &chapter.chapter_id).await;
    let after = rank(&store, &chapter.scope, &[], Some(&last))
        .await
        .unwrap();
    let edited = rank(&store, &chapter.scope, &["夜里"], None).await.unwrap();

    let held = held.unwrap();
    let boats = BOATS.repeat(60);
    assert_eq!(
        (
            held.contains(&format!("[block 5]\n{boats}")),
            held.contains("夜里"),
            places(&after).contains(&"[block 5]"),
            edited.ends_with(&format!("[block 5]\n夜里，{boats}")),
        ),
        (true, false, false, true)
    );
}

/// A read in the Scope of one Project returns no passage and no vector row of another Project,
/// also when the other Project has a nearer vector and the same term. A Project that is not
/// visible gives no result, and a read with no Scope returns no row.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_read_in_one_project_scope_returns_nothing_of_another_project() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let own = chapter(&store, &admin, ["95d", "95e"], "95f", &texts()).await;
    let mut other_texts = texts();
    other_texts[0] = format!("另一个项目。{}", other_texts[0]);
    let other = chapter(&store, &admin, ["957", "958"], "959", &other_texts).await;
    index(&store, &admin, &own).await;
    index(&store, &admin, &other).await;
    let other_digests = passage_digests(&other_texts);
    let question = stored_vector(&admin, &other.scope, &other_digests[0]).await;

    let result = rank(&store, &own.scope, &["灯塔", "另一个项目"], Some(&question))
        .await
        .unwrap();
    let hidden = ProjectScope::new(
        UserId::new(own.scope.owner_user_id.as_ref()),
        ProjectId::new("018f0000-0000-7001-8000-0000000095f9"),
    );
    let hidden = rank(&store, &hidden, &["灯塔"], Some(&question)).await;
    let client = store.connect().await.unwrap();
    client.batch_execute("BEGIN READ ONLY").await.unwrap();
    let other_ids = vec![other.chapter_id.as_str(); other_digests.len()];
    let digests: Vec<&str> = other_digests.iter().map(String::as_str).collect();
    let unscoped = client
        .query(
            NEAREST_PASSAGES_SQL,
            &[
                &other.scope.owner_user_id.as_ref(),
                &other.scope.project_id.as_ref(),
                &other_ids,
                &digests,
                &HOST_FAKE_EMBEDDING_REGISTRATION.revision,
                &PASSAGE_CUT_REVISION,
                &crate::passage_vectors::vector_text(&question),
                &10_i64,
            ],
        )
        .await
        .unwrap()
        .len();
    client.batch_execute("ROLLBACK").await.unwrap();

    assert_eq!(
        (
            places(&result).len(),
            result.contains(&own.chapter_id),
            result.contains(&other.chapter_id),
            result.contains("另一个项目。"),
            hidden,
            unscoped,
        ),
        (3, true, false, false, None, 0)
    );
}

/// With no vector store, the read sends no vector query, and the result with a question vector is
/// the exact-term list. `scripts/verify-project-scope.sh` also runs this test on a database
/// without pgvector. Test prefix lead `69`.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_result_with_no_vector_store_is_the_exact_term_list() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let chapter = chapter(&store, &admin, ["691", "692"], "693", &texts()).await;
    let offered: bool = admin
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM pg_catalog.pg_available_extensions
                             WHERE name = 'vector')",
            &[],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let question = vec![0.25; 1_024];

    let with_question = rank(&store, &chapter.scope, &["渔船"], Some(&question)).await;
    let without = rank(&store, &chapter.scope, &["渔船"], None).await;

    assert_eq!(
        (
            vector_table_visible(&store, &chapter.scope).await,
            places(with_question.as_deref().unwrap()),
            &with_question,
        ),
        (offered, vec!["[block 5]"], &without)
    );
}
