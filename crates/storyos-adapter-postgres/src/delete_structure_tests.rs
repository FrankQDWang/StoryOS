//! The classification, effect rows, and Author Undo Compensation of Delete Volume and Delete
//! Chapter, as one table over the two sibling kinds. `command_sequence_tests/` owns replay,
//! rollback, and evidence.

use storyos_application::{
    ChapterId, DeleteChapterInput, DeleteChapterSettlement, DeleteVolumeInput,
    DeleteVolumeSettlement, GetManuscriptTree, OpenChapter, ProjectScope, StructureAuthority,
    UndoApplied, UndoRecords, UserId, VolumeId, get_manuscript_tree, open_chapter, open_project,
};
use storyos_core::{DeleteChapterCurrent, ReceiptResult, TransitionOutcome};

use crate::PostgresProjectReader;
use crate::command_sequence::tests::{
    DELETE_CHAPTER, DELETE_VOLUME, applied, delete_chapter, delete_volume, named_call,
    outcome_code, seed_project, stores, undo_named,
};
use crate::create_structure_tests::archive;
use crate::set_current_chapter_authority_tests::open_session;
use crate::update_structure_tests::{Seeded, Sibling, tree};

const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const MISSING: &str = "018f0000-0000-7001-8000-00000000ffff";

/// Settles one Delete Volume whose ids derive from the four hex digits `suffix`.
pub(crate) async fn settle_delete_volume(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
    volume_id: &str,
    expected_tree_revision: u64,
) -> DeleteVolumeSettlement {
    let input = DeleteVolumeInput {
        volume_id: VolumeId::new(volume_id),
        expected_tree_revision,
    };
    let call = named_call(store, scope, suffix, &DELETE_VOLUME, input).await;
    delete_volume(store, &call).await.unwrap()
}

/// Settles one Delete Chapter whose ids derive from the four hex digits `suffix`.
pub(crate) async fn settle_delete_chapter(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
    chapter_id: &str,
    expected_tree_revision: u64,
) -> DeleteChapterSettlement {
    let input = DeleteChapterInput {
        chapter_id: ChapterId::new(chapter_id),
        expected_tree_revision,
    };
    let call = named_call(store, scope, suffix, &DELETE_CHAPTER, input).await;
    delete_chapter(store, &call).await.unwrap()
}

/// The applied effect of one deletion: the tree revision, the Volume of the effect, and the
/// Current Chapter disposition of a Chapter deletion.
type Effect = (u64, String, Option<DeleteChapterCurrent>);

