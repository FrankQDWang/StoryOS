//! The Chapter rank batch of Update Chapter: one rank write for many siblings, and the scope of
//! the write. `command_sequence_tests/` owns replay and rollback.

use storyos_application::{
    CanonicalManuscriptTree, ChapterId, ChapterNode, GetManuscriptTree, ProjectScope,
    UpdateChapterInput, UpdateChapterSettlement, UserId, VolumeId, VolumeNode, get_manuscript_tree,
    open_project,
};
use storyos_core::{TransitionOutcome, UpdateChapterApplied};

use crate::PostgresProjectReader;
use crate::command_sequence::tests::{
    CommandCall, UPDATE_CHAPTER, applied, named_call, seed_project, stores, titled_chapter,
    titled_volume, update_chapter,
};
use crate::delete_structure_tests::settle_delete_chapter;

const USER_B: &str = "018f0000-0000-7001-8000-000000000101";

fn hex_suffix(value: u16) -> String {
    format!("{value:04x}")
}

async fn apply_live_chapters(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    volume_id: &str,
    first_suffix: u16,
    count: u64,
    first_tree_revision: u64,
) -> Vec<String> {
    let mut chapter_ids = Vec::new();
    for index in 0..count {
        let title = format!("Chapter {}", index + 1);
        let suffix = hex_suffix(first_suffix + index as u16);
        let revision = first_tree_revision + index;
        chapter_ids.push(titled_chapter(store, scope, &suffix, volume_id, &title, revision).await);
    }
    chapter_ids
}

async fn issue_update(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
    chapter_id: &str,
    title: &str,
    order: u64,
    expected_tree_revision: u64,
) -> CommandCall<UpdateChapterInput> {
    let input = UpdateChapterInput {
        chapter_id: ChapterId::new(chapter_id),
        title: title.to_owned(),
        order,
        expected_tree_revision,
    };
    named_call(store, scope, suffix, &UPDATE_CHAPTER, input).await
}

fn chapter_nodes(ids: &[String], titles: &[&str]) -> Vec<ChapterNode> {
    ids.iter()
        .zip(titles.iter().copied())
        .enumerate()
        .map(|(index, (chapter_id, title))| ChapterNode {
            chapter_id: ChapterId::new(chapter_id.clone()),
            title: title.to_owned(),
            order: index as u64 + 1,
        })
        .collect()
}

async fn drop_chapter_tree_sql_probes(admin: &tokio_postgres::Client) {
    admin
        .batch_execute(
            "DROP TRIGGER IF EXISTS storyos_issue_650_count_manuscript_objects ON storyos.manuscript_objects;
             DROP TRIGGER IF EXISTS storyos_issue_650_count_projects ON storyos.projects;
             DROP FUNCTION IF EXISTS public.storyos_issue_650_count_manuscript_objects();
             DROP FUNCTION IF EXISTS public.storyos_issue_650_count_projects();
             DROP TABLE IF EXISTS public.storyos_issue_650_sql_calls;",
        )
        .await
        .unwrap();
}

