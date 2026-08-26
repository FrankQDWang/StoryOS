use storyos_application::{
    CanonicalTreeFacts, ManuscriptTreeReader, ProjectReadError, ProjectScope, VolumeFact, VolumeId,
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
        let Some(snapshot) = snapshot else {
            transaction.commit().await.map_err(read_error)?;
            return Ok(None);
        };
        let volume_rows = transaction
            .query(
                "SELECT manuscript_object_id::text, title, tree_order::text
                   FROM storyos.manuscript_objects
                  WHERE owner_user_id = $1::text::uuid
                    AND project_id = $2::text::uuid
                    AND object_kind = 'volume'
                  ORDER BY tree_order",
                &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
            )
            .await
            .map_err(read_error)?;
        transaction.commit().await.map_err(read_error)?;
        let mut volumes = Vec::with_capacity(volume_rows.len());
        for volume in volume_rows {
            volumes.push(VolumeFact {
                project_scope: scope.clone(),
                volume_id: VolumeId::new(volume.get::<_, String>(0)),
                title: volume.get(1),
                order: volume
                    .get::<_, String>(2)
                    .parse()
                    .map_err(ProjectReadError::unavailable)?,
                chapters: Vec::new(),
            });
        }
        Ok(Some(CanonicalTreeFacts {
            project_scope: scope.clone(),
            snapshot,
            tree_revision: tree_revision
                .get::<_, String>(0)
                .parse()
                .map_err(ProjectReadError::unavailable)?,
            volumes,
        }))
    }
}

#[cfg(test)]
#[path = "manuscript_tree_tests.rs"]
mod tests;
