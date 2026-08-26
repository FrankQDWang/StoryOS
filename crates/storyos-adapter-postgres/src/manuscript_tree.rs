use storyos_application::{
    CanonicalTreeFacts, ManuscriptTreeReader, ProjectReadError, ProjectScope,
};

use super::{PostgresProjectReader, read_error, set_scope};

impl ManuscriptTreeReader for PostgresProjectReader {
    async fn read_canonical_tree_facts(
        &self,
        scope: &ProjectScope,
    ) -> Result<Option<CanonicalTreeFacts>, ProjectReadError> {
        let mut client = self.connect().await?;
        let transaction = client.transaction().await.map_err(read_error)?;
        set_scope(&transaction, scope).await?;
        let Some(tree_revision) = transaction
            .query_opt(
                "SELECT tree_revision::text
                   FROM storyos.projects
                  WHERE owner_user_id = $1::text::uuid
                    AND project_id = $2::text::uuid
                    AND lifecycle_state = 'active'",
                &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
            )
            .await
            .map_err(read_error)?
        else {
            transaction.commit().await.map_err(read_error)?;
            return Ok(None);
        };
        let snapshot = crate::snapshot::load_latest_canonical_snapshot(&transaction, scope).await?;
        transaction.commit().await.map_err(read_error)?;
        let Some(snapshot) = snapshot else {
            return Ok(None);
        };
        Ok(Some(CanonicalTreeFacts {
            project_scope: scope.clone(),
            snapshot,
            tree_revision: tree_revision
                .get::<_, String>(0)
                .parse()
                .map_err(ProjectReadError::unavailable)?,
            volumes: Vec::new(),
        }))
    }
}

#[cfg(test)]
#[path = "manuscript_tree_tests.rs"]
mod tests;