/// The observable result of one deletion.
struct Deleted {
    code: (ReceiptResult, Option<&'static str>),
    applied: Option<(Effect, StructureAuthority)>,
}

async fn delete(
    kind: Sibling,
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
    id: &str,
    expected_tree_revision: u64,
) -> Deleted {
    match kind {
        Sibling::Volume => {
            let settled =
                settle_delete_volume(store, scope, suffix, id, expected_tree_revision).await;
            Deleted {
                code: outcome_code(&settled.outcome),
                applied: matches!(settled.outcome, TransitionOutcome::Applied(_)).then(|| {
                    let (effect, authority) = applied(&settled);
                    ((effect.tree_revision, effect.volume_id, None), authority)
                }),
            }
        }
        Sibling::Chapter => {
            let settled =
                settle_delete_chapter(store, scope, suffix, id, expected_tree_revision).await;
            Deleted {
                code: outcome_code(&settled.outcome),
                applied: matches!(settled.outcome, TransitionOutcome::Applied(_)).then(|| {
                    let (effect, authority) = applied(&settled);
                    let current = Some(effect.current);
                    ((effect.tree_revision, effect.volume_id, current), authority)
                }),
            }
        }
    }
}

/// The removal decisions of the Project.
async fn removal_decisions(admin: &tokio_postgres::Client, scope: &ProjectScope) -> Vec<String> {
    admin
        .query(
            "SELECT volume_id::text FROM storyos.volume_removal_decisions
              WHERE project_id = $1::text::uuid
             UNION ALL
             SELECT chapter_id::text FROM storyos.chapter_removal_decisions
              WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(/*idx*/ 0))
        .collect()
}

async fn current_chapter(store: &PostgresProjectReader, scope: &ProjectScope) -> Option<ChapterId> {
    open_project(store, scope)
        .await
        .unwrap()
        .expect("the Project is in exact Scope")
        .current_chapter_id
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_delete_classifies_its_outcomes_and_writes_its_effect_rows() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    for (kind, base) in [(Sibling::Volume, 0x96d0), (Sibling::Chapter, 0x96e0)] {
        let suffix = |offset: u16| format!("{:04x}", base + offset);
        let scope = seed_project(&store, &suffix(0)).await;
        // Volume A holds Chapter A. Volume B and Chapter B are empty.
        let Seeded { a, b, chapter } = kind.seed(&store, &scope, base).await;
        let mut codes = Vec::new();
        if kind == Sibling::Volume {
            codes.push(delete(kind, &store, &scope, &suffix(4), &a, 4).await.code);
        }
        let first = delete(kind, &store, &scope, &suffix(5), &b, 4).await;
        let Some((effect, authority)) = first.applied else {
            panic!("the {kind:?} deletion must apply");
        };
        let live = tree(&store, &scope).await;
        let volume_id = live.volumes[0].volume_id.as_ref().to_owned();
        let current = (kind == Sibling::Chapter).then_some(DeleteChapterCurrent::PreserveExisting);
        assert_eq!(
            effect,
            (
                5,
                if kind == Sibling::Volume {
                    b.clone()
                } else {
                    volume_id
                },
                current
            )
        );
        assert_eq!(
            (kind.nodes(&live), &live.snapshot.snapshot_id),
            (
                vec![(a.clone(), kind.title("A"), 1)],
                &authority.snapshot_id
            )
        );
        assert_eq!(removal_decisions(&admin, &scope).await, vec![b.clone()]);
        assert_eq!(
            current_chapter(&store, &scope).await,
            Some(ChapterId::new(chapter.clone()))
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
        if kind == Sibling::Chapter {
            assert_eq!(
                open_chapter(&store, &scope, &ChapterId::new(b.clone()))
                    .await
                    .unwrap(),
                OpenChapter::Missing
            );
        }

        for (offset, id, expected_tree_revision) in
            [(6, b.as_str(), 5), (7, a.as_str(), 4), (8, MISSING, 5)]
        {
            let deleted = delete(
                kind,
                &store,
                &scope,
                &suffix(offset),
                id,
                expected_tree_revision,
            )
            .await;
            codes.push(deleted.code);
        }
        archive(&admin, &scope).await;
        codes.push(delete(kind, &store, &scope, &suffix(9), &a, 5).await.code);
        let mut expected = vec![
            (ReceiptResult::NoEffect, Some("already_removed")),
            (ReceiptResult::Conflicted, Some("stale_tree_revision")),
            (ReceiptResult::Refused, Some(kind.join_refusal())),
            (ReceiptResult::Refused, Some("archived_project")),
        ];
        if kind == Sibling::Volume {
            expected.insert(0, (ReceiptResult::Refused, Some("nonempty_volume")));
        }
        assert_eq!(codes, expected, "{kind:?}");
        assert_eq!(tree(&store, &scope).await.volumes, live.volumes);
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_of_each_delete_restores_the_prior_identity_and_place() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    // The Volume row deletes the empty Volume B. The Chapter row deletes the current Chapter A,
    // and Chapter B becomes current.
    for (kind, base) in [(Sibling::Volume, 0x96f0), (Sibling::Chapter, 0x9700)] {
        let suffix = |offset: u16| format!("{:04x}", base + offset);
        let scope = seed_project(&store, &suffix(0)).await;
        let Seeded { a, b, chapter } = kind.seed(&store, &scope, base).await;
        let (removed, kept_chapter) = match kind {
            Sibling::Volume => (b.clone(), chapter.clone()),
            Sibling::Chapter => (a.clone(), b.clone()),
        };
        let prior = tree(&store, &scope).await;
        let deleted = delete(kind, &store, &scope, &suffix(4), &removed, 4).await;
        let Some(((_, _, current), authority)) = deleted.applied else {
            panic!("the {kind:?} deletion must apply");
        };
        let successor = DeleteChapterCurrent::SelectSuccessor {
            chapter_id: b.clone(),
        };
        assert_eq!(current, (kind == Sibling::Chapter).then_some(successor));
        let editor_session_id = open_session(&store, &scope, &suffix(5)).await;
        let OpenChapter::Found(opened) =
            open_chapter(&store, &scope, &ChapterId::new(kept_chapter))
                .await
                .unwrap()
        else {
            panic!("the kept Chapter must open");
        };
        let undone = undo_named(
            &store,
            &scope,
            &editor_session_id,
            authority.author_action_sequence,
            opened.chapter.revision_id.as_ref(),
            &suffix(6),
        )
        .await;
        let TransitionOutcome::Applied(UndoApplied {
            source_sequence,
            records: UndoRecords::Structure { snapshot_id, .. },
            ..
        }) = undone.outcome
        else {
            panic!("the {kind:?} deletion Undo must write structure Compensation");
        };
        assert_eq!(source_sequence, authority.author_action_sequence);
        let restored = tree(&store, &scope).await;
        assert_eq!(
            (
                restored.tree_revision,
                restored.snapshot.snapshot_id,
                restored.volumes
            ),
            (prior.tree_revision, snapshot_id, prior.volumes),
            "{kind:?}"
        );
        assert_eq!(
            current_chapter(&store, &scope).await,
            Some(ChapterId::new(chapter)),
            "{kind:?}"
        );
        assert_eq!(
            removal_decisions(&admin, &scope).await,
            Vec::<String>::new()
        );
        // The Compensation is no new create command.
        let counts = admin
            .query_one(
                "SELECT (SELECT count(*) FROM storyos.author_action_entries
                          WHERE project_id = $1::text::uuid AND disposition = 'compensation'
                            AND compensated_source_sequence = $2::text::numeric),
                        (SELECT count(*) FROM storyos.domain_receipts
                          WHERE receipt_id = $3::text::uuid
                            AND command_kind IN ('createVolume', 'createChapter'))",
                &[
                    &scope.project_id.as_ref(),
                    &source_sequence.to_string(),
                    &undone.ids.receipt_id,
                ],
            )
            .await
            .unwrap();
        assert_eq!([0, 1].map(|index| counts.get::<_, i64>(index)), [1, 0]);
    }
}