async fn install_chapter_tree_sql_probes(admin: &tokio_postgres::Client) {
    drop_chapter_tree_sql_probes(admin).await;
    admin
        .batch_execute(
            "CREATE TABLE public.storyos_issue_650_sql_calls (
                name text PRIMARY KEY, statement_count bigint NOT NULL);
             INSERT INTO public.storyos_issue_650_sql_calls (name, statement_count)
             VALUES ('manuscript_objects', 0), ('projects', 0), ('rank_write', 0), ('rank_rows', 0);

             CREATE FUNCTION public.storyos_issue_650_count_manuscript_objects()
             RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER
             SET search_path = pg_catalog, public AS $$
             BEGIN
               UPDATE public.storyos_issue_650_sql_calls
                  SET statement_count = statement_count + 1 WHERE name = 'manuscript_objects';
               IF EXISTS (
                 SELECT 1 FROM new_rows n JOIN old_rows o
                   ON o.owner_user_id = n.owner_user_id AND o.project_id = n.project_id
                  AND o.manuscript_object_id = n.manuscript_object_id
                 WHERE n.object_kind = 'chapter'
                   AND n.tree_order IS DISTINCT FROM o.tree_order
                   AND n.tree_order <= 1000000
               ) THEN
                 UPDATE public.storyos_issue_650_sql_calls
                    SET statement_count = statement_count + 1 WHERE name = 'rank_write';
                 UPDATE public.storyos_issue_650_sql_calls
                    SET statement_count = statement_count + (
                      SELECT count(*) FROM new_rows n JOIN old_rows o
                        ON o.owner_user_id = n.owner_user_id AND o.project_id = n.project_id
                       AND o.manuscript_object_id = n.manuscript_object_id
                       WHERE n.object_kind = 'chapter'
                         AND n.tree_order IS DISTINCT FROM o.tree_order
                         AND n.tree_order <= 1000000
                    ) WHERE name = 'rank_rows';
               END IF;
               RETURN NULL;
             END $$;

             CREATE FUNCTION public.storyos_issue_650_count_projects()
             RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER
             SET search_path = pg_catalog, public AS $$
             BEGIN
               UPDATE public.storyos_issue_650_sql_calls
                  SET statement_count = statement_count + 1 WHERE name = 'projects';
               RETURN NULL;
             END $$;

             CREATE TRIGGER storyos_issue_650_count_manuscript_objects
             AFTER UPDATE ON storyos.manuscript_objects
             REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows
             FOR EACH STATEMENT
             EXECUTE FUNCTION public.storyos_issue_650_count_manuscript_objects();
             CREATE TRIGGER storyos_issue_650_count_projects
             AFTER UPDATE ON storyos.projects FOR EACH STATEMENT
             EXECUTE FUNCTION public.storyos_issue_650_count_projects();",
        )
        .await
        .unwrap();
}

async fn chapter_tree_sql_counts(admin: &tokio_postgres::Client) -> (i64, i64, i64, i64) {
    let counts: serde_json::Value = serde_json::from_str(
        &admin
            .query_one(
                "SELECT jsonb_object_agg(name, statement_count)::text
                   FROM public.storyos_issue_650_sql_calls",
                &[],
            )
            .await
            .unwrap()
            .get::<_, String>(0),
    )
    .unwrap();
    (
        counts["manuscript_objects"].as_i64().unwrap(),
        counts["projects"].as_i64().unwrap(),
        counts["rank_write"].as_i64().unwrap(),
        counts["rank_rows"].as_i64().unwrap(),
    )
}

fn assert_applied_reorder_tree(
    tree: &CanonicalManuscriptTree,
    settlement: &UpdateChapterSettlement,
    volume_id: &str,
    expected_ids: &[String],
    expected_titles: &[&str],
    tree_revision: u64,
) {
    let (_, authority) = applied(settlement);
    assert_eq!(tree.tree_revision, tree_revision);
    assert_eq!(tree.snapshot.snapshot_id, authority.snapshot_id);
    let TransitionOutcome::Applied(settled) = &settlement.outcome else {
        panic!("Update Chapter must apply");
    };
    assert_eq!(
        tree.snapshot.project_activity_position,
        settled.project_activity_position
    );
    assert_eq!(
        tree.volumes,
        vec![VolumeNode {
            volume_id: VolumeId::new(volume_id),
            title: "Volume A".to_owned(),
            order: 1,
            chapters: chapter_nodes(expected_ids, expected_titles),
        }]
    );
}

async fn seed_volume_with_chapters(
    store: &PostgresProjectReader,
    project_suffix: u16,
    chapter_count: u64,
) -> (ProjectScope, String, Vec<String>) {
    let scope = seed_project(store, &hex_suffix(project_suffix)).await;
    let volume_suffix = hex_suffix(project_suffix + 1);
    let volume_id = titled_volume(store, &scope, &volume_suffix, "Volume A", 1).await;
    let chapter_ids = apply_live_chapters(
        store,
        &scope,
        &volume_id,
        project_suffix + 2,
        chapter_count,
        /*first_tree_revision*/ 2,
    )
    .await;
    (scope, volume_id, chapter_ids)
}

async fn measure_reorder(
    store: &PostgresProjectReader,
    admin: &tokio_postgres::Client,
    command: &CommandCall<UpdateChapterInput>,
) -> ((i64, i64, i64, i64), UpdateChapterSettlement) {
    install_chapter_tree_sql_probes(admin).await;
    let settlement = update_chapter(store, command).await.unwrap();
    let counts = chapter_tree_sql_counts(admin).await;
    drop_chapter_tree_sql_probes(admin).await;
    (counts, settlement)
}

