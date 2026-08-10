//! Project query use cases and inward-facing ports.
//!
//! Durable identities cannot be exchanged at the Application boundary:
//!
//! ```compile_fail
//! use storyos_application::{ProjectId, ProjectScope, UserId};
//!
//! let _scope = ProjectScope::new(
//!     ProjectId::new("018f0000-0000-7001-8000-000000000002"),
//!     UserId::new("018f0000-0000-7001-8000-000000000001"),
//! );
//! ```
//!
//! ```compile_fail
//! use storyos_application::{Chapter, ChapterId, RevisionId};
//!
//! let _chapter = Chapter {
//!     chapter_id: RevisionId::new("018f0000-0000-7001-8000-000000000005"),
//!     title: "Chapter".to_owned(),
//!     revision_id: ChapterId::new("018f0000-0000-7001-8000-000000000003"),
//!     body: "Body".to_owned(),
//! };
//! ```

use std::future::Future;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserId(String);

impl UserId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl AsRef<str> for UserId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectId(String);

impl ProjectId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl AsRef<str> for ProjectId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChapterId(String);

impl ChapterId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl AsRef<str> for ChapterId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionId(String);

impl RevisionId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl AsRef<str> for RevisionId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectScope {
    pub owner_user_id: UserId,
    pub project_id: ProjectId,
}

impl ProjectScope {
    pub fn new(owner_user_id: UserId, project_id: ProjectId) -> Self {
        Self {
            owner_user_id,
            project_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Project {
    pub project_id: ProjectId,
    pub title: String,
    pub current_chapter_id: ChapterId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Chapter {
    pub chapter_id: ChapterId,
    pub title: String,
    pub revision_id: RevisionId,
    pub body: String,
}

#[derive(Debug)]
pub struct ProjectReadError {
    source: Box<dyn std::error::Error + Send + Sync>,
}

impl ProjectReadError {
    pub fn unavailable(source: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self {
            source: Box::new(source),
        }
    }
}

impl std::fmt::Display for ProjectReadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Project read is unavailable")
    }
}

impl std::error::Error for ProjectReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

/// Reads canonical Project data under one already authenticated exact Project Scope.
pub trait ProjectReader: Sync {
    fn read_project(
        &self,
        scope: &ProjectScope,
    ) -> impl Future<Output = Result<Option<Project>, ProjectReadError>> + Send;

    fn read_chapter(
        &self,
        scope: &ProjectScope,
        chapter_id: &ChapterId,
    ) -> impl Future<Output = Result<Option<Chapter>, ProjectReadError>> + Send;
}

pub async fn open_project(
    reader: &impl ProjectReader,
    scope: &ProjectScope,
) -> Result<Option<Project>, ProjectReadError> {
    reader.read_project(scope).await
}

pub async fn open_current_chapter(
    reader: &impl ProjectReader,
    scope: &ProjectScope,
    chapter_id: &ChapterId,
) -> Result<Option<Chapter>, ProjectReadError> {
    let Some(project) = reader.read_project(scope).await? else {
        return Ok(None);
    };
    if &project.current_chapter_id != chapter_id {
        return Ok(None);
    }
    reader.read_chapter(scope, chapter_id).await
}

#[cfg(test)]
#[path = "project_read_tests.rs"]
mod tests;
