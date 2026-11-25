//! The classification, effect rows, and Author Undo Compensation of Update Volume and Update
//! Chapter, as one table over the two sibling kinds. `command_sequence_tests/` owns replay,
//! rollback, and evidence.

use storyos_application::{
    ChapterId, GetManuscriptTree, OpenChapter, ProjectScope, UndoApplied, UndoRecords, UserId,
    get_manuscript_tree, open_chapter,
};
use storyos_core::{ReceiptResult, TransitionOutcome};

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::{seed_project, stores, undo_named};
use crate::shared_tests::create_structure::current_chapter;
use crate::shared_tests::set_current_chapter_authority::open_session;
use crate::shared_tests::update_structure::{Seeded, Sibling, tree};

const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const MISSING: &str = "018f0000-0000-7001-8000-00000000ffff";

/// The prior title and prior order that the Activity payload of one update records.
async fn prior_fields(
    admin: &crate::statement_cache::Client,
    receipt_id: &str,
) -> (String, String) {
    let row = admin
        .query_one(
            "SELECT payload->>'prior_title', payload->>'prior_order'
               FROM storyos.project_activity_event_payloads
              WHERE receipt_id = $1::text::uuid",
            &[&receipt_id],
        )
        .await
        .unwrap();
    (row.get(/*idx*/ 0), row.get(/*idx*/ 1))
}

