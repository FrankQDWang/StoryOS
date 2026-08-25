use super::{
    Chapter, ChapterId, Project, ProjectId, ProjectReadError, ProjectReader, ProjectScope,
    RevisionId, UserId, open_current_chapter,
};

struct FixtureReader;

impl ProjectReader for FixtureReader {
    async fn read_project(
        &self,
        _scope: &ProjectScope,
    ) -> Result<Option<Project>, ProjectReadError> {
        Ok(Some(Project {
            project_id: ProjectId::new("project-a"),
            title: "Project A".to_owned(),
            current_chapter_id: Some(ChapterId::new("chapter-current")),
        }))
    }

    async fn read_chapter(
        &self,
        _scope: &ProjectScope,
        chapter_id: &ChapterId,
    ) -> Result<Option<Chapter>, ProjectReadError> {
        assert_eq!(chapter_id, &ChapterId::new("chapter-current"));
        Ok(Some(Chapter {
            chapter_id: chapter_id.clone(),
            title: "Chapter".to_owned(),
            revision_id: RevisionId::new("revision-current"),
            body: "Current body".to_owned(),
            project_activity_position: 0,
        }))
    }
}

#[tokio::test]
async fn stale_chapter_identity_is_refused_before_the_chapter_query() {
    let result = open_current_chapter(
        &FixtureReader,
        &ProjectScope::new(UserId::new("user-a"), ProjectId::new("project-a")),
        &ChapterId::new("chapter-stale"),
    )
    .await;

    assert_eq!(result.unwrap(), None);
}
