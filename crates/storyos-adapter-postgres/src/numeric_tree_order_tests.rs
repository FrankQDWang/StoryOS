//! Canonical sibling order of ten or more siblings is numeric (ADR 0018).
//! A text order puts rank 10 before rank 2.

use storyos_application::{
    CanonicalManuscriptTree, CreateVolumeInput, GetManuscriptTree, ProjectScope,
    get_manuscript_tree, issue_project_command_challenge,
    render_readable_manuscript_from_pinned_source,
};
use tokio_postgres::NoTls;

use crate::PostgresProjectReader;
use crate::command_sequence::tests::{
    applied, command_call, create_volume, update_chapter, update_volume,
};
use crate::pinned_export_source::{
    PinnedExportSourceCompleteness, PinnedExportSourceLoad, load_pinned_export_source,
};
use crate::update_volume_tests::{apply_chapter, named_issue, seed_project};

const VOLUME_COUNT: u16 = 10;
const CHAPTER_COUNT: u16 = 12;

fn suffix(value: u16) -> String {
    format!("{value:04x}")
}

fn structure_bytes(title: &str, expected_tree_revision: u64) -> Vec<u8> {
    format!(r#"{{"expected_tree_revision":"{expected_tree_revision}","title":"{title}"}}"#)
        .into_bytes()
}

fn update_bytes(title: &str, order: u64, expected_tree_revision: u64) -> Vec<u8> {
    format!(
        r#"{{"expected_tree_revision":"{expected_tree_revision}","order":"{order}","title":"{title}"}}"#
    )
    .into_bytes()
}

fn digest(profile: &str, bytes: &[u8]) -> String {
    format!("sha256:{profile}:{}", crate::author_edit::sha256_hex(bytes))
}

/// Sibling titles with their ranks, in tree order.
type RankedTitles = Vec<(String, u64)>;

fn numbered(prefix: &str, count: u16) -> Vec<String> {
    (1..=count)
        .map(|index| format!("{prefix} {index}"))
        .collect()
}

async fn connect_admin() -> tokio_postgres::Client {
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let (admin, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        connection.await.unwrap();
    });
    admin
}

fn runtime_store() -> PostgresProjectReader {
    PostgresProjectReader::new(
        std::env::var("STORYOS_TEST_DATABASE_URL")
            .expect("run through scripts/verify-project-scope.sh"),
    )
}

/// One test sends more author commands than one fixed challenge window admits.
async fn reset_challenge_rate(admin: &tokio_postgres::Client, scope: &ProjectScope) {
    admin
        .execute(
            "UPDATE storyos.project_command_challenge_rate_windows SET issued_count = 0
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .unwrap();
}

async fn apply_titled_volume(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    issue_suffix: &str,
    title: &str,
    expected_tree_revision: u64,
) -> String {
    let bytes = structure_bytes(title, expected_tree_revision);
    let issue = named_issue(
        scope,
        issue_suffix,
        "POST",
        "/api/v1/projects/{project_id}/volumes",
        "storyos.command.create-volume.request.v1",
        "createVolume",
        &digest("storyos.command.createVolume.jcs.v1", &bytes),
    );
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    let settlement = create_volume(
        store,
        &command_call(
            issue.binding,
            &issue.nonce_digest,
            issue_suffix,
            &bytes,
            CreateVolumeInput {
                title: title.to_owned(),
                expected_tree_revision,
            },
        ),
    )
    .await
    .unwrap();
    applied(&settlement).0.volume_id
}

/// Seeds ten Volumes and twelve Chapters in the first Volume. The tree revision is 23 after it.
async fn seed_long_tree(
    store: &PostgresProjectReader,
    admin: &tokio_postgres::Client,
    first_suffix: u16,
) -> (ProjectScope, String) {
    let scope = seed_project(store, &suffix(first_suffix)).await;
    let mut volume_ids = Vec::new();
    for (index, title) in numbered("Volume", VOLUME_COUNT).iter().enumerate() {
        let tree_revision = index as u64 + 1;
        reset_challenge_rate(admin, &scope).await;
        volume_ids.push(
            apply_titled_volume(
                store,
                &scope,
                &suffix(first_suffix + 0x10 + index as u16),
                title,
                tree_revision,
            )
            .await,
        );
    }
    for (index, title) in numbered("Chapter", CHAPTER_COUNT).iter().enumerate() {
        let tree_revision = u64::from(VOLUME_COUNT) + index as u64 + 1;
        reset_challenge_rate(admin, &scope).await;
        apply_chapter(
            store,
            &scope,
            &suffix(first_suffix + 0x30 + index as u16),
            &volume_ids[0],
            title,
            &structure_bytes(title, tree_revision),
            tree_revision,
        )
        .await;
    }
    (scope, volume_ids.swap_remove(/*index*/ 0))
}

async fn read_tree(store: &PostgresProjectReader, scope: &ProjectScope) -> CanonicalManuscriptTree {
    let GetManuscriptTree::Found(tree) = get_manuscript_tree(store, scope).await.unwrap() else {
        panic!("the Project has a Canonical Query");
    };
    *tree
}

/// Returns the titles and ranks of the Volumes and of the Chapters in the seeded first Volume.
fn tree_titles(
    tree: &CanonicalManuscriptTree,
    first_volume_id: &str,
) -> (RankedTitles, RankedTitles) {
    let volumes = tree
        .volumes
        .iter()
        .map(|volume| (volume.title.clone(), volume.order))
        .collect();
    let chapters = tree
        .volumes
        .iter()
        .find(|volume| volume.volume_id.as_ref() == first_volume_id)
        .expect("the seeded first Volume is in the tree")
        .chapters
        .iter()
        .map(|chapter| (chapter.title.clone(), chapter.order))
        .collect();
    (volumes, chapters)
}

fn ranked(titles: &[String]) -> RankedTitles {
    titles
        .iter()
        .enumerate()
        .map(|(index, title)| (title.clone(), index as u64 + 1))
        .collect()
}

/// Returns the Volume titles and the first Volume Chapter titles in physical storage order.
async fn storage_titles(
    admin: &tokio_postgres::Client,
    scope: &ProjectScope,
    first_volume_id: &str,
) -> (String, String) {
    let row = admin
        .query_one(
            "SELECT
               (SELECT string_agg(title, ',' ORDER BY tree_order)
                  FROM storyos.manuscript_objects
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                   AND object_kind = 'volume'),
               (SELECT string_agg(title, ',' ORDER BY tree_order)
                  FROM storyos.manuscript_objects
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                   AND object_kind = 'chapter'
                   AND parent_volume_id = $3::text::uuid)",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &first_volume_id,
            ],
        )
        .await
        .unwrap();
    (row.get(/*idx*/ 0), row.get(/*idx*/ 1))
}

