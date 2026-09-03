use super::*;
use crate::archive_path::ARCHIVE_PATH_PROFILE;

const PROTOCOL_ENTRY_BYTES: &str =
    r#"{"project_id":"018f0000-0000-7001-8000-000000000002","schema":"storyos.project-record.v1"}"#;
const PROTOCOL_ENTRY_DIGEST: &str =
    "e2e04dc008f68b1c35338884373c8c3a1f46ee1cb591dbddc6de1a782c682ba2";
const PROTOCOL_ROOT_DIGEST: &str =
    "2780d69d6d43249632da2b459060b571a5695347e126e98fe743d717e492adbf";

fn protocol_facts() -> ProjectArchiveRootFacts {
    ProjectArchiveRootFacts {
        archive_profile: PROJECT_EXPORT_ARCHIVE_PROFILE.to_owned(),
        export_id: "018f0000-0000-7001-8000-000000000071".to_owned(),
        owner_user_id: "018f0000-0000-7001-8000-000000000001".to_owned(),
        project_id: "018f0000-0000-7001-8000-000000000002".to_owned(),
        snapshot_id: "018f0000-0000-7001-8000-000000000072".to_owned(),
        project_activity_position: "44".to_owned(),
        minimum_reader: "storyos.schema.1".to_owned(),
        maximum_reader: "storyos.schema.1".to_owned(),
        schema_catalog: "storyos.schema-catalog.v1".to_owned(),
        table_family_counts: vec![("projects".to_owned(), "1".to_owned())],
        serialization_profile: ARCHIVE_SERIALIZATION_PROFILE.to_owned(),
        archive_path_profile: ARCHIVE_PATH_PROFILE.to_owned(),
        digest_profile: ARCHIVE_ROOT_DIGEST_PROFILE.to_owned(),
        limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
        provenance_status: "complete".to_owned(),
        provenance_edge_count: "0".to_owned(),
        known_purged_gaps: Vec::new(),
        created_at: "2026-07-22T09:00:00.000Z".to_owned(),
    }
}

#[test]
fn protocol_example_entry_and_root_digests_are_golden() {
    assert_eq!(PROTOCOL_ENTRY_BYTES.len(), 90);
    verify_entry_digest(PROTOCOL_ENTRY_BYTES.as_bytes(), PROTOCOL_ENTRY_DIGEST)
        .expect("protocol entry digest");
    let built = build_project_archive(
        &protocol_facts(),
        &[ArchiveEntrySource {
            path: "canonical/project.json".to_owned(),
            media_type: "application/json".to_owned(),
            payload_schema: "storyos.project-record.v1".to_owned(),
            bytes: PROTOCOL_ENTRY_BYTES.as_bytes().to_vec(),
        }],
    )
    .expect("protocol archive");
    assert_eq!(built.root_digest_hex, PROTOCOL_ROOT_DIGEST);
    assert_eq!(built.entries[0].digest_hex, PROTOCOL_ENTRY_DIGEST);
}

#[test]
fn corrupt_entry_digest_is_a_typed_terminal_failure() {
    assert_eq!(
        verify_entry_digest(PROTOCOL_ENTRY_BYTES.as_bytes(), "00".repeat(32).as_str()),
        Err(ProjectArchiveBuildRefusal::CorruptDigest)
    );
}

#[test]
fn case_colliding_paths_are_refused() {
    let source = ArchiveEntrySource {
        path: "canonical/project.json".to_owned(),
        media_type: "application/json".to_owned(),
        payload_schema: "storyos.project-record.v1".to_owned(),
        bytes: b"{}".to_vec(),
    };
    let colliding = ArchiveEntrySource {
        path: "canonical/Project.json".to_owned(),
        media_type: "application/json".to_owned(),
        payload_schema: "storyos.project-record.v1".to_owned(),
        bytes: b"[]".to_vec(),
    };
    assert_eq!(
        build_project_archive(&protocol_facts(), &[source, colliding]),
        Err(ProjectArchiveBuildRefusal::Collision)
    );
}

#[test]
fn file_path_that_equals_a_directory_prefix_is_refused() {
    let file = ArchiveEntrySource {
        path: "canonical/project.json".to_owned(),
        media_type: "application/json".to_owned(),
        payload_schema: "storyos.project-record.v1".to_owned(),
        bytes: b"{}".to_vec(),
    };
    let prefix = ArchiveEntrySource {
        path: "canonical".to_owned(),
        media_type: "application/json".to_owned(),
        payload_schema: "storyos.project-record.v1".to_owned(),
        bytes: b"[]".to_vec(),
    };
    assert_eq!(
        build_project_archive(&protocol_facts(), &[file, prefix]),
        Err(ProjectArchiveBuildRefusal::DirectoryPrefixCollision)
    );
}

#[test]
fn unsigned_byte_order_is_the_sort_key() {
    let later = ArchiveEntrySource {
        path: "canonical/z.json".to_owned(),
        media_type: "application/json".to_owned(),
        payload_schema: "storyos.project-record.v1".to_owned(),
        bytes: b"1".to_vec(),
    };
    let earlier = ArchiveEntrySource {
        path: "canonical/a.json".to_owned(),
        media_type: "application/json".to_owned(),
        payload_schema: "storyos.project-record.v1".to_owned(),
        bytes: b"0".to_vec(),
    };
    let built = build_project_archive(&protocol_facts(), &[later, earlier]).expect("ordered");
    assert_eq!(built.entries[0].path, "canonical/a.json");
    assert_eq!(built.entries[1].path, "canonical/z.json");
}

