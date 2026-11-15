use super::*;

/// The Skills that the Server validated at its start, in name order, after the Project visibility
/// check. The read writes no row and reads no Skill directory (ADR 0089 section 8).
pub(super) async fn list_skills(
    State(state): State<Arc<ServerState>>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<contracts::ListSkillsResponse>, ApiError> {
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    let reader = project_reader(&state).await?;
    if open_project(&reader, &scope)
        .await
        .map_err(service_unavailable)?
        .is_none()
    {
        return Err(resource_unavailable());
    }
    Ok(Json(contracts::ListSkillsResponse {
        schema_id: contracts::LIST_SKILLS_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: Uuid::now_v7().to_string(),
        project_scope: contract_scope(&scope),
        skills: state
            .config
            .skills
            .packages()
            .iter()
            .map(|package| contracts::SkillCatalogEntry {
                reference: contracts::SkillPackageReference {
                    name: package.name.clone(),
                    package_digest: package.package_digest.clone(),
                },
                description: package.description.clone(),
            })
            .collect(),
    }))
}
