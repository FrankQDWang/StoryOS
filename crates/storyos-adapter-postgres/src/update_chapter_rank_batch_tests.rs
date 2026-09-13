use super::*;
use crate::delete_chapter_tests::apply_delete;
use crate::update_chapter_tests::{
    USER_B, UpdateFixture, apply_chapter, apply_volume, named_issue, seed_project, update_command,
    update_issue,
};
use storyos_application::{
    AuthorCommandAdmissionIds, CanonicalManuscriptTree, ChapterId, ChapterNode,
    CreateVolumeCommand, CreateVolumeSettlementEffect, EditorClientBinding, GetManuscriptTree,
    ProjectScope, UpdateChapterCommand, UpdateChapterError, UpdateChapterSettlement,
    UpdateChapterSettlementEffect, UserId, VolumeId, VolumeNode, create_volume,
    get_manuscript_tree, issue_project_command_challenge, open_project, update_chapter,
};
use tokio_postgres::NoTls;

fn hex_suffix(value: u16) -> String {
    format!("{value:04x}")
}

fn titled_structure_bytes(title: &str, expected_tree_revision: u64) -> Vec<u8> {
    format!(r#"{{"expected_tree_revision":"{expected_tree_revision}","title":"{title}"}}"#)
        .into_bytes()
}

fn update_bytes(title: &str, order: u64, expected_tree_revision: u64) -> Vec<u8> {
    format!(
        r#"{{"expected_tree_revision":"{expected_tree_revision}","order":"{order}","title":"{title}"}}"#
    )
    .into_bytes()
}

fn command_digest(profile: &str, bytes: &[u8]) -> String {
    format!("sha256:{profile}:{}", crate::author_edit::sha256_hex(bytes))
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
        let bytes = titled_structure_bytes(&title, first_tree_revision + index);
        chapter_ids.push(
            apply_chapter(
                store,
                scope,
                &hex_suffix(first_suffix + index as u16),
                volume_id,
                &title,
                &bytes,
                first_tree_revision + index,
            )
            .await,
        );
    }
    chapter_ids
}

async fn apply_named_volume(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
    title: &str,
    expected_tree_revision: u64,
) -> String {
    let bytes = titled_structure_bytes(title, expected_tree_revision);
    let digest = command_digest("storyos.command.createVolume.jcs.v1", &bytes);
    let issue = named_issue(
        scope,
        suffix,
        "POST",
        "/api/v1/projects/{project_id}/volumes",
        "storyos.command.create-volume.request.v1",
        "createVolume",
        &digest,
    );
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    let settlement = create_volume(
        store,
        &CreateVolumeCommand {
            project_scope: scope.clone(),
            client_binding: EditorClientBinding {
                binding_ref: issue.binding.client_session_binding_digest.clone(),
                session_generation: issue.binding.client_session_generation,
                client_contract_revision: issue.binding.client_contract_revision.clone(),
                security_policy_revision: issue.binding.security_policy_revision.clone(),
            },
            challenge_binding: issue.binding,
            nonce_digest: issue.nonce_digest,
            canonical_command_bytes: bytes,
            correlation_id: format!("018f0000-0000-7001-8000-00000000{suffix}"),
            title: title.to_owned(),
            expected_tree_revision,
            ids: AuthorCommandAdmissionIds {
                command_id: format!("018f0000-0000-7001-8000-00000001{suffix}"),
                author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{suffix}"),
                receipt_id: format!("018f0000-0000-7001-8000-00000003{suffix}"),
            },
        },
    )
    .await
    .unwrap();
    let CreateVolumeSettlementEffect::Applied { volume_id, .. } = settlement.effect else {
        panic!("{title} must apply");
    };
    volume_id
}