#[test]
fn foreign_missing_lifecycle_and_provenance_are_typed_failures() {
    let owner = "018f0000-0000-7001-8000-000000000001";
    let project = "018f0000-0000-7001-8000-000000000002";
    assert_eq!(
        classify_export_record(
            "018f0000-0000-7001-8000-000000000101",
            project,
            owner,
            project,
            /*lifecycle_eligible*/ true,
            /*payload_present*/ true,
            /*gap_documented*/ false,
        ),
        Err(ProjectArchiveBuildRefusal::ForeignMaterial)
    );
    assert_eq!(
        classify_export_record(
            owner, project, owner, project, /*lifecycle_eligible*/ false,
            /*payload_present*/ true, /*gap_documented*/ false,
        ),
        Err(ProjectArchiveBuildRefusal::IneligibleLifecycle)
    );
    assert_eq!(
        classify_export_record(
            owner, project, owner, project, /*lifecycle_eligible*/ true,
            /*payload_present*/ false, /*gap_documented*/ false,
        ),
        Err(ProjectArchiveBuildRefusal::InvalidProvenance)
    );
    classify_export_record(
        owner, project, owner, project, /*lifecycle_eligible*/ true,
        /*payload_present*/ false, /*gap_documented*/ true,
    )
    .expect("a documented gap stays a gap, not a silent omission");
    classify_export_record(
        owner, project, owner, project, /*lifecycle_eligible*/ true,
        /*payload_present*/ true, /*gap_documented*/ false,
    )
    .expect("in-scope payload is eligible");
    assert_eq!(
        require_delivered_families(&["projects"], &["projects", "manuscript_objects"]),
        Err(ProjectArchiveBuildRefusal::MissingFamily)
    );
}

#[test]
fn catalog_include_names_ignore_other_export_policies() {
    let catalog = serde_json::json!({
        "families": [
            {
                "table_families": ["projects", "anti_forgery_challenges"],
                "portability": {"project_export": "include"}
            },
            {
                "table_families": ["project_command_challenges"],
                "portability": {"project_export": "exclude"}
            },
            {
                "table_families": ["project_snapshots"],
                "portability": {
                    "project_export": "include_if_required_for_historical_evidence"
                }
            }
        ]
    });
    assert_eq!(
        catalog_include_table_names(&catalog),
        vec!["anti_forgery_challenges".to_owned(), "projects".to_owned(),]
    );
}

#[test]
fn required_export_tables_are_live_catalog_include_names() {
    let required = required_export_tables(
        &[
            "projects",
            "author_command_admission_reconfirmations",
            "anti_forgery_challenges",
            "agent_runs",
        ],
        &[
            "projects",
            "author_command_admission_reconfirmations",
            "project_command_challenges",
        ],
    );
    assert_eq!(
        required,
        vec![
            "author_command_admission_reconfirmations".to_owned(),
            "projects".to_owned(),
        ]
    );
    let present = ["projects"];
    let required_refs: Vec<&str> = required.iter().map(String::as_str).collect();
    assert_eq!(
        require_delivered_families(&present, &required_refs),
        Err(ProjectArchiveBuildRefusal::MissingFamily)
    );
}

fn protocol_source() -> ArchiveEntrySource {
    ArchiveEntrySource {
        path: "canonical/project.json".to_owned(),
        media_type: "application/json".to_owned(),
        payload_schema: "storyos.project-record.v1".to_owned(),
        bytes: PROTOCOL_ENTRY_BYTES.as_bytes().to_vec(),
    }
}

#[test]
fn a_missing_root_is_unsettled_and_exposes_no_zip_bytes() {
    assert_eq!(
        package_verified_project_archive_zip(&protocol_facts(), &[protocol_source()], ""),
        Err(ProjectArchiveBuildRefusal::InvalidProvenance)
    );
}

#[test]
fn a_corrupt_payload_is_refused_before_zip_bytes() {
    let built = build_project_archive(&protocol_facts(), &[protocol_source()]).expect("built");
    let mut tampered = protocol_source();
    tampered.bytes = b"tampered".to_vec();
    assert_eq!(
        package_verified_project_archive_zip(
            &protocol_facts(),
            &[tampered],
            &built.root_digest_hex,
        ),
        Err(ProjectArchiveBuildRefusal::CorruptDigest)
    );
}

#[test]
fn verified_archive_zip_repeats_and_contains_uncompressed_entry_bytes() {
    let built = build_project_archive(&protocol_facts(), &[protocol_source()]).expect("built");
    let first = package_verified_project_archive_zip(
        &protocol_facts(),
        &[protocol_source()],
        &built.root_digest_hex,
    )
    .expect("verified zip");
    let second = package_verified_project_archive_zip(
        &protocol_facts(),
        &[protocol_source()],
        &built.root_digest_hex,
    )
    .expect("repeat zip");
    assert_eq!(first, second);
    assert!(first.starts_with(b"PK\x03\x04"));
    assert!(
        first
            .windows(PROTOCOL_ENTRY_BYTES.len())
            .any(|window| { window == PROTOCOL_ENTRY_BYTES.as_bytes() })
    );
    assert!(
        first
            .windows(b"canonical/project.json".len())
            .any(|window| { window == b"canonical/project.json" })
    );
}
