//! The shared test support from `delete_structure_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::command_sequence_structure::{
    DELETE_CHAPTER, DELETE_VOLUME, delete_chapter, delete_volume, named_call,
};
use storyos_application::{
    ChapterId, DeleteChapterInput, DeleteChapterSettlement, DeleteVolumeInput,
    DeleteVolumeSettlement, ProjectScope, VolumeId,
};

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
