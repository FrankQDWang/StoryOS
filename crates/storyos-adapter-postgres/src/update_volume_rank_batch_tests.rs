use super::*;
use crate::delete_volume_tests::apply_delete;
use storyos_application::UpdateVolumeError;

async fn durable_project_rows(
    admin: &tokio_postgres::Client,
    scope: &ProjectScope,
) -> Vec<serde_json::Value> {
    let mut rows = Vec::new();
    for table in [
        "projects",
        "manuscript_objects",
        "domain_receipts",
        "author_command_admissions",
        "project_activity_events",
        "project_activity_event_payloads",
        "authoritative_commits",
        "author_action_entries",
        "project_snapshots",
        "command_idempotency",
    ] {
        let sql = format!(
            "SELECT jsonb_agg(to_jsonb(row) ORDER BY to_jsonb(row)::text)::text
               FROM storyos.{table} AS row
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid"
        );
        let value: Option<String> = admin
            .query_one(
                &sql,
                &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
            )
            .await
            .unwrap()
            .get(0);
        rows.push(serde_json::from_str(value.as_deref().unwrap_or("null")).unwrap());
    }
    rows
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn volume_rank_batch_keeps_sparse_order_tombstones_and_atomic_settlement() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let store = PostgresProjectReader::new(std::env::var("STORYOS_TEST_DATABASE_URL").unwrap());
    let (admin, connection) = tokio_postgres::connect(
        &std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL").unwrap(),
        NoTls,
    )
    .await
    .unwrap();
    tokio::spawn(async move {
        connection.await.unwrap();
    });
    let scope = seed_project(&store, "6870").await;
    let mut volumes = Vec::new();
    for index in 0..4 {
        let suffix = format!("{:04x}", 0x6871 + index);
        let title = format!("Volume {}", index + 1);
        let revision = index + 1;
        let bytes =
            format!(r#"{{"expected_tree_revision":"{revision}","title":"{title}"}}"#).into_bytes();
        let digest = format!(
            "sha256:storyos.command.createVolume.jcs.v1:{}",
            crate::author_edit::sha256_hex(&bytes)
        );
        volumes
            .push(apply_volume(&store, &scope, &suffix, &title, &bytes, &digest, revision).await);
    }
    apply_delete(
        &store,
        &scope,
        "6875",
        &volumes[1],
        /*expected_tree_revision*/ 5,
        br#"{"expected_tree_revision":"5"}"#,
    )
    .await;
    admin
        .batch_execute(
            "CREATE TABLE public.storyos_issue_687_counts (
           object_calls bigint NOT NULL DEFAULT 0, project_calls bigint NOT NULL DEFAULT 0,
           rank_calls bigint NOT NULL DEFAULT 0, rank_rows bigint NOT NULL DEFAULT 0,
           fail_rank boolean NOT NULL DEFAULT false);
         INSERT INTO public.storyos_issue_687_counts DEFAULT VALUES;
         CREATE FUNCTION public.storyos_issue_687_count_objects()
         RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER
         SET search_path = pg_catalog, public AS $$
         DECLARE changed bigint;
         BEGIN
           SELECT count(*) INTO changed FROM new_rows n JOIN old_rows o
             USING (owner_user_id, project_id, manuscript_object_id)
            WHERE n.object_kind = 'volume' AND n.tree_order IS DISTINCT FROM o.tree_order
              AND n.tree_order <= 1000000;
           IF changed > 0 AND (SELECT fail_rank FROM public.storyos_issue_687_counts) THEN
             RAISE EXCEPTION 'injected Volume rank failure';
           END IF;
           UPDATE public.storyos_issue_687_counts
              SET object_calls = object_calls + 1,
                  rank_calls = rank_calls + CASE WHEN changed > 0 THEN 1 ELSE 0 END,
                  rank_rows = rank_rows + changed;
           RETURN NULL;
         END $$;
         CREATE FUNCTION public.storyos_issue_687_count_projects()
         RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER
         SET search_path = pg_catalog, public AS $$
         BEGIN
           UPDATE public.storyos_issue_687_counts SET project_calls = project_calls + 1;
           RETURN NULL;
         END $$;
         CREATE TRIGGER storyos_issue_687_objects AFTER UPDATE ON storyos.manuscript_objects
         REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows FOR EACH STATEMENT
         EXECUTE FUNCTION public.storyos_issue_687_count_objects();
         CREATE TRIGGER storyos_issue_687_projects AFTER UPDATE ON storyos.projects
         FOR EACH STATEMENT EXECUTE FUNCTION public.storyos_issue_687_count_projects();",
        )
        .await
        .unwrap();

    let bytes = br#"{"expected_tree_revision":"6","order":"1","title":"Rejected title"}"#;
    let digest = format!(
        "sha256:storyos.command.updateVolume.jcs.v1:{}",
        crate::author_edit::sha256_hex(bytes)
    );
    let issue = update_issue(&scope, "6879", &digest);
    issue_project_command_challenge(&store, &issue)
        .await
        .unwrap();
    let command = update_command(
        issue.binding,
        &issue.nonce_digest,
        "6879",
        UpdateFixture {
            volume_id: &volumes[3],
            title: "Rejected title",
            order: 1,
            expected_tree_revision: 6,
            bytes,
        },
    );
    let before = durable_project_rows(&admin, &scope).await;
    admin
        .execute(
            "UPDATE public.storyos_issue_687_counts SET fail_rank = true",
            &[],
        )
        .await
        .unwrap();
    let result = update_volume(&store, &command).await;
    match result {
        Err(UpdateVolumeError::Unavailable(error)) => {
            assert!(format!("{error:?}").contains("injected Volume rank failure"));
        }
        other => panic!("expected the injected rank failure, got {other:?}"),
    }
    assert_eq!(durable_project_rows(&admin, &scope).await, before);
    admin
        .execute(
            "UPDATE public.storyos_issue_687_counts SET fail_rank = false",
            &[],
        )
        .await
        .unwrap();

    let mut first_command = None;
    let mut first_settlement = None;
    for (index, order, title, expected_ids, expected_titles, expected_calls) in [
        (
            0,
            1,
            "Volume 4",
            [3, 0, 2],
            ["Volume 4", "Volume 1", "Volume 3"],
            (4, 1, 3),
        ),
        (
            1,
            1,
            "Renamed 4",
            [3, 0, 2],
            ["Renamed 4", "Volume 1", "Volume 3"],
            (2, 0, 0),
        ),
    ] {
        admin
            .execute(
                "UPDATE public.storyos_issue_687_counts
            SET object_calls = 0, project_calls = 0, rank_calls = 0, rank_rows = 0",
                &[],
            )
            .await
            .unwrap();
        let revision = 6 + index;
        let suffix = format!("{:04x}", 0x6876 + index);
        let bytes = format!(
            r#"{{"expected_tree_revision":"{revision}","order":"{order}","title":"{title}"}}"#
        )
        .into_bytes();
        let digest = format!(
            "sha256:storyos.command.updateVolume.jcs.v1:{}",
            crate::author_edit::sha256_hex(&bytes)
        );
        let issue = update_issue(&scope, &suffix, &digest);
        issue_project_command_challenge(&store, &issue)
            .await
            .unwrap();
        let command = update_command(
            issue.binding,
            &issue.nonce_digest,
            &suffix,
            UpdateFixture {
                volume_id: &volumes[3],
                title,
                order,
                expected_tree_revision: revision,
                bytes: &bytes,
            },
        );
        let settlement = update_volume(&store, &command).await.unwrap();
        assert_eq!(
            settlement.effect,
            UpdateVolumeSettlementEffect::Applied {
                title: title.to_owned(),
                order,
                tree_revision: revision + 1,
            }
        );
        let counts = admin
            .query_one(
                "SELECT object_calls + project_calls, rank_calls, rank_rows
               FROM public.storyos_issue_687_counts",
                &[],
            )
            .await
            .unwrap();
        assert_eq!(
            (
                counts.get::<_, i64>(0),
                counts.get::<_, i64>(1),
                counts.get::<_, i64>(2)
            ),
            expected_calls
        );
        let GetManuscriptTree::Found(tree) = get_manuscript_tree(&store, &scope).await.unwrap()
        else {
            panic!("the Project still has a canonical tree")
        };
        assert_eq!(
            tree.volumes,
            expected_ids
                .into_iter()
                .zip(expected_titles)
                .enumerate()
                .map(|(index, (id, title))| VolumeNode {
                    volume_id: VolumeId::new(volumes[id].clone()),
                    title: title.to_owned(),
                    order: index as u64 + 1,
                    chapters: Vec::new(),
                })
                .collect::<Vec<_>>()
        );
        let authority = settlement.authority.as_ref().unwrap();
        assert_eq!(
            (
                tree.tree_revision,
                tree.snapshot.snapshot_id,
                tree.snapshot.project_activity_position
            ),
            (
                revision + 1,
                authority.snapshot_id.clone(),
                settlement.project_activity_position
            )
        );
        let removed_key: i64 = admin
            .query_one(
                "SELECT tree_order FROM storyos.manuscript_objects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND manuscript_object_id = $3::text::uuid",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &volumes[1],
                ],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(removed_key, 1_000_002);
        if first_command.is_none() {
            first_command = Some(command);
            first_settlement = Some(settlement);
        }
    }
    assert_eq!(
        update_volume(&store, &first_command.unwrap())
            .await
            .unwrap(),
        first_settlement.unwrap()
    );

    admin
        .batch_execute(
            "DROP TRIGGER storyos_issue_687_objects ON storyos.manuscript_objects;
         DROP TRIGGER storyos_issue_687_projects ON storyos.projects;
         DROP FUNCTION public.storyos_issue_687_count_objects();
         DROP FUNCTION public.storyos_issue_687_count_projects();
         DROP TABLE public.storyos_issue_687_counts;",
        )
        .await
        .unwrap();
}
