use storyos_application::{
    CanonicalSnapshot, ExportOperationPage, ExportProjectArchiveError, ProjectReadError,
    ProjectScope, VerifiedExportArchive,
};
use storyos_core::{
    ARCHIVE_PATH_PROFILE, ARCHIVE_ROOT_DIGEST_PROFILE, ARCHIVE_SERIALIZATION_PROFILE,
    ArchiveEntrySource, PROJECT_EXPORT_ARCHIVE_PROFILE, ProjectArchiveBuildRefusal,
    ProjectArchiveRootFacts, build_project_archive, canonical_json, catalog_include_table_names,
    classify_export_record, package_verified_project_archive_zip, require_delivered_families,
    required_export_tables,
};

const EXPORT_TABLES: &[(&str, &str)] = &[
    (
        "author_action_entries",
        "canonical/author_action_entries.json",
    ),
    (
        "author_command_admission_outcome_unknown_observations",
        "canonical/author_command_admission_outcome_unknown_observations.json",
    ),
    (
        "author_command_admission_reconfirmations",
        "canonical/author_command_admission_reconfirmations.json",
    ),
    (
        "author_command_admission_settlements",
        "canonical/author_command_admission_settlements.json",
    ),
    (
        "author_command_admissions",
        "canonical/author_command_admissions.json",
    ),
    (
        "authoritative_commits",
        "canonical/authoritative_commits.json",
    ),
    ("authoritative_heads", "canonical/authoritative_heads.json"),
    (
        "authoritative_payloads",
        "canonical/authoritative_payloads.json",
    ),
    (
        "authoritative_revision_envelopes",
        "canonical/authoritative_revision_envelopes.json",
    ),
    (
        "authoritative_revisions",
        "canonical/authoritative_revisions.json",
    ),
    (
        "chapter_removal_decisions",
        "canonical/chapter_removal_decisions.json",
    ),
    ("command_idempotency", "canonical/command_idempotency.json"),
    ("domain_receipts", "canonical/domain_receipts.json"),
    (
        "editor_session_base_snapshots",
        "canonical/editor_session_base_snapshots.json",
    ),
    ("editor_sessions", "canonical/editor_sessions.json"),
    (
        "human_readable_manuscript_export_operations",
        "canonical/human_readable_manuscript_export_operations.json",
    ),
    (
        "human_readable_manuscript_exports",
        "canonical/human_readable_manuscript_exports.json",
    ),
    ("manuscript_blocks", "canonical/manuscript_blocks.json"),
    ("manuscript_objects", "canonical/manuscript_objects.json"),
    (
        "manuscript_revision_members",
        "canonical/manuscript_revision_members.json",
    ),
    (
        "project_activity_event_payloads",
        "canonical/project_activity_event_payloads.json",
    ),
    (
        "project_activity_events",
        "canonical/project_activity_events.json",
    ),
    (
        "project_archival_decisions",
        "canonical/project_archival_decisions.json",
    ),
    (
        "project_export_operations",
        "canonical/project_export_operations.json",
    ),
    ("project_snapshots", "canonical/project_snapshots.json"),
    (
        "project_writer_generations",
        "canonical/project_writer_generations.json",
    ),
    ("projects", "canonical/projects.json"),
    ("replay_floors", "canonical/replay_floors.json"),
    ("replay_generations", "canonical/replay_generations.json"),
    ("scope_counters", "canonical/scope_counters.json"),
    (
        "volume_removal_decisions",
        "canonical/volume_removal_decisions.json",
    ),
];

