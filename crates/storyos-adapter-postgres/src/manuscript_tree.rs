use storyos_application::{
    CanonicalTreeFacts, ChapterFact, ChapterId, ManuscriptTreeReader, ProjectReadError,
    ProjectScope, VolumeFact, VolumeId,
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
        let chapter_rows = transaction
            .query(
                "SELECT parent_volume_id::text, manuscript_object_id::text, title, tree_order::text
                   FROM storyos.manuscript_objects
                  WHERE owner_user_id = $1::text::uuid
                    AND project_id = $2::text::uuid
                    AND object_kind = 'chapter'
                    AND parent_volume_id IS NOT NULL
                  ORDER BY parent_volume_id, tree_order",
                &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
            )
            .await
            .map_err(read_error)?;
        transaction.commit().await.map_err(read_error)?;
        let mut chapters_by_volume = std::collections::BTreeMap::<String, Vec<ChapterFact>>::new();
        for chapter in chapter_rows {
            let volume_id = chapter.get::<_, String>(0);
            chapters_by_volume
                .entry(volume_id)
                .or_default()
                .push(ChapterFact {
                    project_scope: scope.clone(),
                    chapter_id: ChapterId::new(chapter.get::<_, String>(1)),
                    title: chapter.get(2),
                    order: chapter
                        .get::<_, String>(3)
                        .parse()
                        .map_err(ProjectReadError::unavailable)?,
                });
        }
        let mut volumes = Vec::with_capacity(volume_rows.len());
        for volume in volume_rows {
            let volume_id = volume.get::<_, String>(0);
            volumes.push(VolumeFact {
                project_scope: scope.clone(),
                chapters: chapters_by_volume.remove(&volume_id).unwrap_or_default(),
                volume_id: VolumeId::new(volume_id),
                title: volume.get(1),
                order: volume
                    .get::<_, String>(2)
                    .parse()
                    .map_err(ProjectReadError::unavailable)?,
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
