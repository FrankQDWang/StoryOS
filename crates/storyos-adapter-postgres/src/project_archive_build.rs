use storyos_application::{
    CanonicalSnapshot, ExportProjectArchiveCommand, ExportProjectArchiveError, ProjectScope,
};
use storyos_core::{
    ARCHIVE_PATH_PROFILE, ARCHIVE_ROOT_DIGEST_PROFILE, ARCHIVE_SERIALIZATION_PROFILE,
    ArchiveEntrySource, PROJECT_EXPORT_ARCHIVE_PROFILE, ProjectArchiveBuildRefusal,
    ProjectArchiveRootFacts, build_project_archive, canonical_json, classify_export_record,
    require_delivered_families,
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
    command: &ExportProjectArchiveCommand,
    snapshot: &CanonicalSnapshot,
    created_at: &str,
) -> Result<String, ExportProjectArchiveError> {
    let scope = &command.project_scope;
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
    let required: Vec<&str> = EXPORT_TABLES.iter().map(|(table, _)| *table).collect();
    require_delivered_families(&present, &required).map_err(archive_build_error)?;
    let built = build_project_archive(
        &ProjectArchiveRootFacts {
            archive_profile: PROJECT_EXPORT_ARCHIVE_PROFILE.to_owned(),
            export_id: command.export_id.clone(),
            owner_user_id: scope.owner_user_id.as_ref().to_owned(),
            project_id: scope.project_id.as_ref().to_owned(),
            snapshot_id: snapshot.snapshot_id.clone(),
            project_activity_position: snapshot.project_activity_position.to_string(),
            minimum_reader: "storyos.schema.1".to_owned(),
            maximum_reader: "storyos.schema.1".to_owned(),
            schema_catalog: "storyos.schema-catalog.v1".to_owned(),
            table_family_counts: counts,
            serialization_profile: ARCHIVE_SERIALIZATION_PROFILE.to_owned(),
            archive_path_profile: ARCHIVE_PATH_PROFILE.to_owned(),
            digest_profile: ARCHIVE_ROOT_DIGEST_PROFILE.to_owned(),
            limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
            provenance_status: "complete".to_owned(),
            provenance_edge_count: "0".to_owned(),
            known_purged_gaps: Vec::new(),
            created_at: created_at.to_owned(),
        },
        &sources,
    )
    .map_err(archive_build_error)?;
    for (source, descriptor) in sources.iter().zip(built.entries.iter()) {
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
                    &command.export_id,
                    &descriptor.path,
                    &descriptor.media_type,
                    &descriptor.payload_schema,
                    &descriptor.byte_length,
                    &format!("sha256:{}", descriptor.digest_hex),
                    &source.bytes.as_slice(),
                ],
            )
            .await
            .map_err(archive_database_error)?;
    }
    let immutable_root = format!("sha256:{}", built.root_digest_hex);
    client
        .execute(
            "UPDATE storyos.project_export_manifests
                SET immutable_root = $4
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND export_id = $3::text::uuid AND immutable_root IS NULL",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &command.export_id,
                &immutable_root,
            ],
        )
        .await
        .map_err(archive_database_error)?;
    Ok(immutable_root)
}

async fn load_table_json(
    client: &tokio_postgres::Client,
    table: &str,
    scope: &ProjectScope,
) -> Result<Vec<serde_json::Value>, ExportProjectArchiveError> {
    let sql = format!(
        "SELECT COALESCE(json_agg(row_json ORDER BY row_json::text), '[]'::json)::text
           FROM (
             SELECT to_jsonb(source) AS row_json
               FROM storyos.{table} AS source
              WHERE source.owner_user_id = $1::text::uuid
                AND source.project_id = $2::text::uuid
           ) AS ordered"
    );
    let raw: String = client
        .query_one(
            &sql,
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(archive_database_error)?
        .get(0);
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|error| ExportProjectArchiveError::Unavailable(Box::new(error)))?;
    match value {
        serde_json::Value::Array(rows) => Ok(rows),
        _ => Err(archive_build_error(
            ProjectArchiveBuildRefusal::InvalidProvenance,
        )),
    }
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
        let row_owner = object
            .get("owner_user_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(owner);
        let row_project = object
            .get("project_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(project);
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

fn archive_build_error(reason: ProjectArchiveBuildRefusal) -> ExportProjectArchiveError {
    ExportProjectArchiveError::ArchiveBuild(reason)
}

fn archive_database_error(error: tokio_postgres::Error) -> ExportProjectArchiveError {
    ExportProjectArchiveError::Unavailable(Box::new(error))
}