struct MeasuredReorder {
    project_suffix: u16,
    chapter_count: u64,
    move_from: usize,
    move_to: u64,
    title: &'static str,
    expected_tree_revision: u64,
    tree_revision: u64,
    titles: &'static [&'static str],
    issue_suffix: u16,
}

async fn measure_applied_reorder(
    store: &PostgresProjectReader,
    admin: &tokio_postgres::Client,
    case: MeasuredReorder,
) {
    let (scope, volume_id, chapter_ids) =
        seed_volume_with_chapters(store, case.project_suffix, case.chapter_count).await;
    let mut expected_ids = chapter_ids.clone();
    let moved = expected_ids.remove(case.move_from);
    expected_ids.insert((case.move_to - 1) as usize, moved);
    let command = issue_update(
        store,
        &scope,
        &hex_suffix(case.issue_suffix),
        &chapter_ids[case.move_from],
        case.title,
        case.move_to,
        case.expected_tree_revision,
    )
    .await;
    let ((object_updates, project_updates, rank_writes, rank_rows), settlement) =
        measure_reorder(store, admin, &command).await;
    assert_eq!(
        applied(&settlement).0,
        UpdateChapterApplied {
            title: case.title.to_owned(),
            order: case.move_to,
            tree_revision: case.tree_revision,
        }
    );
    assert_eq!(
        (rank_writes, rank_rows, object_updates + project_updates),
        (1, case.chapter_count as i64, 4)
    );
    let GetManuscriptTree::Found(tree) = get_manuscript_tree(store, &scope).await.unwrap() else {
        panic!("the Project still has a Canonical Query");
    };
    assert_applied_reorder_tree(
        &tree,
        &settlement,
        &volume_id,
        &expected_ids,
        case.titles,
        case.tree_revision,
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn applied_chapter_reorder_uses_one_rank_write_for_many_siblings() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    measure_applied_reorder(
        &store,
        &admin,
        MeasuredReorder {
            project_suffix: 0x6500,
            chapter_count: 5,
            move_from: 4,
            move_to: 1,
            title: "Chapter 5",
            expected_tree_revision: 7,
            tree_revision: 8,
            titles: &[
                "Chapter 5",
                "Chapter 1",
                "Chapter 2",
                "Chapter 3",
                "Chapter 4",
            ],
            issue_suffix: 0x6507,
        },
    )
    .await;
    measure_applied_reorder(
        &store,
        &admin,
        MeasuredReorder {
            project_suffix: 0x6510,
            chapter_count: 7,
            move_from: 0,
            move_to: 7,
            title: "Chapter 1",
            expected_tree_revision: 9,
            tree_revision: 10,
            titles: &[
                "Chapter 2",
                "Chapter 3",
                "Chapter 4",
                "Chapter 5",
                "Chapter 6",
                "Chapter 7",
                "Chapter 1",
            ],
            issue_suffix: 0x6519,
        },
    )
    .await;
}

async fn json_query(
    admin: &tokio_postgres::Client,
    sql: &str,
    params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
) -> serde_json::Value {
    serde_json::from_str(
        &admin
            .query_one(sql, params)
            .await
            .unwrap()
            .get::<_, String>(0),
    )
    .unwrap()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn applied_chapter_reorder_keeps_other_volume_scope_and_removed_sibling_storage_keys() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let scope = seed_project(&store, &hex_suffix(0x6520)).await;
    let volume_a = titled_volume(&store, &scope, &hex_suffix(0x6521), "Volume A", 1).await;
    let volume_b = titled_volume(&store, &scope, &hex_suffix(0x6522), "Volume B", 2).await;
    let chapter_ids = apply_live_chapters(
        &store, &scope, &volume_a, /*first_suffix*/ 0x6523, /*count*/ 4,
        /*first_tree_revision*/ 3,
    )
    .await;
    let other_chapter = titled_chapter(
        &store,
        &scope,
        &hex_suffix(0x6527),
        &volume_b,
        "Other Volume Chapter",
        /*expected_tree_revision*/ 7,
    )
    .await;
    settle_delete_chapter(
        &store,
        &scope,
        &hex_suffix(0x6528),
        &chapter_ids[1],
        /*expected_tree_revision*/ 8,
    )
    .await;
    let other_scope = seed_project(&store, &hex_suffix(0x6540)).await;
    let other_volume =
        titled_volume(&store, &other_scope, &hex_suffix(0x6541), "Volume A", 1).await;
    let other_ids = apply_live_chapters(
        &store,
        &other_scope,
        &other_volume,
        /*first_suffix*/ 0x6542,
        /*count*/ 2,
        /*first_tree_revision*/ 2,
    )
    .await;
    let current_before = open_project(&store, &scope)
        .await
        .unwrap()
        .expect("the Project remains in exact Scope")
        .current_chapter_id;
    let command = issue_update(
        &store,
        &scope,
        &hex_suffix(0x6529),
        &chapter_ids[3],
        "Chapter 4",
        /*order*/ 1,
        /*expected_tree_revision*/ 9,
    )
    .await;
    let updated = update_chapter(&store, &command).await.unwrap();
    assert_eq!(
        applied(&updated).0,
        UpdateChapterApplied {
            title: "Chapter 4".to_owned(),
            order: 1,
            tree_revision: 10,
        }
    );
    let GetManuscriptTree::Found(tree) = get_manuscript_tree(&store, &scope).await.unwrap() else {
        panic!("the Project still has a Canonical Query");
    };
    assert_eq!(
        tree.volumes,
        vec![
            VolumeNode {
                volume_id: VolumeId::new(volume_a.clone()),
                title: "Volume A".to_owned(),
                order: 1,
                chapters: chapter_nodes(
                    &[
                        chapter_ids[3].clone(),
                        chapter_ids[0].clone(),
                        chapter_ids[2].clone()
                    ],
                    &["Chapter 4", "Chapter 1", "Chapter 3"],
                ),
            },
            VolumeNode {
                volume_id: VolumeId::new(volume_b.clone()),
                title: "Volume B".to_owned(),
                order: 2,
                chapters: chapter_nodes(
                    std::slice::from_ref(&other_chapter),
                    &["Other Volume Chapter"],
                ),
            },
        ]
    );
    assert_eq!(
        open_project(&store, &scope)
            .await
            .unwrap()
            .expect("the Project remains in exact Scope")
            .current_chapter_id,
        current_before
    );
    let GetManuscriptTree::Found(other_tree) =
        get_manuscript_tree(&store, &other_scope).await.unwrap()
    else {
        panic!("the other Project still has a Canonical Query");
    };
    assert_eq!(
        other_tree.volumes,
        vec![VolumeNode {
            volume_id: VolumeId::new(other_volume),
            title: "Volume A".to_owned(),
            order: 1,
            chapters: chapter_nodes(&other_ids, &["Chapter 1", "Chapter 2"]),
        }]
    );
    assert_eq!(
        get_manuscript_tree(
            &store,
            &ProjectScope::new(UserId::new(USER_B), scope.project_id.clone()),
        )
        .await
        .unwrap(),
        GetManuscriptTree::Missing
    );
    assert_eq!(
        json_query(
            &admin,
            "SELECT jsonb_build_object(
               'live_a', (
                 SELECT string_agg(c.title || ':' || c.tree_order::text, ',' ORDER BY c.tree_order)
                   FROM storyos.manuscript_objects c
                  WHERE c.project_id = $1::text::uuid AND c.object_kind = 'chapter'
                    AND c.parent_volume_id = $2::text::uuid
                    AND NOT EXISTS (
                      SELECT 1 FROM storyos.chapter_removal_decisions r
                       WHERE r.project_id = c.project_id AND r.chapter_id = c.manuscript_object_id)
               ),
               'removed_a', (
                 SELECT tree_order::text FROM storyos.manuscript_objects
                  WHERE project_id = $1::text::uuid AND manuscript_object_id = $4::text::uuid
               ),
               'live_b', (
                 SELECT string_agg(c.title || ':' || c.tree_order::text, ',' ORDER BY c.tree_order)
                   FROM storyos.manuscript_objects c
                  WHERE c.project_id = $1::text::uuid AND c.object_kind = 'chapter'
                    AND c.parent_volume_id = $3::text::uuid
               )
             )::text",
            &[
                &scope.project_id.as_ref(),
                &volume_a,
                &volume_b,
                &chapter_ids[1],
            ],
        )
        .await,
        serde_json::json!({
            "live_a": "Chapter 4:1,Chapter 1:2,Chapter 3:3",
            "removed_a": "1000002",
            "live_b": "Other Volume Chapter:1",
        })
    );
}
