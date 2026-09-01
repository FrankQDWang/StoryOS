//! Build one Project Export Archive manifest coverage and immutable root.

use sha2::{Digest, Sha256};

use crate::archive_path::{AdmittedArchivePath, ArchivePathRefusal, admit_archive_path};

pub const PROJECT_EXPORT_ARCHIVE_PROFILE: &str = "storyos.project-export.v1";
pub const ARCHIVE_ENTRY_DIGEST_PROFILE: &str = "storyos.archive-entry.raw-bytes.v1";
pub const ARCHIVE_ROOT_DIGEST_PROFILE: &str = "storyos.project-archive-root.jcs.v1";
pub const ARCHIVE_SERIALIZATION_PROFILE: &str = "storyos.project-archive-json.jcs.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveEntrySource {
    pub path: String,
    pub media_type: String,
    pub payload_schema: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveEntryDescriptor {
    pub path: String,
    pub media_type: String,
    pub payload_schema: String,
    pub byte_length: String,
    pub digest_hex: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectArchiveRootFacts {
    pub archive_profile: String,
    pub export_id: String,
    pub owner_user_id: String,
    pub project_id: String,
    pub snapshot_id: String,
    pub project_activity_position: String,
    pub minimum_reader: String,
    pub maximum_reader: String,
    pub schema_catalog: String,
    pub table_family_counts: Vec<(String, String)>,
    pub serialization_profile: String,
    pub archive_path_profile: String,
    pub digest_profile: String,
    pub limit_profile_revision: String,
    pub provenance_status: String,
    pub provenance_edge_count: String,
    pub known_purged_gaps: Vec<serde_json::Value>,
    pub created_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltProjectArchive {
    pub entries: Vec<ArchiveEntryDescriptor>,
    pub root_digest_hex: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectArchiveBuildRefusal {
    InvalidPath(ArchivePathRefusal),
    Collision,
    DirectoryPrefixCollision,
    CorruptDigest,
    ForeignMaterial,
    MissingFamily,
    IneligibleLifecycle,
    InvalidProvenance,
}

/// Admit, order, digest, and cover entries, then hash the closed root input.
pub fn build_project_archive(
    facts: &ProjectArchiveRootFacts,
    sources: &[ArchiveEntrySource],
) -> Result<BuiltProjectArchive, ProjectArchiveBuildRefusal> {
    if facts.archive_profile != PROJECT_EXPORT_ARCHIVE_PROFILE
        || facts.archive_path_profile != crate::archive_path::ARCHIVE_PATH_PROFILE
        || facts.digest_profile != ARCHIVE_ROOT_DIGEST_PROFILE
        || facts.serialization_profile != ARCHIVE_SERIALIZATION_PROFILE
    {
        return Err(ProjectArchiveBuildRefusal::InvalidProvenance);
    }
    let mut admitted: Vec<(AdmittedArchivePath, &ArchiveEntrySource)> =
        Vec::with_capacity(sources.len());
    for source in sources {
        let path =
            admit_archive_path(&source.path).map_err(ProjectArchiveBuildRefusal::InvalidPath)?;
        admitted.push((path, source));
    }
    admitted.sort_by(|left, right| left.0.path.as_bytes().cmp(right.0.path.as_bytes()));
    let mut collision_keys = std::collections::BTreeSet::new();
    let mut paths = std::collections::BTreeSet::new();
    let mut descriptors = Vec::with_capacity(admitted.len());
    for (path, source) in &admitted {
        if !collision_keys.insert(path.collision_key.clone()) {
            return Err(ProjectArchiveBuildRefusal::Collision);
        }
        if !paths.insert(path.path.clone()) {
            return Err(ProjectArchiveBuildRefusal::Collision);
        }
        for existing in &paths {
            if existing != &path.path
                && (path.path.starts_with(&format!("{existing}/"))
                    || existing.starts_with(&format!("{}/", path.path)))
            {
                return Err(ProjectArchiveBuildRefusal::DirectoryPrefixCollision);
            }
        }
        let digest_hex = hex_sha256(&source.bytes);
        descriptors.push(ArchiveEntryDescriptor {
            path: path.path.clone(),
            media_type: source.media_type.clone(),
            payload_schema: source.payload_schema.clone(),
            byte_length: source.bytes.len().to_string(),
            digest_hex,
        });
    }
    let root_digest_hex =
        hex_sha256(canonical_json(&root_input_json(facts, &descriptors)).as_bytes());
    Ok(BuiltProjectArchive {
        entries: descriptors,
        root_digest_hex,
    })
}

/// Recompute every entry digest and the immutable root before ZIP STORE bytes.
pub fn package_verified_project_archive_zip(
    facts: &ProjectArchiveRootFacts,
    sources: &[ArchiveEntrySource],
    expected_root_hex: &str,
) -> Result<Vec<u8>, ProjectArchiveBuildRefusal> {
    let expected = expected_root_hex
        .strip_prefix("sha256:")
        .unwrap_or(expected_root_hex);
    if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ProjectArchiveBuildRefusal::InvalidProvenance);
    }
    let built = build_project_archive(facts, sources)?;
    if built.root_digest_hex != expected {
        return Err(ProjectArchiveBuildRefusal::CorruptDigest);
    }
    let mut files = Vec::with_capacity(built.entries.len());
    for descriptor in &built.entries {
        let Some(source) = sources.iter().find(|source| source.path == descriptor.path) else {
            return Err(ProjectArchiveBuildRefusal::InvalidProvenance);
        };
        verify_entry_digest(&source.bytes, &descriptor.digest_hex)?;
        files.push((descriptor.path.as_str(), source.bytes.as_slice()));
    }
    crate::archive_zip::store_zip(&files)
}

/// Classify one supplied digest against the exact uncompressed entry bytes.
pub fn verify_entry_digest(
    bytes: &[u8],
    digest_hex: &str,
) -> Result<(), ProjectArchiveBuildRefusal> {
    if hex_sha256(bytes) == digest_hex {
        Ok(())
    } else {
        Err(ProjectArchiveBuildRefusal::CorruptDigest)
    }
}

/// Classify one candidate archive record against exact Scope, lifecycle, and gap facts.
pub fn classify_export_record(
    row_owner_user_id: &str,
    row_project_id: &str,
    scope_owner_user_id: &str,
    scope_project_id: &str,
    lifecycle_eligible: bool,
    payload_present: bool,
    gap_documented: bool,
) -> Result<(), ProjectArchiveBuildRefusal> {
    if row_owner_user_id != scope_owner_user_id || row_project_id != scope_project_id {
        return Err(ProjectArchiveBuildRefusal::ForeignMaterial);
    }
    if !lifecycle_eligible {
        return Err(ProjectArchiveBuildRefusal::IneligibleLifecycle);
    }
    if !payload_present && !gap_documented {
        return Err(ProjectArchiveBuildRefusal::InvalidProvenance);
    }
    Ok(())
}

/// Require every currently delivered exportable family to appear in the archive.
pub fn require_delivered_families(
    present: &[&str],
    required: &[&str],
) -> Result<(), ProjectArchiveBuildRefusal> {
    for family in required {
        if !present.iter().any(|name| name == family) {
            return Err(ProjectArchiveBuildRefusal::MissingFamily);
        }
    }
    Ok(())
}

pub fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut value, byte| {
            use std::fmt::Write as _;
            write!(value, "{byte:02x}").expect("writing to String cannot fail");
            value
        })
}

