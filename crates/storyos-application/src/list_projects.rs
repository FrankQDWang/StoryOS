use std::future::Future;

use crate::{ChapterId, ProjectReadError, ProjectScope, UserId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectLifecycle {
    Active,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectListItem {
    pub project_scope: ProjectScope,
    pub title: String,
    pub lifecycle: ProjectLifecycle,
    pub revision: u64,
    pub current_chapter_id: Option<ChapterId>,
}

/// Reads the current User's Project library under trusted User-level isolation.
pub trait ProjectLibrary: Sync {
    fn list_owned_projects(
        &self,
        owner_user_id: &UserId,
    ) -> impl Future<Output = Result<Vec<ProjectListItem>, ProjectReadError>> + Send;
}

pub async fn list_owned_projects(
    library: &impl ProjectLibrary,
    owner_user_id: &UserId,
) -> Result<Vec<ProjectListItem>, ProjectReadError> {
    library.list_owned_projects(owner_user_id).await
}
