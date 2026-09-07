use storyos_application::{
    ExportHumanReadableManuscriptError, ExportProjectArchiveError, PinnedArchiveFamily,
    PinnedExportSource, PinnedExportSourceFacts, ProjectScope,
};
use storyos_core::{ReadableExportChapter, ReadableExportVolume, canonical_json};
use tokio_postgres::GenericClient;

use super::{ProjectReadError, read_error};
use crate::author_edit::sha256_hex;

pub(crate) const HUMAN_READABLE_COMPLETENESS: &str = "human_readable_manuscript";
pub(crate) const ARCHIVE_COMPLETENESS: &str = "project_export_archive";

pub(crate) async fn insert_human_readable_pinned_export_source(
    client: &impl GenericClient,
    scope: &ProjectScope,
    export_id: &str,
    source_snapshot_id: &str,
    volumes: &[ReadableExportVolume],
) -> Result<(), ExportHumanReadableManuscriptError> {
    let facts = human_readable_facts_json(volumes);
    let facts_text = facts.to_string();
    let facts_sha256 = sha256_hex(facts_text.as_bytes());
    client
        .execute(
            "INSERT INTO storyos.pinned_export_sources
               (owner_user_id, project_id, export_id, source_snapshot_id,
                completeness_profile, facts, facts_sha256)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5, $6::text::jsonb, $7)",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &export_id,
                &source_snapshot_id,
                &HUMAN_READABLE_COMPLETENESS,
                &facts_text,
                &facts_sha256,
            ],
        )
        .await
        .map_err(|error| ExportHumanReadableManuscriptError::Unavailable(Box::new(error)))?;
    Ok(())
}

pub(crate) async fn load_human_readable_pinned_export_source(
    client: &impl GenericClient,
    scope: &ProjectScope,
    export_id: &str,
    source_snapshot_id: &str,
) -> Result<Option<PinnedExportSource>, ProjectReadError> {
    let Some(row) = client
        .query_opt(
            "SELECT facts::text
               FROM storyos.pinned_export_sources
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND export_id = $3::text::uuid
                AND source_snapshot_id = $4::text::uuid
                AND completeness_profile = $5",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &export_id,
                &source_snapshot_id,
                &HUMAN_READABLE_COMPLETENESS,
            ],
        )
        .await
        .map_err(read_error)?
    else {
        return Ok(None);
    };
    let facts_text: String = row.get(0);
    let volumes = volumes_from_facts_json(&facts_text)?;
    Ok(Some(PinnedExportSource {
        project_scope: scope.clone(),
        export_id: export_id.to_owned(),
        source_snapshot_id: source_snapshot_id.to_owned(),
        facts: PinnedExportSourceFacts::HumanReadableManuscript { volumes },
    }))
}

pub(crate) async fn insert_archive_pinned_export_source(
    client: &impl GenericClient,
    scope: &ProjectScope,
    export_id: &str,
    source_snapshot_id: &str,
    families: &[PinnedArchiveFamily],
) -> Result<(), ExportProjectArchiveError> {
    let facts = archive_facts_json(families)?;
    let facts_text = facts.to_string();
    let facts_sha256 = sha256_hex(facts_text.as_bytes());
    client
        .execute(
            "INSERT INTO storyos.pinned_export_sources
               (owner_user_id, project_id, export_id, source_snapshot_id,
                completeness_profile, facts, facts_sha256)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5, $6::text::jsonb, $7)",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &export_id,
                &source_snapshot_id,
                &ARCHIVE_COMPLETENESS,
                &facts_text,
                &facts_sha256,
            ],
        )
        .await
        .map_err(|error| ExportProjectArchiveError::Unavailable(Box::new(error)))?;
    Ok(())
}

pub(crate) async fn load_archive_pinned_export_source(
    client: &impl GenericClient,
    scope: &ProjectScope,
    export_id: &str,
    source_snapshot_id: &str,
) -> Result<Option<PinnedExportSource>, ProjectReadError> {
    let Some(row) = client
        .query_opt(
            "SELECT facts::text
               FROM storyos.pinned_export_sources
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND export_id = $3::text::uuid
                AND source_snapshot_id = $4::text::uuid
                AND completeness_profile = $5",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &export_id,
                &source_snapshot_id,
                &ARCHIVE_COMPLETENESS,
            ],
        )
        .await
        .map_err(read_error)?
    else {
        return Ok(None);
    };
    let facts_text: String = row.get(0);
    let families = families_from_facts_json(&facts_text)?;
    Ok(Some(PinnedExportSource {
        project_scope: scope.clone(),
        export_id: export_id.to_owned(),
        source_snapshot_id: source_snapshot_id.to_owned(),
        facts: PinnedExportSourceFacts::ProjectExportArchive { families },
    }))
}

