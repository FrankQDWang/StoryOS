//! The Author Undo of Volume reorders and a Volume deletion in one Project, newest first.

use storyos_application::{
    ChapterId, GetManuscriptTree, OpenChapter, UndoApplied, UndoRecords, get_manuscript_tree,
    open_chapter,
};
use storyos_core::TransitionOutcome;

use crate::command_sequence::tests::{
    applied, seed_project, stores, titled_chapter, titled_volume, undo_named,
};
use crate::delete_structure_tests::settle_delete_volume;
use crate::set_current_chapter_authority_tests::open_session;
use crate::update_structure_tests::settle_update_volume;

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn undo_volume_reorders_then_delete_restores_the_original_tree() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (mut store, _admin) = stores().await;
    store.challenge_rate_clock_unix_seconds = Some(60);
    let scope = seed_project(&store, "7060").await;
    let mut volumes = Vec::new();
    for index in 0..4 {
        let suffix = format!("{:04x}", 0x7061 + index);
        let title = format!("Volume {}", index + 1);
        volumes.push(titled_volume(&store, &scope, &suffix, &title, index + 1).await);
    }
    let chapter_id = titled_chapter(
        &store,
        &scope,
        "7065",
        &volumes[0],
        "Chapter A",
        /*expected_tree_revision*/ 5,
    )
    .await;
    let GetManuscriptTree::Found(original) = get_manuscript_tree(&store, &scope).await.unwrap()
    else {
        panic!("the original tree must exist");
    };
    let deleted = settle_delete_volume(
        &store,
        &scope,
        "7066",
        &volumes[1],
        /*expected_tree_revision*/ 6,
    )
    .await;
    let mut prior_trees = vec![original];
    let mut forward_sequences = vec![applied(&deleted).1.author_action_sequence];
    for (index, order, title) in [(0, 1, "First move"), (1, 3, "Second move")] {
        store.challenge_rate_clock_unix_seconds = Some(120 + index as i64 * 60);
        let GetManuscriptTree::Found(prior) = get_manuscript_tree(&store, &scope).await.unwrap()
        else {
            panic!("the live tree must exist");
        };
        prior_trees.push(prior);
        let revision = 7 + index;
        let suffix = format!("{:04x}", 0x7067 + index);
        let updated =
            settle_update_volume(&store, &scope, &suffix, &volumes[3], title, order, revision)
                .await;
        forward_sequences.push(applied(&updated).1.author_action_sequence);
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
        let TransitionOutcome::Applied(UndoApplied {
            source_sequence,
            records: UndoRecords::Structure { snapshot_id, .. },
            ..
        }) = undone.outcome
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