/// The current Chapter and every Chapter Head of the Project.
async fn chapter_identity(admin: &crate::statement_cache::Client, scope: &ProjectScope) -> String {
    admin
        .query_one(
            "SELECT current_chapter_id::text || '/' || (
                      SELECT string_agg(manuscript_object_id::text || ':' || current_revision_id::text,
                                        ',' ORDER BY manuscript_object_id)
                        FROM storyos.authoritative_heads
                       WHERE project_id = $1::text::uuid)
               FROM storyos.projects WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_update_classifies_its_outcomes_and_writes_its_effect_rows() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    for (kind, base) in [(Sibling::Volume, 0x9670), (Sibling::Chapter, 0x9680)] {
        let suffix = |offset: u16| format!("{:04x}", base + offset);
        let scope = seed_project(&store, &suffix(0)).await;
        let Seeded { a, b, chapter } = kind.seed(&store, &scope, base).await;
        let identity = chapter_identity(&admin, &scope).await;
        let renamed = kind.title("B");
        let first = kind
            .update(&store, &scope, &suffix(4), &a, &renamed, 2, 4)
            .await;
        let Some((node, tree_revision, authority)) = first.applied else {
            panic!("the {kind:?} update must apply");
        };
        assert_eq!((node, tree_revision), ((a.clone(), renamed.clone(), 2), 5));
        assert_eq!(
            (
                authority.prior_manuscript_tree_revision,
                authority.resulting_manuscript_tree_revision,
                authority.author_action_sequence
            ),
            (4, 5, 4)
        );
        let live = tree(&store, &scope).await;
        assert_eq!(
            (kind.nodes(&live), &live.snapshot.snapshot_id),
            (
                vec![
                    (b.clone(), renamed.clone(), 1),
                    (a.clone(), renamed.clone(), 2)
                ],
                &authority.snapshot_id
            )
        );
        assert_eq!(
            prior_fields(&admin, &first.receipt_id).await,
            (kind.title("A"), "1".to_owned())
        );
        // An update keeps the current Chapter and every Chapter Head.
        assert_eq!(chapter_identity(&admin, &scope).await, identity);
        // The current Chapter reads the new title only when the update renames it.
        let current = current_chapter(&store, &scope, &ChapterId::new(chapter.clone()))
            .await
            .unwrap()
            .expect("the current Chapter identity is unchanged");
        let current_title = match kind {
            Sibling::Volume => "Chapter A".to_owned(),
            Sibling::Chapter => renamed.clone(),
        };
        assert_eq!(
            (current.chapter_id, current.title, current.body),
            (
                ChapterId::new(chapter.clone()),
                current_title,
                String::new()
            )
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

        let mut codes = Vec::new();
        // Each case sends the stale tree revision 4, so a refusal comes before the conflict.
        for (offset, id, order) in [(5, a.as_str(), 2), (6, MISSING, 2), (7, a.as_str(), 3)] {
            let updated = kind
                .update(
                    &store,
                    &scope,
                    &suffix(offset),
                    id,
                    &renamed,
                    order,
                    /*expected_tree_revision*/ 4,
                )
                .await;
            codes.push(updated.code);
        }
        assert_eq!(tree(&store, &scope).await.volumes, live.volumes);
        assert_eq!(
            codes,
            [
                (ReceiptResult::Conflicted, Some("stale_tree_revision")),
                (ReceiptResult::Refused, Some(kind.join_refusal())),
                (ReceiptResult::Refused, Some("invalid_order")),
            ],
            "{kind:?}"
        );
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_of_each_update_restores_the_prior_title_and_live_sibling_place() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    // A sibling move first changes the live order of A. The last Activity of A still says 1.
    for (kind, base, move_sibling_first) in [
        (Sibling::Volume, 0x9690, true),
        (Sibling::Volume, 0x96a0, false),
        (Sibling::Chapter, 0x96b0, true),
        (Sibling::Chapter, 0x96c0, false),
    ] {
        let suffix = |offset: u16| format!("{:04x}", base + offset);
        let scope = seed_project(&store, &suffix(0)).await;
        let Seeded { a, b, chapter } = kind.seed(&store, &scope, base).await;
        let mut expected_tree_revision = 4;
        let mut forward_updates = 1;
        if move_sibling_first {
            kind.update(&store, &scope, &suffix(4), &b, &kind.title("B"), 1, 4)
                .await;
            expected_tree_revision = 5;
            forward_updates = 2;
        }
        let prior = tree(&store, &scope).await;
        let updated = kind
            .update(
                &store,
                &scope,
                &suffix(5),
                &a,
                "Renamed A",
                2,
                expected_tree_revision,
            )
            .await;
        let Some((_, _, authority)) = updated.applied else {
            panic!("the {kind:?} update must apply");
        };
        assert_eq!(
            kind.nodes(&tree(&store, &scope).await),
            vec![
                (b.clone(), kind.title("B"), 1),
                (a.clone(), "Renamed A".to_owned(), 2)
            ]
        );
        let editor_session_id = open_session(&store, &scope, &suffix(6)).await;
        let OpenChapter::Found(opened) =
            open_chapter(&store, &scope, &ChapterId::new(chapter.clone()))
                .await
                .unwrap()
        else {
            panic!("the seeded Chapter must open");
        };
        let undone = undo_named(
            &store,
            &scope,
            &editor_session_id,
            authority.author_action_sequence,
            opened.chapter.revision_id.as_ref(),
            &suffix(7),
        )
        .await;
        let TransitionOutcome::Applied(UndoApplied {
            source_sequence,
            records: UndoRecords::Structure { snapshot_id, .. },
            ..
        }) = undone.outcome
        else {
            panic!("the {kind:?} update Undo must write structure Compensation");
        };
        assert_eq!(source_sequence, authority.author_action_sequence);
        let restored = tree(&store, &scope).await;
        assert_eq!(
            (
                restored.tree_revision,
                &restored.snapshot.snapshot_id,
                kind.nodes(&restored)
            ),
            (prior.tree_revision, &snapshot_id, kind.nodes(&prior)),
            "{kind:?}"
        );
        let prior_order = if move_sibling_first { "2" } else { "1" };
        assert_eq!(
            prior_fields(&admin, &updated.receipt_id).await,
            (kind.title("A"), prior_order.to_owned())
        );
        let counts = admin
            .query_one(
                "SELECT (SELECT count(*) FROM storyos.author_action_entries
                          WHERE project_id = $1::text::uuid AND disposition = 'compensation'
                            AND compensated_source_sequence = $2::text::numeric),
                        (SELECT count(*) FROM storyos.author_action_entries AS action
                           JOIN storyos.domain_receipts AS receipt USING (receipt_id)
                          WHERE action.project_id = $1::text::uuid
                            AND action.disposition = 'forward'
                            AND receipt.command_kind IN ('updateVolume', 'updateChapter')),
                        (SELECT count(*) FROM storyos.volume_removal_decisions
                          WHERE project_id = $1::text::uuid)
                      + (SELECT count(*) FROM storyos.chapter_removal_decisions
                          WHERE project_id = $1::text::uuid)",
                &[&scope.project_id.as_ref(), &source_sequence.to_string()],
            )
            .await
            .unwrap();
        assert_eq!(
            [0, 1, 2].map(|index| counts.get::<_, i64>(index)),
            [1, forward_updates, 0],
            "{kind:?}"
        );
    }
}