fn human_readable_facts_json(volumes: &[ReadableExportVolume]) -> serde_json::Value {
    serde_json::json!({
        "volumes": volumes
            .iter()
            .map(|volume| {
                serde_json::json!({
                    "title": volume.title,
                    "chapters": volume
                        .chapters
                        .iter()
                        .map(|chapter| {
                            serde_json::json!({
                                "title": chapter.title,
                                "body": chapter.body,
                            })
                        })
                        .collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>(),
    })
}

fn volumes_from_facts_json(
    facts_text: &str,
) -> Result<Vec<ReadableExportVolume>, ProjectReadError> {
    let facts: serde_json::Value = serde_json::from_str(facts_text).map_err(read_facts_error)?;
    let volumes = facts
        .get("volumes")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            read_facts_error(std::io::Error::other("pinned source volumes are required"))
        })?;
    volumes
        .iter()
        .map(|volume| {
            Ok(ReadableExportVolume {
                title: volume
                    .get("title")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| {
                        read_facts_error(std::io::Error::other(
                            "pinned source volume title is required",
                        ))
                    })?
                    .to_owned(),
                chapters: volume
                    .get("chapters")
                    .and_then(serde_json::Value::as_array)
                    .ok_or_else(|| {
                        read_facts_error(std::io::Error::other(
                            "pinned source volume chapters are required",
                        ))
                    })?
                    .iter()
                    .map(|chapter| {
                        Ok(ReadableExportChapter {
                            title: chapter
                                .get("title")
                                .and_then(serde_json::Value::as_str)
                                .ok_or_else(|| {
                                    read_facts_error(std::io::Error::other(
                                        "pinned source chapter title is required",
                                    ))
                                })?
                                .to_owned(),
                            body: match chapter.get("body") {
                                Some(serde_json::Value::Null) | None => None,
                                Some(serde_json::Value::String(text)) => Some(text.clone()),
                                Some(_) => {
                                    return Err(read_facts_error(std::io::Error::other(
                                        "pinned source chapter body must be a string or null",
                                    )));
                                }
                            },
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            })
        })
        .collect()
}

fn archive_facts_json(
    families: &[PinnedArchiveFamily],
) -> Result<serde_json::Value, ExportProjectArchiveError> {
    let mut encoded = Vec::with_capacity(families.len());
    for family in families {
        let rows: serde_json::Value = serde_json::from_str(&family.rows_json)
            .map_err(|error| ExportProjectArchiveError::Unavailable(Box::new(error)))?;
        encoded.push(serde_json::json!({
            "table": family.table,
            "path": family.path,
            "rows": rows,
        }));
    }
    Ok(serde_json::json!({ "families": encoded }))
}

fn families_from_facts_json(
    facts_text: &str,
) -> Result<Vec<PinnedArchiveFamily>, ProjectReadError> {
    let facts: serde_json::Value = serde_json::from_str(facts_text).map_err(read_facts_error)?;
    let families = facts
        .get("families")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            read_facts_error(std::io::Error::other("pinned source families are required"))
        })?;
    families
        .iter()
        .map(|family| {
            Ok(PinnedArchiveFamily {
                table: family
                    .get("table")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| {
                        read_facts_error(std::io::Error::other(
                            "pinned source family table is required",
                        ))
                    })?
                    .to_owned(),
                path: family
                    .get("path")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| {
                        read_facts_error(std::io::Error::other(
                            "pinned source family path is required",
                        ))
                    })?
                    .to_owned(),
                rows_json: canonical_json(family.get("rows").ok_or_else(|| {
                    read_facts_error(std::io::Error::other(
                        "pinned source family rows are required",
                    ))
                })?),
            })
        })
        .collect()
}

fn read_facts_error(error: impl std::error::Error + Send + Sync + 'static) -> ProjectReadError {
    ProjectReadError::unavailable(error)
}
