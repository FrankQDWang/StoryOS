//! The ledger of the Storage Activation (ADR 0104 section 4). Each first activation and each
//! upgrade is one attempt with its phases. Each migration file has one row with the attempt that
//! applied it, so the rows give the recorded files of the database.

use crate::storage_activation::{EmbeddedSource, PROOF_ID, PackagedIdentity};

const RUNNER_REVISION: &str = "storyos-storage";

pub(crate) struct Phase {
    phase_id: &'static str,
    class: &'static str,
    postcondition: &'static str,
}

pub(crate) const FIRST_ACTIVATION_PHASES: [Phase; 4] = [
    Phase {
        phase_id: "preflight",
        class: "control_read",
        postcondition: "new_install_active_catalog_and_empty_migration_chain_verified",
    },
    Phase {
        phase_id: "initial_schema_bootstrap",
        class: "transactional_ddl",
        postcondition: "release1_schema_catalog_roles_rls_and_constraints_created_atomically",
    },
    VALIDATE_TRANSACTION,
    ACTIVATE,
];

pub(crate) const UPGRADE_PHASES: [Phase; 4] = [
    Phase {
        phase_id: "preflight",
        class: "control_read",
        postcondition: "same_catalog_and_recorded_files_are_a_prefix_of_the_package_verified",
    },
    Phase {
        phase_id: "forward_migration",
        class: "transactional_ddl",
        postcondition: "each_later_file_applied_in_order_in_the_same_transaction",
    },
    VALIDATE_TRANSACTION,
    ACTIVATE,
];

const VALIDATE_TRANSACTION: Phase = Phase {
    phase_id: "validate_transaction",
    class: "transactional_validation",
    postcondition: "constraints_indexes_digests_roles_and_catalog_bindings_valid",
};

const ACTIVATE: Phase = Phase {
    phase_id: "activate",
    class: "controlled_cutover",
    postcondition: "one_exact_release_identity_active_and_traffic_fenced_until_proof",
};

/// The path and the checksum of each recorded file, in the byte order of the paths. The build
/// script sorts the embedded files by name in the same order.
pub(crate) async fn recorded_files(
    client: &crate::statement_cache::Client,
) -> Result<Vec<(String, String)>, tokio_postgres::Error> {
    let mut files: Vec<(String, String)> = client
        .query(
            "SELECT source_path, checksum FROM storyos.migration_phase_checksums",
            &[],
        )
        .await?
        .iter()
        .map(|row| (row.get(0), row.get(1)))
        .collect();
    files.sort();
    Ok(files)
}

/// Records one new attempt with `phases` and one row for each file of `applied`.
pub(crate) async fn record_attempt(
    client: &crate::statement_cache::Client,
    identity: &PackagedIdentity,
    phases: &[Phase],
    applied: &[EmbeddedSource],
) -> Result<(), tokio_postgres::Error> {
    let attempt_id: i64 = client
        .query_one(
            "INSERT INTO storyos.schema_migrations (
               attempt_id, schema_version, migration_id, checksum, release_identity,
               runner_revision, started_at, finished_at, status
             )
             SELECT COALESCE(max(attempt_id), 0) + 1, $1, $2, $3, $4, $5,
                    clock_timestamp(), clock_timestamp(), 'applied'
             FROM storyos.schema_migrations
             RETURNING attempt_id",
            &[
                &identity.active_schema_version,
                &identity.migration_chain_id,
                &identity.migration_chain_digest,
                &identity.public_release,
                &RUNNER_REVISION,
            ],
        )
        .await?
        .get(0);
    for phase in phases {
        client
            .execute(
                "INSERT INTO storyos.migration_phases (
                   attempt_id, phase_id, class, postcondition, disposition, recorded_at
                 ) VALUES ($1, $2, $3, $4, 'passed', clock_timestamp())",
                &[
                    &attempt_id,
                    &phase.phase_id,
                    &phase.class,
                    &phase.postcondition,
                ],
            )
            .await?;
    }
    for source in applied {
        client
            .execute(
                "INSERT INTO storyos.migration_phase_checksums (
                   attempt_id, source_path, checksum
                 ) VALUES ($1, $2, $3)",
                &[&attempt_id, &source.path, &source.lf_sha256],
            )
            .await?;
    }
    Ok(())
}

/// Writes the proof of `identity`. An upgrade replaces the proof of the earlier identity.
pub(crate) async fn write_proof(
    client: &crate::statement_cache::Client,
    identity: &PackagedIdentity,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "INSERT INTO storyos.storage_activation_proofs (
               proof_id, phase, catalog_id, catalog_checksum, migration_chain_id,
               migration_chain_digest, database_schema_identity, active_schema_version,
               public_release, route_catalog_id, route_catalog_sha256, activated_at
             ) VALUES (
               $1, 'active', $2, $3, $4, $5, $6, $7, $8, $9, $10, clock_timestamp()
             )
             ON CONFLICT (proof_id) DO UPDATE SET
               phase = EXCLUDED.phase,
               catalog_id = EXCLUDED.catalog_id,
               catalog_checksum = EXCLUDED.catalog_checksum,
               migration_chain_id = EXCLUDED.migration_chain_id,
               migration_chain_digest = EXCLUDED.migration_chain_digest,
               database_schema_identity = EXCLUDED.database_schema_identity,
               active_schema_version = EXCLUDED.active_schema_version,
               public_release = EXCLUDED.public_release,
               route_catalog_id = EXCLUDED.route_catalog_id,
               route_catalog_sha256 = EXCLUDED.route_catalog_sha256,
               activated_at = EXCLUDED.activated_at",
            &[
                &PROOF_ID,
                &identity.catalog_id,
                &identity.catalog_checksum,
                &identity.migration_chain_id,
                &identity.migration_chain_digest,
                &identity.database_schema_identity,
                &identity.active_schema_version,
                &identity.public_release,
                &identity.route_catalog_id,
                &identity.route_catalog_sha256,
            ],
        )
        .await?;
    Ok(())
}