async fn readable_export_headings(
    store: &PostgresProjectReader,
    admin: &tokio_postgres::Client,
    scope: &ProjectScope,
) -> Vec<String> {
    reset_challenge_rate(admin, scope).await;
    let export_id = crate::export_work::tests::admit_readable_export(store, scope).await;
    let snapshot_id: String = admin
        .query_one(
            "SELECT source_snapshot_id::text FROM storyos.pinned_export_sources
              WHERE export_id = $1::text::uuid",
            &[&export_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let PinnedExportSourceLoad::Available(source) = load_pinned_export_source(
        admin,
        scope,
        &export_id,
        &snapshot_id,
        PinnedExportSourceCompleteness::HumanReadableManuscript,
    )
    .await
    .unwrap() else {
        panic!("the readable export pins a complete source");
    };
    crate::export_work::tests::remove_export_work_rows(admin).await;
    render_readable_manuscript_from_pinned_source(&source)
        .lines()
        .filter(|line| line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn tree_and_readable_export_list_ten_or_more_siblings_in_numeric_rank_order() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let store = runtime_store();
    let admin = connect_admin().await;
    let (scope, first_volume_id) = seed_long_tree(&store, &admin, /*first_suffix*/ 0xc000).await;

    let volume_titles = numbered("Volume", VOLUME_COUNT);
    let chapter_titles = numbered("Chapter", CHAPTER_COUNT);
    let mut expected_headings = vec![format!("# {}", volume_titles[0])];
    expected_headings.extend(chapter_titles.iter().map(|title| format!("## {title}")));
    expected_headings.extend(volume_titles[1..].iter().map(|title| format!("# {title}")));
    assert_eq!(
        (
            tree_titles(&read_tree(&store, &scope).await, &first_volume_id),
            readable_export_headings(&store, &admin, &scope).await,
        ),
        (
            (ranked(&volume_titles), ranked(&chapter_titles)),
            expected_headings
        )
    );
}

/// One Update Chapter or Update Volume request from the editor.
struct TreeEdit<'a> {
    issue_suffix: &'a str,
    current_title: &'a str,
    title: &'a str,
    /// `None` sends the rank that the public tree returns, as a rename in the editor does.
    order: Option<u64>,
    expected_tree_revision: u64,
}

async fn update_chapter_by_title(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    edit: TreeEdit<'_>,
) {
    let tree = read_tree(store, scope).await;
    let chapter = tree
        .volumes
        .iter()
        .flat_map(|volume| &volume.chapters)
        .find(|chapter| chapter.title == edit.current_title)
        .expect("the Chapter is in the tree");
    let order = edit.order.unwrap_or(chapter.order);
    let bytes = update_bytes(edit.title, order, edit.expected_tree_revision);
    let issue = crate::update_chapter_tests::update_issue(
        scope,
        edit.issue_suffix,
        &digest("storyos.command.updateChapter.jcs.v1", &bytes),
    );
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    let settlement = update_chapter(
        store,
        &crate::update_chapter_tests::update_command(
            issue.binding,
            &issue.nonce_digest,
            edit.issue_suffix,
            crate::update_chapter_tests::UpdateFixture {
                chapter_id: chapter.chapter_id.as_ref(),
                title: edit.title,
                order,
                expected_tree_revision: edit.expected_tree_revision,
                bytes: &bytes,
            },
        ),
    )
    .await
    .unwrap();
    applied(&settlement);
}

async fn update_volume_by_title(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    edit: TreeEdit<'_>,
) {
    let tree = read_tree(store, scope).await;
    let volume = tree
        .volumes
        .iter()
        .find(|volume| volume.title == edit.current_title)
        .expect("the Volume is in the tree");
    let order = edit.order.unwrap_or(volume.order);
    let bytes = update_bytes(edit.title, order, edit.expected_tree_revision);
    let issue = crate::update_volume_tests::update_issue(
        scope,
        edit.issue_suffix,
        &digest("storyos.command.updateVolume.jcs.v1", &bytes),
    );
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    let settlement = update_volume(
        store,
        &crate::update_volume_tests::update_command(
            issue.binding,
            &issue.nonce_digest,
            edit.issue_suffix,
            crate::update_volume_tests::UpdateFixture {
                volume_id: volume.volume_id.as_ref(),
                title: edit.title,
                order,
                expected_tree_revision: edit.expected_tree_revision,
                bytes: &bytes,
            },
        ),
    )
    .await
    .unwrap();
    applied(&settlement);
}

async fn assert_tree_and_storage_order(
    store: &PostgresProjectReader,
    admin: &tokio_postgres::Client,
    scope: &ProjectScope,
    first_volume_id: &str,
    volume_titles: &[String],
    chapter_titles: &[String],
) {
    assert_eq!(
        tree_titles(&read_tree(store, scope).await, first_volume_id),
        (ranked(volume_titles), ranked(chapter_titles))
    );
    assert_eq!(
        storage_titles(admin, scope, first_volume_id).await,
        (volume_titles.join(","), chapter_titles.join(","))
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn rename_and_move_with_ten_or_more_siblings_keep_the_physical_order() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let store = runtime_store();
    let admin = connect_admin().await;
    let (scope, first_volume_id) = seed_long_tree(&store, &admin, /*first_suffix*/ 0xc100).await;
    let mut volume_titles = numbered("Volume", VOLUME_COUNT);
    let mut chapter_titles = numbered("Chapter", CHAPTER_COUNT);

    let renamed = "Chapter 12 Renamed".to_owned();
    update_chapter_by_title(
        &store,
        &scope,
        TreeEdit {
            issue_suffix: "c180",
            current_title: "Chapter 12",
            title: &renamed,
            order: None,
            expected_tree_revision: 23,
        },
    )
    .await;
    chapter_titles[11] = renamed;
    let moved = chapter_titles.remove(/*index*/ 0);
    update_chapter_by_title(
        &store,
        &scope,
        TreeEdit {
            issue_suffix: "c181",
            current_title: &moved,
            title: &moved,
            order: Some(12),
            expected_tree_revision: 24,
        },
    )
    .await;
    chapter_titles.push(moved);
    assert_tree_and_storage_order(
        &store,
        &admin,
        &scope,
        &first_volume_id,
        &volume_titles,
        &chapter_titles,
    )
    .await;

    let renamed = "Volume 10 Renamed".to_owned();
    update_volume_by_title(
        &store,
        &scope,
        TreeEdit {
            issue_suffix: "c182",
            current_title: "Volume 10",
            title: &renamed,
            order: None,
            expected_tree_revision: 25,
        },
    )
    .await;
    volume_titles[9] = renamed;
    let moved = volume_titles.remove(/*index*/ 0);
    update_volume_by_title(
        &store,
        &scope,
        TreeEdit {
            issue_suffix: "c183",
            current_title: &moved,
            title: &moved,
            order: Some(10),
            expected_tree_revision: 26,
        },
    )
    .await;
    volume_titles.push(moved);
    assert_tree_and_storage_order(
        &store,
        &admin,
        &scope,
        &first_volume_id,
        &volume_titles,
        &chapter_titles,
    )
    .await;
}
