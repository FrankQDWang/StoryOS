//! The shared test support from `passage_vectors_tests.rs`.

use crate::PostgresProjectReader;
use crate::passage_index_work::{MarkedChapters, mark_passage_index_work};
use crate::shared_tests::agent_run_finish_rule::complete;
use crate::shared_tests::create_volume_authority::{NamedEdit, adjusted_edit_command};
use crate::shared_tests::passage_index_work::{deploy, park_other_work, wake};
use crate::shared_tests::project_embedding_binding::{FAKE, embedding_project};
use crate::shared_tests::set_current_chapter_authority::open_session_request;
use crate::statement_cache::Client;
use storyos_adapter_fake_destination::FakeEmbeddings;
use storyos_application::{
    ApplyAuthorEditCommand, ChallengeRateClass, EditorSessionStore, PassageIndexStep, ProjectScope,
    claim_next_passage_index_work, step_passage_index_work,
};
use storyos_core::{
    AuthorEditPrimitive, AuthorEditUnit, EmbeddingRegistration, HOST_FAKE_EMBEDDING_REGISTRATION,
    PassageBlock, SelectionSnapshot, UTF16_COORDINATE_PROFILE, cut_passages,
};

pub(crate) const FAKE_ONLY: &[&EmbeddingRegistration] = &[&HOST_FAKE_EMBEDDING_REGISTRATION];

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

pub(crate) async fn block_ids(admin: &Client, revision_id: &str) -> Vec<String> {
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
pub(crate) async fn edit(
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
pub(crate) async fn mark(admin: &Client, chapter: &Chapter) {
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
