//! The passage vectors of a Chapter (ADR 0095 sections 2 and 4) with real PostgreSQL and the fake
//! embedding destination. Test prefix lead `28`.

use storyos_adapter_fake_destination::FakeEmbeddings;
use storyos_application::{
    ApplyAuthorEditCommand, ChallengeRateClass, ClaimedPassageIndex, EditorSessionStore,
    PASSAGE_INDEX_LEASE, PassageIndexStep, PassageVectorStore, ProjectId, ProjectScope, UserId,
    claim_next_passage_index_work, step_passage_index_work,
};
use storyos_core::{
    AuthorEditPrimitive, AuthorEditUnit, EmbeddingRegistration, HOST_FAKE_EMBEDDING_REGISTRATION,
    PassageBlock, SelectionSnapshot, TransitionOutcome, UTF16_COORDINATE_PROFILE, cut_passages,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::stores;
use crate::agent_run_finish_rule::tests::complete;
use crate::command_sequence::tests::titled_chapter;
use crate::create_volume_authority_tests::{NamedEdit, adjusted_edit_command};
use crate::delete_structure_tests::settle_delete_chapter;
use crate::passage_index_work::tests::{deploy, park_other_work, wake};
use crate::passage_index_work::{MarkedChapters, mark_passage_index_work};
use crate::project_embedding_binding::tests::{FAKE, embedding_project};
use crate::set_current_chapter_authority_tests::open_session_request;

const FAKE_ONLY: &[&EmbeddingRegistration] = &[&HOST_FAKE_EMBEDDING_REGISTRATION];

pub(crate) const TIDE: &str = "潮水退去，灯塔亮了。";
pub(crate) const SHORE: &str = "她沿着堤岸慢慢走。";
pub(crate) const WIND: &str = "风从海上吹来，带着盐。";
pub(crate) const BOATS: &str = "第二天早上，渔船回港。";

/// The blocks of each test Chapter: two blocks of 300 and 270 characters, a scene separator, and
/// two blocks of 220 and 660 characters. They give three passages.
pub(crate) fn texts() -> Vec<String> {
    vec![
        TIDE.repeat(30),
        SHORE.repeat(30),
        "＊＊＊".to_owned(),
        WIND.repeat(20),
        BOATS.repeat(60),
    ]
}

/// The sorted digests of the passages of `texts`.
pub(crate) fn digests(texts: &[String]) -> Vec<String> {
    let blocks: Vec<PassageBlock<'_>> = texts
        .iter()
        .map(|text| PassageBlock {
            block_id: "b",
            text,
        })
        .collect();
    let mut digests: Vec<String> = cut_passages(&blocks)
        .into_iter()
        .map(|passage| passage.digest)
        .collect();
    digests.sort();
    digests
}

/// One Chapter with its Editor Session, its head, and its blocks.
pub(crate) struct Chapter {
    pub(crate) scope: ProjectScope,
    pub(crate) chapter_id: String,
    pub(crate) editor_session_id: String,
    pub(crate) head: String,
    pub(crate) blocks: Vec<String>,
}

async fn block_ids(admin: &Client, revision_id: &str) -> Vec<String> {
    admin
        .query(
            "SELECT manuscript_block_id::text FROM storyos.manuscript_revision_members
              WHERE revision_id = $1::text::uuid ORDER BY block_order",
            &[&revision_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(/*idx*/ 0))
        .collect()
}

pub(crate) async fn head(admin: &Client, chapter_id: &str) -> String {
    admin
        .query_one(
            "SELECT current_revision_id::text FROM storyos.authoritative_heads
              WHERE manuscript_object_id = $1::text::uuid",
            &[&chapter_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// The Author Edit of `chapter` with the one unit of `primitives`.
async fn edit(
    store: &PostgresProjectReader,
    chapter: &Chapter,
    named: (&str, u64),
    primitives: Vec<AuthorEditPrimitive>,
) -> ApplyAuthorEditCommand {
    edit_at(store, chapter, named, primitives, /*selection*/ 0).await
}

/// The Author Edit of `chapter` with the one unit of `primitives` and an empty selection at the
/// UTF-16 offset `selection`. A join has its selection at the end of its left block.
pub(crate) async fn edit_at(
    store: &PostgresProjectReader,
    chapter: &Chapter,
    (suffix, sequence): (&str, u64),
    primitives: Vec<AuthorEditPrimitive>,
    selection: u32,
) -> ApplyAuthorEditCommand {
    let edit = NamedEdit {
        editor_session_id: &chapter.editor_session_id,
        chapter_id: &chapter.chapter_id,
        expected_revision_id: &chapter.head,
        suffix,
        local_intent_sequence: sequence,
        text: "",
        proposal_target: None,
    };
    adjusted_edit_command(store, &chapter.scope, edit, |command| {
        // The Author Edit class keeps the Shared Challenges of the Project for the Runs.
        command.challenge_binding.challenge_rate_policy_revision =
            ChallengeRateClass::AuthorEdit.policy_revision().to_owned();
        command.author_edit_units = vec![AuthorEditUnit {
            normalized_primitives: primitives,
            selection_snapshot: SelectionSnapshot {
                coordinate_profile: UTF16_COORDINATE_PROFILE.to_owned(),
                from: selection,
                to: selection,
            },
        }];
    })
    .await
}

/// A Project with the fake embedding binding whose Chapter has one block for each of `texts`.
/// `prefixes` give the identities of the Project, and `session` the four hex digits of the Editor
/// Session and of its first Author Edit (`session` with its last digit 1).
pub(crate) async fn chapter(
    store: &PostgresProjectReader,
    admin: &Client,
    prefixes: [&str; 2],
    session: &str,
    texts: &[String],
) -> Chapter {
    let (scope, chapter_id, run_id) = embedding_project(store, admin, prefixes, FAKE).await;
    complete(store, admin, &scope, &run_id).await;
    let request = open_session_request(store, &scope, &format!("{session}0")).await;
    let created = store.create_editor_session(&request).await.unwrap();
    let base = created.base_snapshot.authoritative_revision_id;
    let first = block_ids(admin, &base).await.remove(0);
    let mut primitives = vec![AuthorEditPrimitive::ReplaceBlockSelection {
        manuscript_block_id: first.clone(),
        from: 0,
        to: 0,
        text: texts.concat(),
    }];
    let mut split = first;
    for text in &texts[..texts.len() - 1] {
        let new_block = uuid::Uuid::now_v7().to_string();
        primitives.push(AuthorEditPrimitive::SplitBlock {
            manuscript_block_id: split,
            offset: text.encode_utf16().count() as u32,
            new_manuscript_block_id: new_block.clone(),
        });
        split = new_block;
    }
    let mut chapter = Chapter {
        scope,
        chapter_id,
        editor_session_id: request.editor_session_id.as_ref().to_owned(),
        head: base,
        blocks: Vec::new(),
    };
    let command = edit(store, &chapter, (&format!("{session}1"), 1), primitives).await;
    store.apply_author_edit(&command).await.unwrap();
    chapter.head = head(admin, &chapter.chapter_id).await;
    chapter.blocks = block_ids(admin, &chapter.head).await;
    assert_eq!(chapter.blocks.len(), texts.len());
    chapter
}

/// The Author Edit that puts `text` before the text of block `index`.
pub(crate) async fn prepend(
    store: &PostgresProjectReader,
    chapter: &Chapter,
    (suffix, sequence): (&str, u64),
    index: usize,
    text: &str,
) -> ApplyAuthorEditCommand {
    let primitive = AuthorEditPrimitive::ReplaceBlockSelection {
        manuscript_block_id: chapter.blocks[index].clone(),
        from: 0,
        to: 0,
        text: text.to_owned(),
    };
    edit(store, chapter, (suffix, sequence), vec![primitive]).await
}

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

/// The sorted digests of the rows of the Project.
pub(crate) async fn rows(admin: &Client, scope: &ProjectScope) -> Vec<String> {
    admin
        .query(
            "SELECT passage_digest FROM storyos.passage_vectors
              WHERE project_id = $1::text::uuid ORDER BY passage_digest",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(/*idx*/ 0))
        .collect()
}

/// Marks the Chapter for the deployment with the fake Embedding Registration, ends its quiet time,
/// and moves the work of each other Project one day later. `admin` is the separate admin
/// connection of `stores`, and not a pooled client.
async fn mark(admin: &Client, chapter: &Chapter) {
    admin.batch_execute("BEGIN").await.unwrap();
    deploy(admin).await;
    let changed = [chapter.chapter_id.as_str()];
    mark_passage_index_work(admin, &chapter.scope, MarkedChapters::Changed(&changed))
        .await
        .unwrap();
    admin.batch_execute("COMMIT").await.unwrap();
    wake(admin, &chapter.scope).await;
    park_other_work(admin, &chapter.scope).await;
}

/// Indexes the Chapter through the Worker path with the fake embedding destination: a mark of the
/// Chapter and the end of its quiet time, the work of each other Project waits, and each claim
/// steps until a step does not write.
pub(crate) async fn index(store: &PostgresProjectReader, admin: &Client, chapter: &Chapter) {
    let store = store.clone().with_embedding_registration(FAKE);
    mark(admin, chapter).await;
    while let Some(claim) = claim_next_passage_index_work(&store, FAKE_ONLY)
        .await
        .unwrap()
    {
        while step_passage_index_work(&store, &FakeEmbeddings, FAKE_ONLY, &claim)
            .await
            .unwrap()
            == PassageIndexStep::Written
        {}
    }
}

/// If the vector store is available to the runtime role: the database has the vector table.
pub(crate) async fn vector_table_visible(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
) -> bool {
    store
        .in_read_transaction(scope, async |client| {
            Ok(client
                .query_one(
                    "SELECT to_regclass('storyos.passage_vectors') IS NOT NULL",
                    &[],
                )
                .await
                .unwrap()
                .get(/*idx*/ 0))
        })
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_read_in_one_project_scope_gets_only_its_rows_and_cannot_name_another_chapter() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
