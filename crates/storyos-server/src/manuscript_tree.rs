use storyos_application::get_manuscript_tree as read_canonical_manuscript_tree;

use super::*;

pub(super) async fn get_manuscript_tree(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<contracts::GetManuscriptTreeResponse>, ApiError> {
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    let reader = project_reader(&state)?;
    let Some(tree) = read_canonical_manuscript_tree(&reader, &scope)
        .await
        .map_err(service_unavailable)?
    else {
        return Err(resource_unavailable());
    };
    let project_scope = contract_scope(&scope);
    Ok(Json(contracts::GetManuscriptTreeResponse {
        schema_id: contracts::GET_MANUSCRIPT_TREE_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: Uuid::now_v7().to_string(),
        project_scope: project_scope.clone(),
        tree_revision: tree.tree_revision.to_string(),
        snapshot: contracts::SnapshotDescriptor {
            snapshot_id: tree.snapshot.snapshot_id,
            project_scope,
            snapshot_kind: contracts::SnapshotKind::Canonical,
            project_activity_position: tree.snapshot.project_activity_position.to_string(),
            source_watermarks: contracts::CanonicalSnapshotMaps {},
            projection_generations: contracts::CanonicalSnapshotMaps {},
            redaction_profile: tree.snapshot.redaction_profile,
            schema_profile: tree.snapshot.schema_profile,
            replay_generation: tree.snapshot.replay_generation.to_string(),
            created_at: tree.snapshot.created_at,
            expires_at: tree.snapshot.expires_at,
        },
        volumes: tree
            .volumes
            .into_iter()
            .map(|volume| contracts::ManuscriptVolumeNode {
                volume_id: volume.volume_id.as_ref().to_owned(),
                title: volume.title,
                order: volume.order.to_string(),
                chapters: volume
                    .chapters
                    .into_iter()
                    .map(|chapter| contracts::ManuscriptChapterNode {
                        chapter_id: chapter.chapter_id.as_ref().to_owned(),
                        title: chapter.title,
                        order: chapter.order.to_string(),
                    })
                    .collect(),
            })
            .collect(),
    }))
}