async fn issue_update(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
    fixture: UpdateFixture<'_>,
) -> UpdateChapterCommand {
    let digest = command_digest("storyos.command.updateChapter.jcs.v1", fixture.bytes);
    let issue = update_issue(scope, suffix, &digest);
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    update_command(issue.binding, &issue.nonce_digest, suffix, fixture)
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

async fn drop_chapter_tree_sql_probes(admin: &tokio_postgres::Client) {
    admin
        .batch_execute(
            "DROP TRIGGER IF EXISTS storyos_issue_650_count_manuscript_objects ON storyos.manuscript_objects;
             DROP TRIGGER IF EXISTS storyos_issue_650_count_projects ON storyos.projects;
             DROP TRIGGER IF EXISTS storyos_issue_650_fail_rank_write ON storyos.manuscript_objects;
             DROP FUNCTION IF EXISTS public.storyos_issue_650_count_manuscript_objects();
             DROP FUNCTION IF EXISTS public.storyos_issue_650_count_projects();
             DROP FUNCTION IF EXISTS public.storyos_issue_650_fail_rank_write();
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
    let authority = settlement
        .authority
        .as_ref()
        .expect("Applied Update Chapter must write Structural Authority Settlement");
    assert_eq!(tree.tree_revision, tree_revision);
    assert_eq!(tree.snapshot.snapshot_id, authority.snapshot_id);
    assert_eq!(
        tree.snapshot.project_activity_position,
        settlement.project_activity_position
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
    let volume_id = apply_volume(store, &scope, &hex_suffix(project_suffix + 1)).await;
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
    command: &UpdateChapterCommand,
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
    retry: bool,
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
    let bytes = update_bytes(case.title, case.move_to, case.expected_tree_revision);
    let command = issue_update(
        store,
        &scope,
        &hex_suffix(case.issue_suffix),
        UpdateFixture {
            chapter_id: &chapter_ids[case.move_from],
            title: case.title,
            order: case.move_to,
            expected_tree_revision: case.expected_tree_revision,
            bytes: &bytes,
        },
    )
    .await;
    let ((object_updates, project_updates, rank_writes, rank_rows), settlement) =
        measure_reorder(store, admin, &command).await;
    assert_eq!(
        settlement.effect,
        UpdateChapterSettlementEffect::Applied {
            title: case.title.to_owned(),
            order: case.move_to,
            tree_revision: case.tree_revision,
        }
    );
    assert_eq!(
        (rank_writes, rank_rows, object_updates + project_updates),
        (1, case.chapter_count as i64, 4)
    );
    if case.retry {
        assert_eq!(update_chapter(store, &command).await.unwrap(), settlement);
    }
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
    let store = runtime_store();
    let admin = connect_admin().await;
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
            retry: true,
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
            retry: false,
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
    let store = runtime_store();
    let admin = connect_admin().await;
    let scope = seed_project(&store, &hex_suffix(0x6520)).await;
    let volume_a = apply_volume(&store, &scope, &hex_suffix(0x6521)).await;
    let volume_b = apply_named_volume(
        &store,
        &scope,
        &hex_suffix(0x6522),
        "Volume B",
        /*expected_tree_revision*/ 2,
    )
    .await;
    let chapter_ids = apply_live_chapters(
        &store, &scope, &volume_a, /*first_suffix*/ 0x6523, /*count*/ 4,
        /*first_tree_revision*/ 3,
    )
    .await;
    let other_bytes =
        titled_structure_bytes("Other Volume Chapter", /*expected_tree_revision*/ 7);
    let other_chapter = apply_chapter(
        &store,
        &scope,
        &hex_suffix(0x6527),
        &volume_b,
        "Other Volume Chapter",
        &other_bytes,
        /*expected_tree_revision*/ 7,
    )
    .await;
    apply_delete(
        &store,
        &scope,
        &hex_suffix(0x6528),
        &chapter_ids[1],
        /*expected_tree_revision*/ 8,
        br#"{"expected_tree_revision":"8"}"#,
    )
    .await;
    let other_scope = seed_project(&store, &hex_suffix(0x6540)).await;
    let other_volume = apply_volume(&store, &other_scope, &hex_suffix(0x6541)).await;
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
    let move_bytes = update_bytes(
        "Chapter 4",
        /*order*/ 1,
        /*expected_tree_revision*/ 9,
    );
    let command = issue_update(
        &store,
        &scope,
        &hex_suffix(0x6529),
        UpdateFixture {
            chapter_id: &chapter_ids[3],
            title: "Chapter 4",
            order: 1,
            expected_tree_revision: 9,
            bytes: &move_bytes,
        },
    )
    .await;
    let updated = update_chapter(&store, &command).await.unwrap();
    assert_eq!(
        updated.effect,
        UpdateChapterSettlementEffect::Applied {
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

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn failed_chapter_rank_write_rolls_back_the_entire_command() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let store = runtime_store();
    let admin = connect_admin().await;
    let (scope, volume_id, chapter_ids) = seed_volume_with_chapters(
        &store, /*project_suffix*/ 0x6530, /*chapter_count*/ 3,
    )
    .await;
    let move_bytes = update_bytes(
        "Chapter 3",
        /*order*/ 1,
        /*expected_tree_revision*/ 5,
    );
    let command = issue_update(
        &store,
        &scope,
        &hex_suffix(0x6535),
        UpdateFixture {
            chapter_id: &chapter_ids[2],
            title: "Chapter 3",
            order: 1,
            expected_tree_revision: 5,
            bytes: &move_bytes,
        },
    )
    .await;
    drop_chapter_tree_sql_probes(&admin).await;
    admin
        .batch_execute(
            "CREATE FUNCTION public.storyos_issue_650_fail_rank_write()
             RETURNS trigger LANGUAGE plpgsql AS $$
             BEGIN
               IF NEW.object_kind = 'chapter'
                  AND NEW.tree_order IS DISTINCT FROM OLD.tree_order
                  AND NEW.tree_order <= 1000000 THEN
                 RAISE EXCEPTION 'injected Chapter rank persistence failure';
               END IF;
               RETURN NEW;
             END $$;
             CREATE TRIGGER storyos_issue_650_fail_rank_write
             BEFORE UPDATE ON storyos.manuscript_objects FOR EACH ROW
             EXECUTE FUNCTION public.storyos_issue_650_fail_rank_write();",
        )
        .await
        .unwrap();
    let error = update_chapter(&store, &command)
        .await
        .expect_err("a failed rank write must stop acknowledgement");
    drop_chapter_tree_sql_probes(&admin).await;
    assert!(matches!(error, UpdateChapterError::Unavailable(_)));
    let GetManuscriptTree::Found(tree) = get_manuscript_tree(&store, &scope).await.unwrap() else {
        panic!("the Project still has a Canonical Query");
    };
    assert_eq!(tree.tree_revision, 5);
    assert_eq!(
        tree.volumes,
        vec![VolumeNode {
            volume_id: VolumeId::new(volume_id),
            title: "Volume A".to_owned(),
            order: 1,
            chapters: chapter_nodes(&chapter_ids, &["Chapter 1", "Chapter 2", "Chapter 3"]),
        }]
    );
    assert_eq!(
        json_query(
            &admin,
            "SELECT jsonb_build_object(
               'update_receipts', (SELECT count(*) FROM storyos.domain_receipts
                 WHERE project_id = $1::text::uuid AND command_kind = 'updateChapter'),
               'chapter_updated', (SELECT count(*) FROM storyos.project_activity_event_payloads
                 WHERE project_id = $1::text::uuid AND event_kind = 'chapter_updated'),
               'update_admissions', (SELECT count(*) FROM storyos.author_command_admissions
                 WHERE project_id = $1::text::uuid AND command_kind = 'updateChapter'),
               'settled_idempotency', (SELECT count(*) FROM storyos.command_idempotency
                 WHERE project_id = $1::text::uuid AND command_kind = 'updateChapter'
                   AND outcome_kind = 'settled'),
               'commits', (SELECT count(*) FROM storyos.authoritative_commits
                 WHERE project_id = $1::text::uuid),
               'actions', (SELECT count(*) FROM storyos.author_action_entries
                 WHERE project_id = $1::text::uuid),
               'snapshots', (SELECT count(*) FROM storyos.project_snapshots
                 WHERE project_id = $1::text::uuid)
             )::text",
            &[&scope.project_id.as_ref()],
        )
        .await,
        serde_json::json!({
            "update_receipts": 0,
            "chapter_updated": 0,
            "update_admissions": 0,
            "settled_idempotency": 0,
            "commits": 4,
            "actions": 4,
            "snapshots": 5,
        })
    );
}
