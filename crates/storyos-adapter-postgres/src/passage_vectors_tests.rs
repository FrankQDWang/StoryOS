//! The passage vectors of a Chapter (ADR 0095 sections 2 and 4) with real PostgreSQL and the fake
//! embedding destination. Test prefix lead `28`.

use storyos_adapter_fake_destination::FakeEmbeddings;
use storyos_application::{
    ClaimedPassageIndex, PASSAGE_INDEX_LEASE, PassageIndexStep, PassageVectorStore, ProjectId,
    ProjectScope, UserId, claim_next_passage_index_work, step_passage_index_work,
};
use storyos_core::{HOST_FAKE_EMBEDDING_REGISTRATION, TransitionOutcome};
use tokio_postgres::Client;

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_structure::titled_chapter;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::delete_structure::settle_delete_chapter;
use crate::shared_tests::passage_vectors::{
    FAKE_ONLY, chapter, digests, index, mark, rows, texts, vector_table_visible,
};
use crate::shared_tests::project_embedding_binding::FAKE;

/// The number of Embedding Attempts of the Project.
async fn attempts(admin: &Client, scope: &ProjectScope) -> i64 {
    admin
        .query_one(
            "SELECT count(*) FROM storyos.embedding_attempts WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_read_in_one_project_scope_gets_only_its_rows_and_cannot_name_another_chapter() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let own = chapter(&store, &admin, ["28a", "28b"], "28e", &texts()).await;
    let mut other_texts = texts();
    other_texts[0] = format!("另一个项目。{}", other_texts[0]);
    let other = chapter(&store, &admin, ["28c", "28d"], "28f", &other_texts).await;
    index(&store, &admin, &own).await;
    index(&store, &admin, &other).await;
    let other_rows = rows(&admin, &other.scope).await;
    let other_attempts = attempts(&admin, &other.scope).await;
    // The question is a vector of the other Project, so its nearest row is in that Project.
    let question: String = admin
        .query_one(
            "SELECT embedding::text FROM storyos.passage_vectors
              WHERE project_id = $1::text::uuid LIMIT 1",
            &[&other.scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let nearest = |scope: ProjectScope| {
        let store = &store;
        let question = question.clone();
        async move {
            store
                .in_read_transaction(&scope, async |client| {
                    Ok(client
                        .query(
                            "SELECT project_id::text FROM storyos.passage_vectors
                              ORDER BY embedding OPERATOR(extensions.<=>)
                                       $1::text::extensions.vector
                              LIMIT 10",
                            &[&question],
                        )
                        .await
                        .unwrap()
                        .iter()
                        .map(|row| row.get::<_, String>(/*idx*/ 0))
                        .collect::<Vec<_>>())
                })
                .await
                .unwrap()
        }
    };

    // The read of the own Project Scope names its own Chapter, and then the Chapter of the other
    // Project.
    let read = async |chapter_id: &str| {
        let facts = store
            .read_passage_index_facts(&own.scope, chapter_id)
            .await
            .unwrap();
        (facts.chapter.is_some(), facts.stored.len())
    };
    let own_facts = read(&own.chapter_id).await;
    let crossed_facts = read(&other.chapter_id).await;
    // The other Project holds its work: a later mark and a live lease of its claim.
    let held = admin
        .query_one(
            "UPDATE storyos.passage_index_work
                SET mark_generation = mark_generation + 1,
                    lease_expires_at = clock_timestamp() + interval '1 hour'
              WHERE project_id = $1::text::uuid AND chapter_id = $2::text::uuid
          RETURNING mark_generation, fence_token",
            &[&other.scope.project_id.as_ref(), &other.chapter_id],
        )
        .await
        .unwrap();
    let other_work = async || -> (i64, i64, Option<String>, Option<String>) {
        let row = admin
            .query_one(
                "SELECT mark_generation, completed_generation, lease_expires_at::text,
                        indexed_cut_revision
                   FROM storyos.passage_index_work
                  WHERE project_id = $1::text::uuid AND chapter_id = $2::text::uuid",
                &[&other.scope.project_id.as_ref(), &other.chapter_id],
            )
            .await
            .unwrap();
        (row.get(0), row.get(1), row.get(2), row.get(3))
    };
    let held_work = other_work().await;
    // A claim of the own Project Scope with the Chapter, the mark generation, and the fence token
    // of the live claim of the other Project.
    let crossed = ClaimedPassageIndex {
        project_scope: own.scope.clone(),
        chapter_id: other.chapter_id.clone(),
        embedding_registration_revision: HOST_FAKE_EMBEDDING_REGISTRATION.revision.to_owned(),
        mark_generation: held.get(/*idx*/ 0),
        fence_token: held.get(/*idx*/ 1),
        lease: PASSAGE_INDEX_LEASE,
    };
    let configured = store.clone().with_embedding_registration(FAKE);
    let crossed = step_passage_index_work(&configured, &FakeEmbeddings, FAKE_ONLY, &crossed)
        .await
        .unwrap();

    assert_eq!((own_facts, crossed_facts), ((true, 3), (false, 0)));
    assert_eq!(crossed, PassageIndexStep::StaleFence);
    assert_eq!(other_work().await, held_work);
    assert_eq!(
        (
            rows(&admin, &other.scope).await,
            attempts(&admin, &other.scope).await,
            attempts(&admin, &own.scope).await
        ),
        (other_rows, other_attempts, 3)
    );
    assert_eq!(
        nearest(own.scope.clone()).await,
        vec![own.scope.project_id.as_ref().to_owned(); 3]
    );
    assert_eq!(
        nearest(ProjectScope::new(
            UserId::new(own.scope.owner_user_id.as_ref()),
            ProjectId::new("018f0000-0000-7001-8000-0000000028f9"),
        ))
        .await,
        Vec::<String>::new()
    );

    // A Chapter that is not live keeps no row. A second Chapter lets the first go.
    let volume: String = admin
        .query_one(
            "SELECT parent_volume_id::text FROM storyos.manuscript_objects
              WHERE manuscript_object_id = $1::text::uuid",
            &[&own.chapter_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    titled_chapter(&store, &own.scope, "28e2", &volume, "Chapter B", 3).await;
    let deleted = settle_delete_chapter(
        &store,
        &own.scope,
        "28e3",
        &own.chapter_id,
        /*expected_tree_revision*/ 4,
    )
    .await;
    assert!(matches!(deleted.outcome, TransitionOutcome::Applied(_)));
    index(&store, &admin, &own).await;
    assert_eq!(rows(&admin, &own.scope).await, Vec::<String>::new());
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_vector_store_is_available_exactly_when_the_database_offers_the_extension() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let scope = ProjectScope::new(
        UserId::new("018f0000-0000-7001-8000-0000000028f1"),
        ProjectId::new("018f0000-0000-7001-8000-0000000028f2"),
    );
    let offered: bool = admin
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM pg_catalog.pg_available_extensions
                             WHERE name = 'vector')",
            &[],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);

    assert_eq!(vector_table_visible(&store, &scope).await, offered);
}

/// ADR 0095 section 4: the Worker path follows the vector store of the database. With the vector
/// table, the mark of a Chapter gives work that a claim takes, and its steps write one row for
/// each passage. Without it, the mark gives no work, a claim takes nothing, and no Embedding
/// Attempt starts.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_passage_index_work_follows_the_vector_store_of_the_database() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let chapter = chapter(&store, &admin, ["281", "282"], "283", &texts()).await;
    let available = vector_table_visible(&store, &chapter.scope).await;
    let configured = store.clone().with_embedding_registration(FAKE);
    mark(&admin, &chapter).await;
    let work_rows: i64 = admin
        .query_one(
            "SELECT count(*) FROM storyos.passage_index_work WHERE project_id = $1::text::uuid",
            &[&chapter.scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);

    let claim = claim_next_passage_index_work(&configured, FAKE_ONLY)
        .await
        .unwrap();
    let claimed = claim.is_some();
    let indexed = match claim {
        Some(claim) => {
            while step_passage_index_work(&configured, &FakeEmbeddings, FAKE_ONLY, &claim)
                .await
                .unwrap()
                == PassageIndexStep::Written
            {}
            rows(&admin, &chapter.scope).await
        }
        None => Vec::new(),
    };
    let expected = match available {
        true => digests(&texts()),
        false => Vec::new(),
    };

    assert_eq!(
        (
            work_rows > 0,
            claimed,
            indexed,
            attempts(&admin, &chapter.scope).await > 0
        ),
        (available, available, expected, available)
    );
}
