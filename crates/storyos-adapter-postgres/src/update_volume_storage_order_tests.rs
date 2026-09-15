use super::*;
use crate::delete_volume_tests::apply_delete;
use crate::set_current_chapter_authority_tests::{open_session, undo_named};

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn undo_volume_reorders_then_delete_restores_the_original_tree() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let mut store = PostgresProjectReader {
        challenge_rate_clock_unix_seconds: Some(60),
        ..PostgresProjectReader::new(std::env::var("STORYOS_TEST_DATABASE_URL").unwrap())
    };
    let scope = seed_project(&store, "7060").await;
    let mut volumes = Vec::new();
    for index in 0..4 {
        let suffix = format!("{:04x}", 0x7061 + index);
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
    let chapter_id = apply_chapter(
        &store,
        &scope,
        "7065",
        &volumes[0],
        "Chapter A",
        br#"{"expected_tree_revision":"5","title":"Chapter A"}"#,
        /*expected_tree_revision*/ 5,
    )
    .await;
    let GetManuscriptTree::Found(original) = get_manuscript_tree(&store, &scope).await.unwrap()
    else {
        panic!("the original tree must exist");
    };
    let deleted = apply_delete(
        &store,
        &scope,
        "7066",
        &volumes[1],
        /*expected_tree_revision*/ 6,
        br#"{"expected_tree_revision":"6"}"#,
    )
    .await;
    let mut prior_trees = vec![original];
    let mut forward_sequences = vec![deleted.authority.unwrap().author_action_sequence];
    for (index, order, title) in [(0, 1, "First move"), (1, 3, "Second move")] {
        store.challenge_rate_clock_unix_seconds = Some(120 + index as i64 * 60);
        let GetManuscriptTree::Found(prior) = get_manuscript_tree(&store, &scope).await.unwrap()
        else {
            panic!("the live tree must exist");
        };
        prior_trees.push(prior);
        let revision = 7 + index;
        let suffix = format!("{:04x}", 0x7067 + index);
        let bytes = format!(
            r#"{{"expected_tree_revision":"{revision}","order":"{order}","title":"{title}"}}"#
        )
        .into_bytes();
        let updated = apply_update(
            &store,
            &scope,
            &suffix,
            UpdateFixture {
                volume_id: &volumes[3],
                title,
                order,
                expected_tree_revision: revision,
                bytes: &bytes,
            },
        )
        .await;
        forward_sequences.push(updated.authority.unwrap().author_action_sequence);
    }
    let editor_session_id = open_session(&store, &scope, "7069").await;
    let OpenChapter::Found(opened) = open_chapter(&store, &scope, &ChapterId::new(chapter_id))
        .await
        .unwrap()
    else {
        panic!("the retained Chapter must open");
    };
    for (index, (sequence, prior)) in forward_sequences
        .into_iter()
        .zip(prior_trees)
        .rev()
        .enumerate()
    {
        store.challenge_rate_clock_unix_seconds = Some(240 + index as i64 * 60);
        let suffix = format!("{:04x}", 0x706a + index);
        let undone = undo_named(
            &store,
            &scope,
            &editor_session_id,
            sequence,
            opened.chapter.revision_id.as_ref(),
            &suffix,
        )
        .await;
        let UndoLatestAuthorActionSettlementEffect::CompensatedStructure {
            source_sequence,
            snapshot_id,
            ..
        } = undone.effect
        else {
            panic!("Undo must compensate the structural action");
        };
        let GetManuscriptTree::Found(restored) = get_manuscript_tree(&store, &scope).await.unwrap()
        else {
            panic!("the compensated tree must exist");
        };
        assert_eq!(source_sequence, sequence);
        assert_eq!(restored.volumes, prior.volumes);
        assert_eq!(restored.tree_revision, prior.tree_revision);
        assert_eq!(restored.snapshot.snapshot_id, snapshot_id);
    }
}