pub(super) async fn persist_export_archive(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    export_id: &str,
    snapshot: &CanonicalSnapshot,
    created_at: &str,
) -> Result<String, ExportProjectArchiveError> {
    let mut sources = Vec::with_capacity(EXPORT_TABLES.len());
    let mut present = Vec::with_capacity(EXPORT_TABLES.len());
    let mut counts = Vec::with_capacity(EXPORT_TABLES.len());
    for (table, path) in EXPORT_TABLES {
        let rows = load_table_json(client, table, scope).await?;
        classify_rows(&rows, scope)?;
        present.push(*table);
        counts.push(((*table).to_owned(), rows.len().to_string()));
        let bytes = canonical_json(&serde_json::Value::Array(rows)).into_bytes();
        sources.push(ArchiveEntrySource {
            path: (*path).to_owned(),
            media_type: "application/json".to_owned(),
            payload_schema: format!("storyos.table.{table}.v1"),
            bytes,
        });
    }
    let catalog: serde_json::Value = serde_json::from_str(PERSISTENCE_CATALOG)
        .map_err(|error| ExportProjectArchiveError::Unavailable(Box::new(error)))?;
    if catalog
        .get("families")
        .and_then(serde_json::Value::as_array)
        .is_none()
    {
        return Err(archive_build_error(
            ProjectArchiveBuildRefusal::InvalidProvenance,
        ));
    }
    let include_names = catalog_include_table_names(&catalog);
    let live_rows = client
        .query(
            "SELECT tablename FROM pg_catalog.pg_tables WHERE schemaname = 'storyos'",
            &[],
        )
        .await
        .map_err(|error| archive_table_error("pg_tables", error))?;
    let live_tables: Vec<String> = live_rows.iter().map(|row| row.get(0)).collect();
    let include_refs: Vec<&str> = include_names.iter().map(String::as_str).collect();
    let live_refs: Vec<&str> = live_tables.iter().map(String::as_str).collect();
    let required = required_export_tables(&include_refs, &live_refs);
    let required_refs: Vec<&str> = required.iter().map(String::as_str).collect();
    require_delivered_families(&present, &required_refs).map_err(archive_build_error)?;
    let built = build_project_archive(
        &archive_root_facts(export_id, scope, snapshot, counts, created_at),
        &sources,
    )
    .map_err(archive_build_error)?;
    for descriptor in &built.entries {
        let Some(source) = sources.iter().find(|source| source.path == descriptor.path) else {
            return Err(archive_build_error(
                ProjectArchiveBuildRefusal::InvalidProvenance,
            ));
        };
        client
            .execute(
                "INSERT INTO storyos.project_export_entries
                   (owner_user_id, project_id, export_id, path, media_type, payload_schema,
                    byte_length, digest, payload)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4, $5, $6,
                         $7::text::bigint, $8, $9)",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &export_id,
                    &descriptor.path,
                    &descriptor.media_type,
                    &descriptor.payload_schema,
                    &descriptor.byte_length,
                    &format!("sha256:{}", descriptor.digest_hex),
                    &source.bytes.as_slice(),
                ],
            )
            .await
            .map_err(|error| archive_table_error("project_export_entries", error))?;
    }
    let immutable_root = format!("sha256:{}", built.root_digest_hex);
    let updated = client
        .execute(
            "UPDATE storyos.project_export_manifests
                SET immutable_root = $4
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND export_id = $3::text::uuid AND immutable_root IS NULL",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &export_id,
                &immutable_root,
            ],
        )
        .await
        .map_err(|error| archive_table_error("project_export_manifests", error))?;
    if updated != 1 {
        return Err(ExportProjectArchiveError::Unavailable(Box::new(
            std::io::Error::other("the admitted export did not persist one immutable root"),
        )));
    }
    Ok(immutable_root)
}

pub(super) async fn package_stored_export(
    client: &impl tokio_postgres::GenericClient,
    scope: &ProjectScope,
    page: &ExportOperationPage,
) -> Result<VerifiedExportArchive, ProjectReadError> {
    let immutable_root = page.immutable_root.as_str();
    let created_at = client
        .query_opt(
            "SELECT to_char(receipt.created_at AT TIME ZONE 'UTC',
                            'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
               FROM storyos.project_export_manifests AS export
               JOIN storyos.domain_receipts AS receipt
                 ON receipt.owner_user_id = export.owner_user_id
                AND receipt.project_id = export.project_id
                AND receipt.receipt_id = export.receipt_id
              WHERE export.owner_user_id = $1::text::uuid
                AND export.project_id = $2::text::uuid
                AND export.export_id = $3::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &page.export_id,
            ],
        )
        .await
        .map_err(ProjectReadError::unavailable)?;
    let Some(created_at) = created_at else {
        return Ok(VerifiedExportArchive::Unsettled);
    };
    let created_at: String = created_at.get(0);
    let rows = client
        .query(
            "SELECT path, media_type, payload_schema, payload
               FROM storyos.project_export_entries
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND export_id = $3::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &page.export_id,
            ],
        )
        .await
        .map_err(ProjectReadError::unavailable)?;
    if rows.is_empty() {
        return Ok(VerifiedExportArchive::Unsettled);
    }
    let mut sources = Vec::with_capacity(rows.len());
    let mut counts = Vec::with_capacity(rows.len());
    for row in rows {
        let path: String = row.get(0);
        let media_type: String = row.get(1);
        let payload_schema: String = row.get(2);
        let bytes: Vec<u8> = row.get(3);
        let Some(table) = path
            .strip_prefix("canonical/")
            .and_then(|name| name.strip_suffix(".json"))
        else {
            return Ok(VerifiedExportArchive::Refused(
                ProjectArchiveBuildRefusal::InvalidProvenance,
            ));
        };
        let count = match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(serde_json::Value::Array(values)) => values.len().to_string(),
            _ => {
                return Ok(VerifiedExportArchive::Refused(
                    ProjectArchiveBuildRefusal::InvalidProvenance,
                ));
            }
        };
        counts.push((table.to_owned(), count));
        sources.push(ArchiveEntrySource {
            path,
            media_type,
            payload_schema,
            bytes,
        });
    }
    match package_verified_project_archive_zip(
        &archive_root_facts(
            page.export_id.as_str(),
            scope,
            &page.source_snapshot,
            counts,
            &created_at,
        ),
        &sources,
        immutable_root,
    ) {
        Ok(bytes) => Ok(VerifiedExportArchive::Ready(bytes)),
        Err(reason) => Ok(VerifiedExportArchive::Refused(reason)),
    }
}

