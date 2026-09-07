use storyos_application::{
    ExportHumanReadableManuscriptError, ExportProjectArchiveError, PinnedArchiveFamily,
    PinnedExportSource, PinnedExportSourceFacts, ProjectScope,
};
use storyos_core::{ReadableExportChapter, ReadableExportVolume, canonical_json};
use tokio_postgres::GenericClient;

use super::{ProjectReadError, read_error};
use crate::author_edit::sha256_hex;

/// Receipt reason when export settlement fails closed.
///
/// The persisted `domain_receipts_result_shape` vocabulary admits one refusal
/// reason for export commands, so an unavailable Pinned Export Source or
/// Snapshot records the same word as an archived Project. A distinct reason
/// needs a receipt-shape migration.
pub(crate) const EXPORT_REFUSED_RECEIPT_REASON: &str = "archived_project";

/// Completeness profile stored with one Pinned Export Source row.
#[derive(Clone, Copy)]
pub(crate) enum PinnedExportSourceCompleteness {
    HumanReadableManuscript,
    ProjectExportArchive,
}

impl PinnedExportSourceCompleteness {
    fn column_value(self) -> &'static str {
        match self {
            Self::HumanReadableManuscript => "human_readable_manuscript",
            Self::ProjectExportArchive => "project_export_archive",
        }
    }
}

/// Availability of one Pinned Export Source at Worker settlement.
///
/// The source is unavailable when its row is missing, stores another
/// completeness profile, fails its recorded digest, or cannot restore the
/// facts that its profile requires. Settlement must then fail closed.
pub(crate) enum PinnedExportSourceLoad {
    Available(PinnedExportSource),
    Unavailable,
}

pub(crate) async fn insert_human_readable_pinned_export_source(
    client: &impl GenericClient,
    scope: &ProjectScope,
    export_id: &str,
    source_snapshot_id: &str,
    volumes: &[ReadableExportVolume],
) -> Result<(), ExportHumanReadableManuscriptError> {
    insert_pinned_export_source(
        client,
        scope,
        export_id,
        source_snapshot_id,
        PinnedExportSourceCompleteness::HumanReadableManuscript,
        &human_readable_facts_json(volumes),
    )
    .await
    .map_err(|error| ExportHumanReadableManuscriptError::Unavailable(Box::new(error)))
}

pub(crate) async fn insert_archive_pinned_export_source(
    client: &impl GenericClient,
    scope: &ProjectScope,
    export_id: &str,
    source_snapshot_id: &str,
    families: &[PinnedArchiveFamily],
) -> Result<(), ExportProjectArchiveError> {
    let facts = archive_facts_json(families)?;
    insert_pinned_export_source(
        client,
        scope,
        export_id,
        source_snapshot_id,
        PinnedExportSourceCompleteness::ProjectExportArchive,
        &facts,
    )
    .await
    .map_err(|error| ExportProjectArchiveError::Unavailable(Box::new(error)))
}

async fn insert_pinned_export_source(
    client: &impl GenericClient,
    scope: &ProjectScope,
    export_id: &str,
    source_snapshot_id: &str,
    completeness: PinnedExportSourceCompleteness,
    facts: &serde_json::Value,
) -> Result<(), tokio_postgres::Error> {
    // The digest covers the canonical JSON text so that a later jsonb read can
    // re-canonicalize the stored value and compare it independently of the
    // PostgreSQL jsonb output format.
    let facts_text = canonical_json(facts);
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
                &completeness.column_value(),
                &facts_text,
                &facts_sha256,
            ],
        )
        .await?;
    Ok(())
}

pub(crate) async fn load_pinned_export_source(
    client: &impl GenericClient,
    scope: &ProjectScope,
    export_id: &str,
    source_snapshot_id: &str,
    completeness: PinnedExportSourceCompleteness,
) -> Result<PinnedExportSourceLoad, ProjectReadError> {
    let Some(row) = client
        .query_opt(
            "SELECT facts::text, facts_sha256
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
                &completeness.column_value(),
            ],
        )
        .await
        .map_err(read_error)?
    else {
        return Ok(PinnedExportSourceLoad::Unavailable);
    };
    let facts_text: String = row.get(0);
    let facts_sha256: String = row.get(1);
    let Ok(facts) = serde_json::from_str::<serde_json::Value>(&facts_text) else {
        return Ok(PinnedExportSourceLoad::Unavailable);
    };
    if sha256_hex(canonical_json(&facts).as_bytes()) != facts_sha256 {
        return Ok(PinnedExportSourceLoad::Unavailable);
    }
    let facts = match completeness {
        PinnedExportSourceCompleteness::HumanReadableManuscript => volumes_from_facts_json(&facts)
            .map(|volumes| PinnedExportSourceFacts::HumanReadableManuscript { volumes }),
        PinnedExportSourceCompleteness::ProjectExportArchive => families_from_facts_json(&facts)
            .map(|families| PinnedExportSourceFacts::ProjectExportArchive { families }),
    };
    Ok(match facts {
        Some(facts) => PinnedExportSourceLoad::Available(PinnedExportSource {
            project_scope: scope.clone(),
            export_id: export_id.to_owned(),
            source_snapshot_id: source_snapshot_id.to_owned(),
            facts,
        }),
        None => PinnedExportSourceLoad::Unavailable,
    })
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

/// Restores manuscript facts. `None` means the source is partial.
fn volumes_from_facts_json(facts: &serde_json::Value) -> Option<Vec<ReadableExportVolume>> {
    facts
        .get("volumes")?
        .as_array()?
        .iter()
        .map(|volume| {
            Some(ReadableExportVolume {
                title: volume.get("title")?.as_str()?.to_owned(),
                chapters: volume
                    .get("chapters")?
                    .as_array()?
                    .iter()
                    .map(|chapter| {
                        Some(ReadableExportChapter {
                            title: chapter.get("title")?.as_str()?.to_owned(),
                            body: match chapter.get("body") {
                                Some(serde_json::Value::Null) | None => None,
                                Some(serde_json::Value::String(text)) => Some(text.clone()),
                                Some(_) => return None,
                            },
                        })
                    })
                    .collect::<Option<Vec<_>>>()?,
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

/// Restores Archive families. `None` means the source is partial.
fn families_from_facts_json(facts: &serde_json::Value) -> Option<Vec<PinnedArchiveFamily>> {
    facts
        .get("families")?
        .as_array()?
        .iter()
        .map(|family| {
            Some(PinnedArchiveFamily {
                table: family.get("table")?.as_str()?.to_owned(),
                path: family.get("path")?.as_str()?.to_owned(),
                rows_json: canonical_json(family.get("rows")?),
            })
        })
        .collect()
}
