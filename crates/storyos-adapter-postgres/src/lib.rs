//! PostgreSQL implementation of StoryOS Application ports.

use storyos_application::{
    Chapter, ChapterId, Project, ProjectId, ProjectReadError, ProjectReader, ProjectScope,
    RevisionId,
};
use tokio_postgres::NoTls;

#[derive(Clone, Debug)]
pub struct PostgresProjectReader {
    database_url: String,
}

impl PostgresProjectReader {
    pub fn new(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
        }
    }

    async fn connect(&self) -> Result<tokio_postgres::Client, ProjectReadError> {
        let (client, connection) = tokio_postgres::connect(&self.database_url, NoTls)
            .await
            .map_err(read_error)?;
        tokio::spawn(async move {
            let _connection_result = connection.await;
        });
        Ok(client)
    }
}

impl ProjectReader for PostgresProjectReader {
    async fn read_project(
        &self,
        scope: &ProjectScope,
    ) -> Result<Option<Project>, ProjectReadError> {
        let mut client = self.connect().await?;
        let transaction = client.transaction().await.map_err(read_error)?;
        set_scope(&transaction, scope).await?;
        let project = transaction
            .query_opt(
                "SELECT project_id::text, title, current_chapter_id::text \
                 FROM storyos.projects \
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
                &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
            )
            .await
            .map_err(read_error)?
            .map(|row| Project {
                project_id: ProjectId::new(row.get::<_, String>(0)),
                title: row.get(1),
                current_chapter_id: ChapterId::new(row.get::<_, String>(2)),
            });
        transaction.commit().await.map_err(read_error)?;
        Ok(project)
    }

    async fn read_chapter(
        &self,
        scope: &ProjectScope,
        chapter_id: &ChapterId,
    ) -> Result<Option<Chapter>, ProjectReadError> {
        let mut client = self.connect().await?;
        let transaction = client.transaction().await.map_err(read_error)?;
        set_scope(&transaction, scope).await?;
        let chapter = transaction
            .query_opt(
                "SELECT object.manuscript_object_id::text, object.title, \
                        revision.revision_id::text, convert_from(payload.canonical_bytes, 'UTF8') \
                 FROM storyos.manuscript_objects AS object \
                 JOIN storyos.projects AS project \
                   ON (project.owner_user_id, project.project_id, project.current_chapter_id) = \
                      (object.owner_user_id, object.project_id, object.manuscript_object_id) \
                 JOIN storyos.authoritative_heads AS head \
                   ON (head.owner_user_id, head.project_id, head.manuscript_object_id) = \
                      (object.owner_user_id, object.project_id, object.manuscript_object_id) \
                 JOIN storyos.authoritative_revisions AS revision \
                   ON (revision.owner_user_id, revision.project_id, revision.manuscript_object_id, revision.revision_id) = \
                      (head.owner_user_id, head.project_id, head.manuscript_object_id, head.current_revision_id) \
                 JOIN storyos.authoritative_payloads AS payload \
                   ON (payload.owner_user_id, payload.project_id, payload.payload_id) = \
                      (revision.owner_user_id, revision.project_id, revision.payload_id) \
                 WHERE object.owner_user_id = $1::text::uuid AND object.project_id = $2::text::uuid \
                   AND object.manuscript_object_id = $3::text::uuid AND object.object_kind = 'chapter'",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &chapter_id.as_ref(),
                ],
            )
            .await
            .map_err(read_error)?
            .map(|row| Chapter {
                chapter_id: ChapterId::new(row.get::<_, String>(0)),
                title: row.get(1),
                revision_id: RevisionId::new(row.get::<_, String>(2)),
                body: row.get(3),
            });
        transaction.commit().await.map_err(read_error)?;
        Ok(chapter)
    }
}

async fn set_scope(
    transaction: &tokio_postgres::Transaction<'_>,
    scope: &ProjectScope,
) -> Result<(), ProjectReadError> {
    transaction
        .execute(
            "SELECT set_config('storyos.user_id', $1, true), \
             set_config('storyos.owner_user_id', $1, true), \
             set_config('storyos.project_id', $2, true)",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map(|_| ())
        .map_err(read_error)
}

fn read_error(source: tokio_postgres::Error) -> ProjectReadError {
    ProjectReadError::unavailable(source)
}
