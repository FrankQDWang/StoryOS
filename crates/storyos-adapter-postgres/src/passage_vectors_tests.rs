//! The passage vectors of a Chapter (ADR 0095 sections 2 and 4) with real PostgreSQL and the fake
//! embedding destination. Test prefix lead `28`.

use std::sync::Mutex;

use storyos_adapter_fake_destination::{EMBEDDING_REJECTION_MARKER, FakeEmbeddings};
use storyos_application::{
    ApplyAuthorEditCommand, ChallengeRateClass, EditorSessionStore, EmbeddingObservation,
    EmbeddingProviderAdapter, EmbeddingResult, EmbeddingRoute, PassageIndexFailure,
    PassageIndexReport, PassageIndexResult, PassageVectorStore, PreDispatchRefusal,
    PreparedEmbeddingRequest, ProjectId, ProjectScope, UserId, index_chapter_passages,
};
use storyos_core::{
    AuthorEditPrimitive, AuthorEditUnit, EmbeddingInput, EmbeddingRegistration,
    EmbeddingSettlement, PassageBlock, SelectionSnapshot, TransitionOutcome,
    UTF16_COORDINATE_PROFILE, cut_passages,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::stores;
use crate::agent_run_finish_rule::tests::complete;
use crate::command_sequence::tests::titled_chapter;
use crate::create_volume_authority_tests::{NamedEdit, adjusted_edit_command};
use crate::delete_structure_tests::settle_delete_chapter;
use crate::project_embedding_binding::tests::{FAKE, embedding_project};
use crate::set_current_chapter_authority_tests::open_session_request;

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
fn digests(texts: &[String]) -> Vec<String> {
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
    editor_session_id: String,
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
async fn rows(admin: &Client, scope: &ProjectScope) -> Vec<String> {
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

pub(crate) async fn index(
    store: &PostgresProjectReader,
    adapter: &impl EmbeddingProviderAdapter,
    chapter: &Chapter,
) -> PassageIndexResult {
    index_chapter_passages(store, adapter, FAKE, &chapter.scope, &chapter.chapter_id)
        .await
        .unwrap()
}

fn indexed(unchanged: usize, written: usize, removed: u64) -> PassageIndexResult {
    PassageIndexResult::Indexed(PassageIndexReport {
        passages: 3,
        unchanged,
        written,
        removed,
        failed: Vec::new(),
    })
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_rows_follow_the_current_passages_with_one_request_for_each_new_passage() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut chapter = chapter(&store, &admin, ["281", "282"], "283", &texts()).await;

    let first = index(&store, &FakeEmbeddings, &chapter).await;
    let first_counts = (
        attempts(&admin, &chapter.scope).await,
        rows(&admin, &chapter.scope).await,
    );
    let again = index(&store, &FakeEmbeddings, &chapter).await;
    let again_attempts = attempts(&admin, &chapter.scope).await;
    let command = prepend(&store, &chapter, ("2832", 2), 3, "夜里，").await;
    store.apply_author_edit(&command).await.unwrap();
    chapter.head = head(&admin, &chapter.chapter_id).await;
    let changed = index(&store, &FakeEmbeddings, &chapter).await;
    let mut changed_texts = texts();
    changed_texts[3] = format!("夜里，{}", changed_texts[3]);

    assert_eq!(
        [first, again, changed],
        [indexed(0, 3, 0), indexed(3, 0, 0), indexed(2, 1, 1)]
    );
    assert_eq!(first_counts, (3, digests(&texts())));
    assert_eq!(again_attempts, 3);
    assert_eq!(
        (
            attempts(&admin, &chapter.scope).await,
            rows(&admin, &chapter.scope).await
        ),
        (4, digests(&changed_texts))
    );

    // A loss of all rows loses no author content: the function makes the same rows again.
    admin
        .execute(
            "DELETE FROM storyos.passage_vectors WHERE project_id = $1::text::uuid",
            &[&chapter.scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    let rebuilt = index(&store, &FakeEmbeddings, &chapter).await;
    assert_eq!(rebuilt, indexed(0, 3, 0));
    assert_eq!(
        (
            rows(&admin, &chapter.scope).await,
            head(&admin, &chapter.chapter_id).await
        ),
        (digests(&changed_texts), chapter.head.clone())
    );
}

/// The fake embedding adapter. Its first exchange applies `edit` to the Chapter before it gives
/// the vector, so the Chapter changes between the read and the write of the call.
struct Held<'a> {
    store: &'a PostgresProjectReader,
    edit: Mutex<Option<ApplyAuthorEditCommand>>,
}

impl EmbeddingProviderAdapter for Held<'_> {
    type Prepared = <FakeEmbeddings as EmbeddingProviderAdapter>::Prepared;

    async fn prepare(
        &self,
        registration: &'static EmbeddingRegistration,
        route: &EmbeddingRoute,
        input: &EmbeddingInput,
    ) -> Result<PreparedEmbeddingRequest<Self::Prepared>, PreDispatchRefusal> {
        FakeEmbeddings.prepare(registration, route, input).await
    }

    async fn exchange(&self, prepared: Self::Prepared) -> EmbeddingObservation {
        let edit = self.edit.lock().unwrap().take();
        if let Some(edit) = edit {
            self.store.apply_author_edit(&edit).await.unwrap();
        }
        FakeEmbeddings.exchange(prepared).await
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_change_during_the_call_keeps_the_written_rows_and_the_next_call_removes_them() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut chapter = chapter(&store, &admin, ["284", "285"], "286", &texts()).await;
    let held = Held {
        store: &store,
        edit: Mutex::new(Some(
            prepend(&store, &chapter, ("2862", 2), 3, "夜里，").await,
        )),
    };

    let during = index(&store, &held, &chapter).await;
    let written = rows(&admin, &chapter.scope).await;
    chapter.head = head(&admin, &chapter.chapter_id).await;
    let next = index(&store, &FakeEmbeddings, &chapter).await;
    let mut changed_texts = texts();
    changed_texts[3] = format!("夜里，{}", changed_texts[3]);

    // Each written row is a fact about the passage text that the call read.
    assert_eq!((during, written), (indexed(0, 3, 0), digests(&texts())));
    assert_eq!(next, indexed(2, 1, 1));
    assert_eq!(rows(&admin, &chapter.scope).await, digests(&changed_texts));
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_refused_request_writes_no_row_and_the_other_rows_stay() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut marked = texts();
    marked[1] = format!("{} {EMBEDDING_REJECTION_MARKER}", marked[1]);
    let chapter = chapter(&store, &admin, ["287", "288"], "289", &marked).await;
    let rejected = digests(&marked)
        .into_iter()
        .find(|digest| !digests(&texts()).contains(digest))
        .unwrap();
    let failure = |unchanged, written| {
        PassageIndexResult::Indexed(PassageIndexReport {
            passages: 3,
            unchanged,
            written,
            removed: 0,
            failed: vec![PassageIndexFailure {
                passage_digest: rejected.clone(),
                result: EmbeddingResult::Settled(EmbeddingSettlement::Rejected),
            }],
        })
    };

    let first = index(&store, &FakeEmbeddings, &chapter).await;
    let again = index(&store, &FakeEmbeddings, &chapter).await;

    assert_eq!([first, again], [failure(0, 2), failure(2, 0)]);
    let mut kept = digests(&marked);
    kept.retain(|digest| *digest != rejected);
    assert_eq!(
        (
            rows(&admin, &chapter.scope).await,
            attempts(&admin, &chapter.scope).await
        ),
        (kept, 4)
    );
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
    index(&store, &FakeEmbeddings, &own).await;
    index(&store, &FakeEmbeddings, &other).await;
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

    let crossed =
        index_chapter_passages(&store, &FakeEmbeddings, FAKE, &own.scope, &other.chapter_id)
            .await
            .unwrap();

    assert_eq!(crossed, PassageIndexResult::ChapterNotLive { removed: 0 });
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
    assert_eq!(
        index(&store, &FakeEmbeddings, &own).await,
        PassageIndexResult::ChapterNotLive { removed: 3 }
    );
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

    assert_eq!(store.vector_store_available(&scope).await.unwrap(), offered);
}
