//! The shared test support from `update_structure_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::command_sequence_structure::{
    UPDATE_CHAPTER, UPDATE_VOLUME, named_call, outcome_code, titled_chapter, titled_volume,
    update_chapter, update_volume,
};
use crate::shared_tests::command_sequence_support::applied;
use storyos_application::{
    CanonicalManuscriptTree, ChapterId, GetManuscriptTree, ProjectScope, StructureAuthority,
    UpdateChapterInput, UpdateChapterSettlement, UpdateVolumeInput, UpdateVolumeSettlement,
    VolumeId, get_manuscript_tree,
};
use storyos_core::{ReceiptResult, TransitionOutcome};

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
pub(crate) struct Updated {
    pub(crate) code: (ReceiptResult, Option<&'static str>),
    pub(crate) receipt_id: String,
    pub(crate) applied: Option<(Node, u64, StructureAuthority)>,
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
    pub(crate) async fn update(
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
