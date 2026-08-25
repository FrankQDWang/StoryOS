use storyos_application::{
    ChapterId, ProjectId, ProjectLibrary, ProjectLifecycle, ProjectListItem, ProjectReadError,
    ProjectScope, UserId,
};

use super::{PostgresProjectReader, read_error};

impl ProjectLibrary for PostgresProjectReader {
    async fn list_owned_projects(
        &self,
        owner_user_id: &UserId,
    ) -> Result<Vec<ProjectListItem>, ProjectReadError> {
        let mut client = self.connect().await?;
        let transaction = client.transaction().await.map_err(read_error)?;
        transaction
            .execute(
                "SELECT set_config('storyos.scope_mode', 'user', true),
                        set_config('storyos.user_id', $1, true)",
                &[&owner_user_id.as_ref()],
            )
            .await
            .map(|_| ())
            .map_err(read_error)?;
        let rows = transaction
            .query(
                "SELECT project_id::text, title, current_chapter_id::text,
                        lifecycle_state, revision::text
                   FROM storyos.projects
                  WHERE owner_user_id = $1::text::uuid
                  ORDER BY project_id",
                &[&owner_user_id.as_ref()],
            )
            .await
            .map_err(read_error)?;
        let mut projects = Vec::with_capacity(rows.len());
        for row in rows {
            let lifecycle = match row.get::<_, String>(3).as_str() {
                "active" => ProjectLifecycle::Active,
                other => {
                    return Err(ProjectReadError::unavailable(std::io::Error::other(
                        format!("unsupported Project lifecycle {other}"),
                    )));
                }
            };
            projects.push(ProjectListItem {
                project_scope: ProjectScope::new(
                    owner_user_id.clone(),
                    ProjectId::new(row.get::<_, String>(0)),
                ),
                title: row.get(1),
                lifecycle,
                revision: row
                    .get::<_, String>(4)
                    .parse()
                    .map_err(ProjectReadError::unavailable)?,
                current_chapter_id: row.get::<_, Option<String>>(2).map(ChapterId::new),
            });
        }
        transaction.commit().await.map_err(read_error)?;
        Ok(projects)
    }
}
