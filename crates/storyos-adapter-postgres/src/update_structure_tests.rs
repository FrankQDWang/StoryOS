//! The classification, effect rows, and Author Undo Compensation of Update Volume and Update
//! Chapter, as one table over the two sibling kinds. `command_sequence_tests/` owns replay,
//! rollback, and evidence.

use storyos_application::{
    CanonicalManuscriptTree, ChapterId, GetManuscriptTree, OpenChapter, ProjectScope,
    StructureAuthority, UndoApplied, UndoRecords, UpdateChapterInput, UpdateChapterSettlement,
    UpdateVolumeInput, UpdateVolumeSettlement, UserId, VolumeId, get_manuscript_tree, open_chapter,
    open_current_chapter,
};
use storyos_core::{ReceiptResult, TransitionOutcome};

use crate::PostgresProjectReader;
use crate::command_sequence::tests::{
    UPDATE_CHAPTER, UPDATE_VOLUME, applied, named_call, outcome_code, seed_project, stores,
    titled_chapter, titled_volume, undo_named, update_chapter, update_volume,
};
use crate::set_current_chapter_authority_tests::open_session;

const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const MISSING: &str = "018f0000-0000-7001-8000-00000000ffff";

/// Settles one Update Volume whose ids derive from the four hex digits `suffix`.
pub(crate) async fn settle_update_volume(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
    volume_id: &str,
    title: &str,
    order: u64,
    expected_tree_revision: u64,
) -> UpdateVolumeSettlement {
    let input = UpdateVolumeInput {
        volume_id: VolumeId::new(volume_id),
        title: title.to_owned(),
        order,
        expected_tree_revision,
    };
    let call = named_call(store, scope, suffix, &UPDATE_VOLUME, input).await;
    update_volume(store, &call).await.unwrap()
}

/// Settles one Update Chapter whose ids derive from the four hex digits `suffix`.
pub(crate) async fn settle_update_chapter(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
    chapter_id: &str,
    title: &str,
    order: u64,
    expected_tree_revision: u64,
) -> UpdateChapterSettlement {
    let input = UpdateChapterInput {
        chapter_id: ChapterId::new(chapter_id),
        title: title.to_owned(),
        order,
        expected_tree_revision,
    };
    let call = named_call(store, scope, suffix, &UPDATE_CHAPTER, input).await;
    update_chapter(store, &call).await.unwrap()
}

pub(crate) async fn tree(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
) -> CanonicalManuscriptTree {
    let GetManuscriptTree::Found(tree) = get_manuscript_tree(store, scope).await.unwrap() else {
        panic!("the Project has a Canonical Manuscript Tree");
    };
    *tree
}

/// One live sibling: its identity, title, and canonical order.
pub(crate) type Node = (String, String, u64);

/// The structure kind of one table row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Sibling {
    Volume,
    Chapter,
}

/// Two siblings A and B at tree revision 4, and a Chapter that an Editor Session can open.
pub(crate) struct Seeded {
    pub(crate) a: String,
    pub(crate) b: String,
    pub(crate) chapter: String,
}

/// The observable result of one update.
struct Updated {
    code: (ReceiptResult, Option<&'static str>),
    receipt_id: String,
    applied: Option<(Node, u64, StructureAuthority)>,
}

impl Sibling {
    pub(crate) fn title(self, name: &str) -> String {
        format!("{self:?} {name}")
    }

    pub(crate) async fn seed(
        self,
        store: &PostgresProjectReader,
        scope: &ProjectScope,
        base: u16,
    ) -> Seeded {
        let suffix = |offset: u16| format!("{:04x}", base + offset);
        match self {
            Self::Volume => {
                let a = titled_volume(store, scope, &suffix(1), "Volume A", 1).await;
                let b = titled_volume(store, scope, &suffix(2), "Volume B", 2).await;
                let chapter = titled_chapter(store, scope, &suffix(3), &a, "Chapter A", 3).await;
                Seeded { a, b, chapter }
            }
            Self::Chapter => {
                let volume = titled_volume(store, scope, &suffix(1), "Volume A", 1).await;
                let a = titled_chapter(store, scope, &suffix(2), &volume, "Chapter A", 2).await;
                let b = titled_chapter(store, scope, &suffix(3), &volume, "Chapter B", 3).await;
                Seeded {
                    chapter: a.clone(),
                    a,
                    b,
                }
            }
        }
    }

