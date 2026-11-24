//! The upgrade edge of the Storage Activation on a real database (ADR 0104 section 4). The roles of
//! `0000_roles.sql` are of the cluster, so the test needs a new cluster: the container of the
//! controlled image, or the cluster of `STORYOS_TEST_UPGRADE_ADMIN_URL`, which the lane
//! `provider-activation` gives on the image of the hosted provider.

use sha2::{Digest, Sha256};
use tokio_postgres::NoTls;
use tokio_postgres::error::SqlState;

use crate::statement_cache::Client;

use super::{
    EMBEDDED_SOURCES, EmbeddedSource, PackagedIdentity, ROUTE_CATALOG_JSON, StorageActivation,
    StorageActivationError, StorageActivationRefusal, activate_sources, identity, lf_sha256,
    packaged_identity, read_proof,
};
use crate::PostgresProjectReader;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_structure::{titled_chapter, titled_volume};
use crate::shared_tests::command_sequence_support::seed_owned_project;
use crate::shared_tests::postgres_container::PostgresContainer;

const ADMIN_URL: &str = "STORYOS_TEST_UPGRADE_ADMIN_URL";
const OWNER: &str = "018f0000-0000-7001-8000-000000000001";
/// A new file that fails at its second statement.
const FAILING_FILE: &str =
    "CREATE TABLE storyos.upgrade_probe (probe_id integer);\nSELECT 1 / 0;\n";

#[tokio::test]
#[ignore = "run through make check"]
async fn an_active_database_of_an_earlier_package_takes_the_upgrade_edge() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let container = std::env::var(ADMIN_URL)
        .is_err()
        .then(|| PostgresContainer::start("storyos-upgrade-test"));
    let admin_url = container.as_ref().map_or_else(
        || std::env::var(ADMIN_URL).unwrap(),
        PostgresContainer::admin_url,
    );
    let earlier_files = &EMBEDDED_SOURCES[..EMBEDDED_SOURCES.len() - 1];
    let earlier = package_identity(earlier_files, ROUTE_CATALOG_JSON);
    assert_eq!(
        activate_sources(&admin_url, &earlier, earlier_files)
            .await
            .unwrap(),
        StorageActivation::Active
    );
    let admin = connect(&admin_url).await;
    let (project_id, chapter_id) = write_project_with_chapter(&admin, &admin_url).await;

    refusals_keep_the_proof_and_the_ledger(&admin, &admin_url, earlier_files).await;
    a_failing_new_file_keeps_the_earlier_state(&admin, &admin_url, &earlier, earlier_files).await;

    let full = EMBEDDED_SOURCES.len();
    assert_eq!(
        activate_sources(&admin_url, &packaged_identity(), EMBEDDED_SOURCES)
            .await
            .unwrap(),
        StorageActivation::Upgraded {
            from_files: full - 1,
            to_files: full
        }
    );
    assert_eq!(read_proof(&admin).await.unwrap(), Some(packaged_identity()));
    let titles: (String, String) = admin
        .query_one(
            "SELECT project.title, chapter.title
               FROM storyos.projects AS project
               JOIN storyos.manuscript_objects AS chapter USING (owner_user_id, project_id)
              WHERE project.project_id = $1::text::uuid
                AND chapter.manuscript_object_id = $2::text::uuid",
            &[&project_id, &chapter_id],
        )
        .await
        .map(|row| (row.get(0), row.get(1)))
        .unwrap();
    assert_eq!(titles, ("Empty Novel".to_owned(), "Chapter A".to_owned()));
    let runtime = connect(&runtime_url(&admin_url)).await;
    runtime.simple_query("SELECT 1").await.unwrap();
    let (_, later_files) = EMBEDDED_SOURCES.split_at(earlier_files.len());
    let expected_files: Vec<(i64, String)> = [(1, earlier_files), (2, later_files)]
        .into_iter()
        .flat_map(|(attempt_id, files)| {
            files
                .iter()
                .map(move |source| (attempt_id, source.path.to_owned()))
        })
        .collect();
    assert_eq!(attempts_of_files(&admin).await, expected_files);
    assert_eq!(attempt_count(&admin).await, 2);
    let upgrade_phases: i64 = admin
        .query_one(
            "SELECT count(*) FROM storyos.migration_phases WHERE attempt_id = 2",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(upgrade_phases, 4);

    // A second run accepts the new identity and records no attempt.
    assert_eq!(
        activate_sources(&admin_url, &packaged_identity(), EMBEDDED_SOURCES)
            .await
            .unwrap(),
        StorageActivation::AlreadyActive
    );
    assert_eq!(attempt_count(&admin).await, 2);

    // The package of the earlier release does not have each recorded file.
    let state = ledger_state(&admin).await;
    assert_identity_mismatch(
        activate_sources(&admin_url, &earlier, earlier_files).await,
        "a recorded file that the package does not have",
    );
    assert_eq!(ledger_state(&admin).await, state);

    // The same files with another route catalog: a new attempt and a new proof, and no file row.
    let other_catalog = ROUTE_CATALOG_JSON.replacen(
        "\"error_profile\": \"query_protocol\"",
        "\"error_profile\": \"query_canonical\"",
        1,
    );
    assert_ne!(other_catalog, ROUTE_CATALOG_JSON);
    let other = package_identity(EMBEDDED_SOURCES, &other_catalog);
    assert_eq!(
        activate_sources(&admin_url, &other, EMBEDDED_SOURCES)
            .await
            .unwrap(),
        StorageActivation::Upgraded {
            from_files: full,
            to_files: full
        }
    );
    assert_eq!(read_proof(&admin).await.unwrap(), Some(other));
    assert_eq!(attempt_count(&admin).await, 3);
    assert_eq!(attempts_of_files(&admin).await.len(), full);
}