fn archive_root_facts(
    export_id: &str,
    scope: &ProjectScope,
    snapshot: &CanonicalSnapshot,
    table_family_counts: Vec<(String, String)>,
    created_at: &str,
) -> ProjectArchiveRootFacts {
    ProjectArchiveRootFacts {
        archive_profile: PROJECT_EXPORT_ARCHIVE_PROFILE.to_owned(),
        export_id: export_id.to_owned(),
        owner_user_id: scope.owner_user_id.as_ref().to_owned(),
        project_id: scope.project_id.as_ref().to_owned(),
        snapshot_id: snapshot.snapshot_id.clone(),
        project_activity_position: snapshot.project_activity_position.to_string(),
        minimum_reader: "storyos.schema.1".to_owned(),
        maximum_reader: "storyos.schema.1".to_owned(),
        schema_catalog: "storyos.schema-catalog.v1".to_owned(),
        table_family_counts,
        serialization_profile: ARCHIVE_SERIALIZATION_PROFILE.to_owned(),
        archive_path_profile: ARCHIVE_PATH_PROFILE.to_owned(),
        digest_profile: ARCHIVE_ROOT_DIGEST_PROFILE.to_owned(),
        limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
        provenance_status: "complete".to_owned(),
        provenance_edge_count: "0".to_owned(),
        known_purged_gaps: Vec::new(),
        created_at: created_at.to_owned(),
    }
}

async fn load_table_json(
    client: &tokio_postgres::Client,
    table: &str,
    scope: &ProjectScope,
) -> Result<Vec<serde_json::Value>, ExportProjectArchiveError> {
    let sql = format!(
        "SELECT to_jsonb(source)::text
           FROM storyos.{table} AS source
          WHERE source.owner_user_id = $1::text::uuid
            AND source.project_id = $2::text::uuid"
    );
    let rows = client
        .query(
            &sql,
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(|error| archive_table_error(table, error))?;
    let mut values = Vec::with_capacity(rows.len());
    for row in rows {
        let raw: String = row.get(0);
        let value: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|error| ExportProjectArchiveError::Unavailable(Box::new(error)))?;
        values.push(value);
    }
    values.sort_by_key(canonical_json);
    Ok(values)
}

fn classify_rows(
    rows: &[serde_json::Value],
    scope: &ProjectScope,
) -> Result<(), ExportProjectArchiveError> {
    let owner = scope.owner_user_id.as_ref();
    let project = scope.project_id.as_ref();
    for row in rows {
        let Some(object) = row.as_object() else {
            return Err(archive_build_error(
                ProjectArchiveBuildRefusal::InvalidProvenance,
            ));
        };
        let Some(row_owner) = object
            .get("owner_user_id")
            .and_then(serde_json::Value::as_str)
        else {
            return Err(archive_build_error(
                ProjectArchiveBuildRefusal::InvalidProvenance,
            ));
        };
        let Some(row_project) = object.get("project_id").and_then(serde_json::Value::as_str) else {
            return Err(archive_build_error(
                ProjectArchiveBuildRefusal::InvalidProvenance,
            ));
        };
        classify_export_record(
            row_owner,
            row_project,
            owner,
            project,
            /*lifecycle_eligible*/ true,
            /*payload_present*/ true,
            /*gap_documented*/ false,
        )
        .map_err(archive_build_error)?;
    }
    Ok(())
}

const PERSISTENCE_CATALOG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/foundation/postgresql-release-1-persistence-catalog.json"
));

fn archive_build_error(reason: ProjectArchiveBuildRefusal) -> ExportProjectArchiveError {
    ExportProjectArchiveError::ArchiveBuild(reason)
}

fn archive_table_error(table: &str, error: tokio_postgres::Error) -> ExportProjectArchiveError {
    ExportProjectArchiveError::Unavailable(Box::new(std::io::Error::other(format!(
        "{table}: {error}"
    ))))
}

#[cfg(test)]
#[path = "project_archive_build_tests.rs"]
mod tests;