    pub(crate) fn nodes(self, tree: &CanonicalManuscriptTree) -> Vec<Node> {
        match self {
            Self::Volume => tree
                .volumes
                .iter()
                .map(|volume| {
                    let id = volume.volume_id.as_ref().to_owned();
                    (id, volume.title.clone(), volume.order)
                })
                .collect(),
            Self::Chapter => tree.volumes[0]
                .chapters
                .iter()
                .map(|chapter| {
                    let id = chapter.chapter_id.as_ref().to_owned();
                    (id, chapter.title.clone(), chapter.order)
                })
                .collect(),
        }
    }

    #[expect(clippy::too_many_arguments, reason = "one table row names each input")]
    async fn update(
        self,
        store: &PostgresProjectReader,
        scope: &ProjectScope,
        suffix: &str,
        id: &str,
        title: &str,
        order: u64,
        expected_tree_revision: u64,
    ) -> Updated {
        match self {
            Self::Volume => {
                let settled = settle_update_volume(
                    store,
                    scope,
                    suffix,
                    id,
                    title,
                    order,
                    expected_tree_revision,
                )
                .await;
                Updated {
                    code: outcome_code(&settled.outcome),
                    receipt_id: settled.ids.receipt_id.clone(),
                    applied: matches!(settled.outcome, TransitionOutcome::Applied(_)).then(|| {
                        let (effect, authority) = applied(&settled);
                        let node = (id.to_owned(), effect.title, effect.order);
                        (node, effect.tree_revision, authority)
                    }),
                }
            }
            Self::Chapter => {
                let settled = settle_update_chapter(
                    store,
                    scope,
                    suffix,
                    id,
                    title,
                    order,
                    expected_tree_revision,
                )
                .await;
                Updated {
                    code: outcome_code(&settled.outcome),
                    receipt_id: settled.ids.receipt_id.clone(),
                    applied: matches!(settled.outcome, TransitionOutcome::Applied(_)).then(|| {
                        let (effect, authority) = applied(&settled);
                        let node = (id.to_owned(), effect.title, effect.order);
                        (node, effect.tree_revision, authority)
                    }),
                }
            }
        }
    }

    pub(crate) fn join_refusal(self) -> &'static str {
        match self {
            Self::Volume => "invalid_volume_join",
            Self::Chapter => "invalid_chapter_join",
        }
    }
}

/// The prior title and prior order that the Activity payload of one update records.
async fn prior_fields(admin: &tokio_postgres::Client, receipt_id: &str) -> (String, String) {
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
async fn chapter_identity(admin: &tokio_postgres::Client, scope: &ProjectScope) -> String {
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
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
        let current = open_current_chapter(&store, &scope, &ChapterId::new(chapter.clone()))
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
        // An archived Project has no readable Manuscript Tree, so this step is the last.
        crate::create_structure_tests::archive(&admin, &scope).await;
        let archived = kind
            .update(&store, &scope, &suffix(8), &a, &renamed, 2, 5)
            .await;
        codes.push(archived.code);
        assert_eq!(
            codes,
            [
                (ReceiptResult::Conflicted, Some("stale_tree_revision")),
                (ReceiptResult::Refused, Some(kind.join_refusal())),
                (ReceiptResult::Refused, Some("invalid_order")),
                (ReceiptResult::Refused, Some("archived_project")),
            ],
            "{kind:?}"
        );
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_of_each_update_restores_the_prior_title_and_live_sibling_place() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