pub fn canonical_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_owned(),
        serde_json::Value::Bool(true) => "true".to_owned(),
        serde_json::Value::Bool(false) => "false".to_owned(),
        serde_json::Value::Number(number) => number.to_string(),
        serde_json::Value::String(text) => {
            serde_json::to_string(text).expect("a JSON string is serializable")
        }
        serde_json::Value::Array(values) => {
            let body = values
                .iter()
                .map(canonical_json)
                .collect::<Vec<_>>()
                .join(",");
            format!("[{body}]")
        }
        serde_json::Value::Object(fields) => {
            let mut keys: Vec<&String> = fields.keys().collect();
            keys.sort();
            let body = keys
                .into_iter()
                .map(|key| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(key).expect("a JSON object key is serializable"),
                        canonical_json(&fields[key])
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{body}}}")
        }
    }
}

fn root_input_json(
    facts: &ProjectArchiveRootFacts,
    entries: &[ArchiveEntryDescriptor],
) -> serde_json::Value {
    let mut counts = serde_json::Map::new();
    for (family, count) in &facts.table_family_counts {
        counts.insert(family.clone(), serde_json::Value::String(count.clone()));
    }
    let entry_values = entries
        .iter()
        .map(|entry| {
            serde_json::json!({
                "path": entry.path,
                "media_type": entry.media_type,
                "payload_schema": entry.payload_schema,
                "byte_length": entry.byte_length,
                "digest": {
                    "algorithm": "sha256",
                    "profile": ARCHIVE_ENTRY_DIGEST_PROFILE,
                    "value_hex_lowercase": entry.digest_hex,
                }
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "archive_profile": facts.archive_profile,
        "export_id": facts.export_id,
        "project_scope": {
            "owner_user_id": facts.owner_user_id,
            "project_id": facts.project_id,
        },
        "source_snapshot": {
            "snapshot_id": facts.snapshot_id,
            "project_activity_position": facts.project_activity_position,
        },
        "source_schema_compatibility": {
            "minimum_reader": facts.minimum_reader,
            "maximum_reader": facts.maximum_reader,
        },
        "schema_catalog": facts.schema_catalog,
        "table_family_counts": counts,
        "serialization_profile": facts.serialization_profile,
        "archive_path_profile": facts.archive_path_profile,
        "digest_profile": facts.digest_profile,
        "limit_profile_revision": facts.limit_profile_revision,
        "entries": entry_values,
        "provenance_closure": {
            "status": facts.provenance_status,
            "edge_count": facts.provenance_edge_count,
        },
        "known_purged_gaps": facts.known_purged_gaps,
        "created_at": facts.created_at,
    })
}

#[cfg(test)]
#[path = "project_archive_tests.rs"]
mod tests;
