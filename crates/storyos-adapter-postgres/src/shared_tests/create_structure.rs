//! The shared test support from `create_structure_tests.rs`.

use storyos_application::{Chapter, ProjectReadError, ProjectReader};
use storyos_application::{ChapterId, ProjectScope};

/// The Chapter `chapter_id` when it is the current Chapter of the Project of `scope`.
pub(crate) async fn current_chapter(
    reader: &impl ProjectReader,
    scope: &ProjectScope,
    chapter_id: &ChapterId,
) -> Result<Option<Chapter>, ProjectReadError> {
    let Some(project) = reader.read_project(scope).await? else {
        return Ok(None);
    };
    if project.current_chapter_id.as_ref() != Some(chapter_id) {
        return Ok(None);
    }
    reader.read_chapter(scope, chapter_id).await
}