/// Each refusal of the edge is `identity_mismatch` and changes no row of the ledger and the proof.
async fn refusals_keep_the_proof_and_the_ledger(
    admin: &Client,
    admin_url: &str,
    earlier_files: &[EmbeddedSource],
) {
    let baseline = earlier_files[1].path;
    let checksum = earlier_files[1].lf_sha256;
    let other_checksum = format!("sha256:{}", "1".repeat(64));
    let cases = [
        (
            "a recorded file with another checksum",
            format!(
                "UPDATE storyos.migration_phase_checksums SET checksum = '{other_checksum}'
                  WHERE source_path = '{baseline}'"
            ),
            format!(
                "UPDATE storyos.migration_phase_checksums SET checksum = '{checksum}'
                  WHERE source_path = '{baseline}'"
            ),
        ),
        (
            "another catalog identifier",
            "UPDATE storyos.storage_activation_proofs SET catalog_id = 'storyos.persistence.other'"
                .to_owned(),
            format!(
                "UPDATE storyos.storage_activation_proofs SET catalog_id = '{}'",
                super::CATALOG_ID
            ),
        ),
    ];
    for (case, change, restore) in cases {
        admin.batch_execute(&change).await.unwrap();
        let state = ledger_state(admin).await;
        assert_identity_mismatch(
            activate_sources(admin_url, &packaged_identity(), EMBEDDED_SOURCES).await,
            case,
        );
        assert_eq!(ledger_state(admin).await, state, "{case}");
        admin.batch_execute(&restore).await.unwrap();
    }
}

