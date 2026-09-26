use serde_json::{Value, json};
use storyos_application::{ExportProjectArchiveError, ProjectScope};
use storyos_core::{ArchiveEntrySource, ProjectArchiveBuildRefusal, canonical_json, hex_sha256};

use super::project_archive_draft::text;

struct RestrictedDraftSource {
    draft_id: String,
    revision_id: String,
    command_id: String,
}

async fn restricted_sources(
    client: &impl tokio_postgres::GenericClient,
    scope: &ProjectScope,
) -> Result<
    std::collections::BTreeMap<String, RestrictedDraftSource>,
    storyos_application::ProjectReadError,
> {
    let rows = client
        .query(
            "SELECT event.author_command_admission_id::text, draft.draft_id::text, draft.current_revision_id::text, event.command_id::text
           FROM storyos.draft_artifacts AS draft
           LEFT JOIN storyos.draft_lifecycle_events AS event
             ON (event.owner_user_id,event.project_id,event.draft_id,event.revision_id) =
                (draft.owner_user_id,draft.project_id,draft.draft_id,draft.current_revision_id)
          WHERE draft.owner_user_id=$1::text::uuid AND draft.project_id=$2::text::uuid
            AND draft.retention_state='tombstoned'",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(crate::read_error)?;
    rows.into_iter()
        .map(|row| {
            Ok((
                row.get::<_, Option<String>>(0).ok_or_else(|| {
                    storyos_application::ProjectReadError::unavailable(std::io::Error::other(
                        "Draft creation source is missing",
                    ))
                })?,
                RestrictedDraftSource {
                    draft_id: row.get(1),
                    revision_id: row.get(2),
                    command_id: row.get::<_, Option<String>>(3).ok_or_else(|| {
                        storyos_application::ProjectReadError::unavailable(std::io::Error::other(
                            "Draft command source is missing",
                        ))
                    })?,
                },
            ))
        })
        .collect()
}

fn copied_restricted_drafts(
    facts: &Value,
    restricted: &std::collections::BTreeMap<String, RestrictedDraftSource>,
    scope: &ProjectScope,
) -> Result<std::collections::BTreeSet<String>, ProjectArchiveBuildRefusal> {
    let draft_revisions: std::collections::BTreeMap<&str, &str> = restricted
        .values()
        .map(|source| (source.draft_id.as_str(), source.revision_id.as_str()))
        .collect();
    let mut found = std::collections::BTreeSet::new();
    let mut pending = vec![facts];
    while let Some(facts) = pending.pop() {
        let families = facts
            .get("families")
            .and_then(Value::as_array)
            .ok_or(ProjectArchiveBuildRefusal::InvalidProvenance)?;
        for family in families {
            let table = text(family, "table")?;
            let rows = family
                .get("rows")
                .and_then(Value::as_array)
                .ok_or(ProjectArchiveBuildRefusal::InvalidProvenance)?;
            if matches!(
                table,
                "draft_artifact_revisions" | "author_command_admissions" | "pinned_export_sources"
            ) && rows.iter().any(|row| {
                row["owner_user_id"] != scope.owner_user_id.as_ref()
                    || row["project_id"] != scope.project_id.as_ref()
            }) {
                return Err(ProjectArchiveBuildRefusal::ForeignMaterial);
            }
            match table {
                "draft_artifact_revisions" => {
                    for row in rows {
                        let draft_id = text(row, "draft_id")?;
                        if row.get("payload").is_some()
                            && let Some(revision_id) = draft_revisions.get(draft_id)
                        {
                            if row["revision_id"] != *revision_id {
                                return Err(ProjectArchiveBuildRefusal::InvalidProvenance);
                            }
                            found.insert(draft_id.to_owned());
                        }
                    }
                }
                "author_command_admissions" => {
                    for row in rows {
                        if row.get("command_payload").is_some()
                            && let Some(draft_id) =
                                restricted.get(text(row, "author_command_admission_id")?)
                        {
                            if row["command_id"] != draft_id.command_id {
                                return Err(ProjectArchiveBuildRefusal::InvalidProvenance);
                            }
                            found.insert(draft_id.draft_id.clone());
                        }
                    }
                }
                "pinned_export_sources" => {
                    for row in rows {
                        match text(row, "completeness_profile")? {
                            "human_readable_manuscript" => {}
                            "project_export_archive" => {
                                if let Some(nested) = row.get("facts") {
                                    if hex_sha256(canonical_json(nested).as_bytes())
                                        != text(row, "facts_sha256")?
                                    {
                                        return Err(ProjectArchiveBuildRefusal::CorruptDigest);
                                    }
                                    pending.push(nested);
                                } else {
                                    let gap = row
                                        .get("payload_availability")
                                        .ok_or(ProjectArchiveBuildRefusal::InvalidProvenance)?;
                                    let ids = gap
                                        .get("restricted_draft_ids")
                                        .and_then(Value::as_array)
                                        .filter(|ids| !ids.is_empty())
                                        .ok_or(ProjectArchiveBuildRefusal::InvalidProvenance)?;
                                    if gap
                                        != &json!({"kind":"refused_edit_pinned_export_source_facts","reason":"withheld_due_to_tombstone",
                                        "entry_path":"canonical/pinned_export_sources.json","record_id":text(row,"export_id")?,
                                        "payload_field":"facts","restricted_draft_ids":ids,"facts_sha256":text(row,"facts_sha256")?})
                                    {
                                        return Err(ProjectArchiveBuildRefusal::InvalidProvenance);
                                    }
                                }
                            }
                            _ => return Err(ProjectArchiveBuildRefusal::InvalidProvenance),
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(found)
}

pub(super) async fn withhold_pinned_source_copies(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    rows: &mut [Value],
) -> Result<(), ExportProjectArchiveError> {
    let restricted = restricted_sources(client, scope)
        .await
        .map_err(|error| ExportProjectArchiveError::Unavailable(Box::new(error)))?;
    if restricted.is_empty() {
        return Ok(());
    }
    for row in rows {
        if row["completeness_profile"] != "project_export_archive" {
            continue;
        }
        let facts = row.get("facts").ok_or_else(|| {
            ExportProjectArchiveError::ArchiveBuild(ProjectArchiveBuildRefusal::InvalidProvenance)
        })?;
        if hex_sha256(canonical_json(facts).as_bytes())
            != text(row, "facts_sha256").map_err(ExportProjectArchiveError::ArchiveBuild)?
        {
            return Err(ExportProjectArchiveError::ArchiveBuild(
                ProjectArchiveBuildRefusal::CorruptDigest,
            ));
        }
        let restricted_draft_ids = copied_restricted_drafts(facts, &restricted, scope)
            .map_err(ExportProjectArchiveError::ArchiveBuild)?;
        if restricted_draft_ids.is_empty() {
            continue;
        }
        let facts_sha256 = text(row, "facts_sha256")
            .map_err(ExportProjectArchiveError::ArchiveBuild)?
            .to_owned();
        let gap = json!({"kind":"refused_edit_pinned_export_source_facts", "reason":"withheld_due_to_tombstone",
            "entry_path":"canonical/pinned_export_sources.json", "record_id":text(row,"export_id").map_err(ExportProjectArchiveError::ArchiveBuild)?,
            "payload_field":"facts", "restricted_draft_ids":restricted_draft_ids, "facts_sha256":facts_sha256});
        let object = row.as_object_mut().ok_or_else(|| {
            ExportProjectArchiveError::ArchiveBuild(ProjectArchiveBuildRefusal::InvalidProvenance)
        })?;
        object.remove("facts");
        object.insert("payload_availability".to_owned(), gap);
    }
    Ok(())
}

pub(super) async fn stored_payload_is_ineligible(
    client: &impl tokio_postgres::GenericClient,
    scope: &ProjectScope,
    sources: &[ArchiveEntrySource],
) -> Result<bool, storyos_application::ProjectReadError> {
    let restricted = restricted_sources(client, scope).await?;
    if restricted.is_empty() {
        return Ok(false);
    }
    let mut families = Vec::new();
    for source in sources {
        let Some(table) = source
            .path
            .strip_prefix("canonical/")
            .and_then(|path| path.strip_suffix(".json"))
        else {
            continue;
        };
        if !matches!(
            table,
            "draft_artifact_revisions" | "author_command_admissions" | "pinned_export_sources"
        ) {
            continue;
        }
        let rows: Vec<Value> = serde_json::from_slice(&source.bytes)
            .map_err(storyos_application::ProjectReadError::unavailable)?;
        families.push(json!({"table":table,"rows":rows}));
    }
    let found = copied_restricted_drafts(&json!({"families":families}), &restricted, scope)
        .map_err(|error| {
            storyos_application::ProjectReadError::unavailable(std::io::Error::other(format!(
                "{error:?}"
            )))
        })?;
    Ok(!found.is_empty())
}
