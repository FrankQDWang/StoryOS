use sha2::{Digest, Sha256};

use super::{
    EMBEDDED_SOURCES, EmbeddedSource, PackagedIdentity, StorageActivation, identity, later_sources,
    lf_sha256, packaged_identity,
};

const GENERATED_ROUTE_CATALOG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../generated/route-catalog/versioned-protocol-release-1-route-catalog.json"
));

fn sha256(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{digest}")
}

/// Each SQL file of the migrations directory, in file name order, with its LF-normalized checksum.
fn migrations_directory() -> Vec<(String, String)> {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let mut names: Vec<String> = std::fs::read_dir(&directory)
        .expect("read migrations directory")
        .map(|entry| entry.expect("migrations entry").file_name())
        .map(|name| name.into_string().expect("migration name is UTF-8"))
        .filter(|name| name.ends_with(".sql"))
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let sql = std::fs::read_to_string(directory.join(&name)).expect("read migration");
            (
                format!("crates/storyos-adapter-postgres/migrations/{name}"),
                lf_sha256(&sql),
            )
        })
        .collect()
}

#[test]
fn the_packaged_identity_derives_from_the_migrations_directory_and_the_route_catalog() {
    let sources = migrations_directory();
    let chain_digest = sha256(
        &sources
            .iter()
            .map(|(path, checksum)| format!("{path}\n{checksum}\n"))
            .collect::<String>(),
    );
    let embedded: Vec<(String, String)> = EMBEDDED_SOURCES
        .iter()
        .map(|source| (source.path.to_owned(), source.lf_sha256.to_owned()))
        .collect();

    assert_eq!(embedded, sources);
    assert_eq!(
        packaged_identity(),
        PackagedIdentity {
            catalog_id: "storyos.persistence.release-1".to_owned(),
            catalog_checksum: chain_digest.clone(),
            migration_chain_id: "storyos.persistence.bootstrap.release-1.v3".to_owned(),
            migration_chain_digest: chain_digest.clone(),
            database_schema_identity: chain_digest,
            active_schema_version: "storyos.persistence.release-1.v3".to_owned(),
            public_release: "storyos.public.release.1".to_owned(),
            route_catalog_id: "storyos.public.route-catalog.release-1.v1".to_owned(),
            route_catalog_sha256: sha256(GENERATED_ROUTE_CATALOG),
        }
    );
}

#[test]
fn a_changed_generated_route_catalog_changes_the_packaged_identity() {
    let packaged = packaged_identity();
    let changed_catalog = GENERATED_ROUTE_CATALOG.replacen(
        "\"error_profile\": \"query_protocol\"",
        "\"error_profile\": \"query_canonical\"",
        1,
    );
    assert_ne!(changed_catalog, GENERATED_ROUTE_CATALOG);

    let changed = identity(&packaged.migration_chain_digest, &changed_catalog);

    assert_ne!(changed.route_catalog_sha256, packaged.route_catalog_sha256);
    assert_ne!(
        changed, packaged,
        "a stored identity of the changed catalog must refuse with identity_mismatch"
    );
}

fn recorded(sources: &[EmbeddedSource]) -> Vec<(String, String)> {
    sources
        .iter()
        .map(|source| (source.path.to_owned(), source.lf_sha256.to_owned()))
        .collect()
}

#[test]
fn the_files_after_a_recorded_prefix_are_the_later_files_of_the_package() {
    let package = EMBEDDED_SOURCES;
    let earlier = &package[..package.len() - 1];

    let later = later_sources(&recorded(earlier), package).expect("a prefix");
    assert_eq!(recorded(later), recorded(&package[package.len() - 1..]));
    assert!(
        later_sources(&recorded(package), package)
            .expect("the same files")
            .is_empty()
    );
}

#[test]
fn recorded_files_that_are_not_a_prefix_of_the_package_give_no_later_files() {
    let package = EMBEDDED_SOURCES;
    let earlier = &package[..package.len() - 1];
    // An older package does not have each recorded file.
    assert!(later_sources(&recorded(package), earlier).is_none());
    let mut changed_checksum = recorded(earlier);
    changed_checksum[1].1 = format!("sha256:{}", "1".repeat(64));
    assert!(later_sources(&changed_checksum, package).is_none());
    let mut changed_path = recorded(earlier);
    changed_path[1].0.push_str(".old");
    assert!(later_sources(&changed_path, package).is_none());
    assert!(later_sources(&[], package).is_none());
}

#[test]
fn an_upgrade_prints_one_more_line_with_the_counts_of_the_files() {
    assert_eq!(
        StorageActivation::Active.to_string(),
        "STORYOS_STORAGE_ACTIVATION=Active"
    );
    assert_eq!(
        StorageActivation::AlreadyActive.to_string(),
        "STORYOS_STORAGE_ACTIVATION=Active"
    );
    assert_eq!(
        StorageActivation::Upgraded {
            from_files: 2,
            to_files: 3
        }
        .to_string(),
        "STORYOS_STORAGE_ACTIVATION=Active\nSTORYOS_STORAGE_UPGRADE=from 2 files to 3 files"
    );
}

/// A forward file runs as the admin login in an upgrade and as `storyos_owner` after the baseline
/// in a first activation, so it sets its role first (CODING_STANDARDS.md, section Migrations).
#[test]
fn each_forward_file_sets_its_role_as_its_first_statement() {
    let (baseline, forward) = EMBEDDED_SOURCES.split_at(2);
    assert!(baseline[0].path.ends_with("/0000_roles.sql"));
    assert!(baseline[1].path.ends_with("/0001_release_1_baseline.sql"));
    assert!(!forward.is_empty());
    for source in forward {
        let first_statement = source
            .sql
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty() && !line.starts_with("--"))
            .unwrap_or_default();
        assert!(
            ["RESET ROLE;", "SET LOCAL ROLE storyos_owner;"].contains(&first_statement),
            "{}: {first_statement}",
            source.path
        );
    }
}