/// A failure inside a new file rolls the upgrade back, so the earlier package still starts.
async fn a_failing_new_file_keeps_the_earlier_state(
    admin: &Client,
    admin_url: &str,
    earlier: &PackagedIdentity,
    earlier_files: &[EmbeddedSource],
) {
    let mut files: Vec<EmbeddedSource> = earlier_files
        .iter()
        .map(|source| EmbeddedSource {
            path: source.path,
            lf_sha256: source.lf_sha256,
            sql: source.sql,
        })
        .collect();
    files.push(EmbeddedSource {
        path: "crates/storyos-adapter-postgres/migrations/0002_failing.sql",
        lf_sha256: Box::leak(lf_sha256(FAILING_FILE).into_boxed_str()),
        sql: FAILING_FILE,
    });
    let state = ledger_state(admin).await;
    let outcome = activate_sources(
        admin_url,
        &package_identity(&files, ROUTE_CATALOG_JSON),
        &files,
    )
    .await;
    assert!(
        matches!(
            &outcome,
            Err(StorageActivationError::Unavailable(source))
                if source
                    .downcast_ref::<tokio_postgres::Error>()
                    .and_then(tokio_postgres::Error::code)
                    == Some(&SqlState::DIVISION_BY_ZERO)
        ),
        "the second statement must fail: {outcome:?}"
    );
    assert_eq!(ledger_state(admin).await, state);
    assert_eq!(read_proof(admin).await.unwrap().as_ref(), Some(earlier));
    let probe: Option<String> = admin
        .query_one("SELECT to_regclass('storyos.upgrade_probe')::text", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(probe, None);
}

/// Sets the password of the runtime role and writes one Project with one Chapter through the store.
async fn write_project_with_chapter(admin: &Client, admin_url: &str) -> (String, String) {
    admin
        .batch_execute(&format!(
            "ALTER ROLE storyos_runtime PASSWORD 'runtime';
             INSERT INTO storyos.users (user_id) VALUES ('{OWNER}');"
        ))
        .await
        .unwrap();
    let store = PostgresProjectReader::new(runtime_url(admin_url));
    let scope = seed_owned_project(&store, OWNER, "0a01").await;
    let volume_id = titled_volume(&store, &scope, "0a02", "Volume A", 1).await;
    let chapter_id = titled_chapter(&store, &scope, "0a03", &volume_id, "Chapter A", 2).await;
    (scope.project_id.as_ref().to_owned(), chapter_id)
}

/// The identity of a package with `files` and the route catalog `route_catalog`, as `build.rs`
/// derives it.
fn package_identity(files: &[EmbeddedSource], route_catalog: &str) -> PackagedIdentity {
    let chain: String = files
        .iter()
        .map(|source| format!("{}\n{}\n", source.path, source.lf_sha256))
        .collect();
    let digest = Sha256::digest(chain.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    identity(&format!("sha256:{digest}"), route_catalog)
}

/// Each row of the proof and of the three ledger tables, as one text.
async fn ledger_state(admin: &Client) -> String {
    admin
        .query_one(
            "SELECT jsonb_build_array(
                      (SELECT jsonb_agg(row_to_json(proof)) FROM storyos.storage_activation_proofs AS proof),
                      (SELECT jsonb_agg(row_to_json(attempt) ORDER BY attempt.attempt_id)
                         FROM storyos.schema_migrations AS attempt),
                      (SELECT jsonb_agg(row_to_json(phase) ORDER BY phase.attempt_id, phase.phase_id)
                         FROM storyos.migration_phases AS phase),
                      (SELECT jsonb_agg(row_to_json(file) ORDER BY file.source_path)
                         FROM storyos.migration_phase_checksums AS file)
                    )::text",
            &[],
        )
        .await
        .unwrap()
        .get(0)
}

async fn attempts_of_files(admin: &Client) -> Vec<(i64, String)> {
    admin
        .query(
            "SELECT attempt_id, source_path FROM storyos.migration_phase_checksums
              ORDER BY attempt_id, source_path",
            &[],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| (row.get(0), row.get(1)))
        .collect()
}

async fn attempt_count(admin: &Client) -> i64 {
    admin
        .query_one("SELECT count(*) FROM storyos.schema_migrations", &[])
        .await
        .unwrap()
        .get(0)
}

fn assert_identity_mismatch(
    outcome: Result<StorageActivation, StorageActivationError>,
    case: &str,
) {
    assert!(
        matches!(
            outcome,
            Err(StorageActivationError::Refused {
                reason: StorageActivationRefusal::IdentityMismatch
            })
        ),
        "{case}: {outcome:?}"
    );
}

/// The runtime login of the cluster, with the password that the test sets.
fn runtime_url(admin_url: &str) -> String {
    let runtime_url = admin_url.replacen("postgres:admin@", "storyos_runtime:runtime@", 1);
    assert_ne!(runtime_url, admin_url, "the form of postgres_admin_url");
    runtime_url
}

async fn connect(url: &str) -> Client {
    let (client, connection) = tokio_postgres::connect(url, NoTls).await.unwrap();
    let client = crate::statement_cache::Client::new(client);
    tokio::spawn(async move {
        let _connection_result = connection.await;
    });
    client
}
