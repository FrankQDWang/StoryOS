//! Packaged Release 1 Storage Activation owner.

use std::fmt;

use sha2::{Digest, Sha256};

use crate::connection_cause::connection_cause;
use crate::database_url::{DatabaseUrlRefusal, check_database_url};
use crate::storage_ledger;

include!(concat!(env!("OUT_DIR"), "/embedded_bootstrap.rs"));

const ADVISORY_LOCK_CLASS: i32 = 0x5354_4f53;
const ADVISORY_LOCK_OBJECT: i32 = 0x4143_5431;
pub(crate) const PROOF_ID: &str = "release-1";
pub(crate) const CATALOG_ID: &str = "storyos.persistence.release-1";
const MIGRATION_CHAIN_ID: &str = "storyos.persistence.bootstrap.release-1.v3";
const ACTIVE_SCHEMA_VERSION: &str = "storyos.persistence.release-1.v3";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageActivation {
    Active,
    AlreadyActive,
    /// The database was Active with an earlier identity, and it now holds `to_files` files.
    Upgraded {
        from_files: usize,
        to_files: usize,
    },
}

impl fmt::Display for StorageActivation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "STORYOS_STORAGE_ACTIVATION=Active")?;
        if let Self::Upgraded {
            from_files,
            to_files,
        } = self
        {
            write!(
                formatter,
                "\nSTORYOS_STORAGE_UPGRADE=from {from_files} files to {to_files} files"
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageActivationRefusal {
    NonEmptyWithoutLedger,
    IdentityMismatch,
}

impl StorageActivationRefusal {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NonEmptyWithoutLedger => "non_empty_without_ledger",
            Self::IdentityMismatch => "identity_mismatch",
        }
    }
}

#[derive(Debug)]
pub enum StorageActivationError {
    AdminUrlMissing,
    AdminUrlInvalid(DatabaseUrlRefusal),
    Refused { reason: StorageActivationRefusal },
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for StorageActivationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AdminUrlMissing => write!(
                formatter,
                "STORYOS_STORAGE_ACTIVATION=Refused\nSTORYOS_STORAGE_ACTIVATION_REASON=admin_url_missing"
            ),
            Self::AdminUrlInvalid(refusal) => write!(
                formatter,
                "STORYOS_STORAGE_ACTIVATION=Refused\nSTORYOS_STORAGE_ACTIVATION_REASON=admin_url_invalid\nSTORYOS_STORAGE_ADMIN_URL: {}",
                refusal.rule()
            ),
            Self::Refused { reason } => write!(
                formatter,
                "STORYOS_STORAGE_ACTIVATION=Refused\nSTORYOS_STORAGE_ACTIVATION_REASON={}",
                reason.as_str()
            ),
            Self::Unavailable(source) => {
                write!(
                    formatter,
                    "STORYOS_STORAGE_ACTIVATION=Refused\nSTORYOS_STORAGE_ACTIVATION_REASON=unavailable\n{source}"
                )?;
                // A hosted run has no container log, so the line gives each cause of the chain.
                let mut cause = source.source();
                while let Some(next) = cause {
                    write!(formatter, ": {next}")?;
                    cause = next.source();
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for StorageActivationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::AdminUrlMissing | Self::AdminUrlInvalid(_) | Self::Refused { .. } => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PackagedIdentity {
    pub(crate) catalog_id: String,
    pub(crate) catalog_checksum: String,
    pub(crate) migration_chain_id: String,
    pub(crate) migration_chain_digest: String,
    pub(crate) database_schema_identity: String,
    pub(crate) active_schema_version: String,
    pub(crate) public_release: String,
    pub(crate) route_catalog_id: String,
    pub(crate) route_catalog_sha256: String,
}

pub async fn activate_release1_storage(
    admin_url: &str,
) -> Result<StorageActivation, StorageActivationError> {
    if admin_url.is_empty() {
        return Err(StorageActivationError::AdminUrlMissing);
    }
    check_database_url(admin_url).map_err(StorageActivationError::AdminUrlInvalid)?;
    activate(admin_url).await.map_err(|error| match error {
        StorageActivationError::Unavailable(source) => {
            let cause: &(dyn std::error::Error + 'static) = source.as_ref();
            unavailable(std::io::Error::other(connection_cause(cause)))
        }
        error @ (StorageActivationError::AdminUrlMissing
        | StorageActivationError::AdminUrlInvalid(_)
        | StorageActivationError::Refused { .. }) => error,
    })
}

async fn activate(admin_url: &str) -> Result<StorageActivation, StorageActivationError> {
    activate_sources(admin_url, &packaged_identity(), EMBEDDED_SOURCES).await
}

/// Runs the state machine with the identity and the files of a package. A test gives the files
/// of an earlier package.
async fn activate_sources(
    admin_url: &str,
    identity: &PackagedIdentity,
    sources: &[EmbeddedSource],
) -> Result<StorageActivation, StorageActivationError> {
    let client = crate::connection_pool::open(admin_url)
        .await
        .map_err(unavailable)?;
    client
        .execute(
            "SELECT pg_advisory_lock($1, $2)",
            &[&ADVISORY_LOCK_CLASS, &ADVISORY_LOCK_OBJECT],
        )
        .await
        .map_err(unavailable)?;
    let outcome = run_state_machine(&client, identity, sources).await;
    let _unlock = client
        .execute(
            "SELECT pg_advisory_unlock($1, $2)",
            &[&ADVISORY_LOCK_CLASS, &ADVISORY_LOCK_OBJECT],
        )
        .await;
    outcome
}

async fn run_state_machine(
    client: &tokio_postgres::Client,
    identity: &PackagedIdentity,
    sources: &[EmbeddedSource],
) -> Result<StorageActivation, StorageActivationError> {
    if let Some(existing) = read_proof(client).await? {
        if existing == *identity {
            return Ok(StorageActivation::AlreadyActive);
        }
        if existing.catalog_id != identity.catalog_id {
            return Err(refused(StorageActivationRefusal::IdentityMismatch));
        }
        let recorded = storage_ledger::recorded_files(client)
            .await
            .map_err(unavailable)?;
        let Some(later) = later_sources(&recorded, sources) else {
            return Err(refused(StorageActivationRefusal::IdentityMismatch));
        };
        in_transaction(client, persist(client, identity, later, Attempt::Upgrade)).await?;
        return Ok(StorageActivation::Upgraded {
            from_files: recorded.len(),
            to_files: sources.len(),
        });
    }
    if crate::storage_empty_check::database_has_user_state(client)
        .await
        .map_err(unavailable)?
    {
        return Err(refused(StorageActivationRefusal::NonEmptyWithoutLedger));
    }
    in_transaction(
        client,
        persist(client, identity, sources, Attempt::FirstActivation),
    )
    .await?;
    Ok(StorageActivation::Active)
}

/// The files of the package after the recorded files, when the recorded files are a prefix of
/// the files of the package with the same path and the same checksum for each one.
fn later_sources<'a>(
    recorded: &[(String, String)],
    sources: &'a [EmbeddedSource],
) -> Option<&'a [EmbeddedSource]> {
    // An Active database without a recorded file is not a state of the ledger.
    if recorded.is_empty() || recorded.len() > sources.len() {
        return None;
    }
    let prefix = recorded
        .iter()
        .zip(sources)
        .all(|((path, checksum), source)| *path == source.path && *checksum == source.lf_sha256);
    prefix.then(|| &sources[recorded.len()..])
}

async fn in_transaction(
    client: &tokio_postgres::Client,
    work: impl std::future::Future<Output = Result<(), StorageActivationError>>,
) -> Result<(), StorageActivationError> {
    client.batch_execute("BEGIN").await.map_err(unavailable)?;
    match work.await {
        Ok(()) => client.batch_execute("COMMIT").await.map_err(unavailable),
        Err(error) => {
            let _rollback = client.batch_execute("ROLLBACK").await;
            Err(error)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Attempt {
    /// The runtime role has no password yet: the operator sets it after the activation.
    FirstActivation,
    /// The database is in use: the attempt keeps the password of the runtime role and each row.
    Upgrade,
}

/// Applies `files` as the login of the activation, verifies the schema, and records the attempt
/// and the proof.
async fn persist(
    client: &tokio_postgres::Client,
    identity: &PackagedIdentity,
    files: &[EmbeddedSource],
    attempt: Attempt,
) -> Result<(), StorageActivationError> {
    for source in files {
        client
            .batch_execute(source.sql)
            .await
            .map_err(unavailable)?;
    }
    client
        .batch_execute("RESET ROLE")
        .await
        .map_err(unavailable)?;
    let verified: bool = client
        .query_one(
            "SELECT NOT owner.rolcanlogin
                    AND ($1 OR runtime.rolpassword IS NULL)
                    AND to_regnamespace('storyos') IS NOT NULL
                    AND to_regclass('storyos.storage_activation_proofs') IS NOT NULL
                    AND to_regclass('storyos.schema_migrations') IS NOT NULL
                    AND EXISTS (
                      SELECT 1
                      FROM pg_class AS cls
                      JOIN pg_namespace AS ns ON ns.oid = cls.relnamespace
                      WHERE ns.nspname = 'storyos'
                        AND cls.relname = 'projects'
                        AND cls.relrowsecurity
                        AND cls.relforcerowsecurity
                    )
             FROM pg_roles AS owner
             JOIN pg_authid AS runtime ON runtime.rolname = 'storyos_runtime'
             WHERE owner.rolname = 'storyos_owner'",
            &[&(attempt == Attempt::Upgrade)],
        )
        .await
        .map_err(unavailable)?
        .get(0);
    if !verified {
        return Err(unavailable(std::io::Error::other(
            "Release 1 bootstrap failed role, schema, or forced-RLS verification",
        )));
    }
    client
        .batch_execute("SET LOCAL ROLE storyos_owner")
        .await
        .map_err(unavailable)?;
    let phases = match attempt {
        Attempt::FirstActivation => &storage_ledger::FIRST_ACTIVATION_PHASES,
        Attempt::Upgrade => &storage_ledger::UPGRADE_PHASES,
    };
    storage_ledger::record_attempt(client, identity, phases, files)
        .await
        .map_err(unavailable)?;
    storage_ledger::write_proof(client, identity)
        .await
        .map_err(unavailable)?;
    Ok(())
}

pub(crate) async fn read_proof(
    client: &tokio_postgres::Client,
) -> Result<Option<PackagedIdentity>, StorageActivationError> {
    let present: bool = client
        .query_one(
            "SELECT to_regclass('storyos.storage_activation_proofs') IS NOT NULL",
            &[],
        )
        .await
        .map_err(unavailable)?
        .get(0);
    if !present {
        return Ok(None);
    }
    let row = client
        .query_opt(
            "SELECT catalog_id, catalog_checksum, migration_chain_id, migration_chain_digest,
                    database_schema_identity, active_schema_version, public_release,
                    route_catalog_id, route_catalog_sha256
             FROM storyos.storage_activation_proofs
             WHERE proof_id = $1 AND phase = 'active'",
            &[&PROOF_ID],
        )
        .await
        .map_err(unavailable)?;
    Ok(row.map(|row| PackagedIdentity {
        catalog_id: row.get(0),
        catalog_checksum: row.get(1),
        migration_chain_id: row.get(2),
        migration_chain_digest: row.get(3),
        database_schema_identity: row.get(4),
        active_schema_version: row.get(5),
        public_release: row.get(6),
        route_catalog_id: row.get(7),
        route_catalog_sha256: row.get(8),
    }))
}

pub(crate) fn packaged_identity() -> PackagedIdentity {
    identity(MIGRATION_CHAIN_DIGEST, ROUTE_CATALOG_JSON)
}

/// The migrations directory gives the chain digest, and the generated route catalog gives its digest.
fn identity(migration_chain_digest: &str, route_catalog: &str) -> PackagedIdentity {
    PackagedIdentity {
        catalog_id: CATALOG_ID.to_owned(),
        catalog_checksum: migration_chain_digest.to_owned(),
        migration_chain_id: MIGRATION_CHAIN_ID.to_owned(),
        migration_chain_digest: migration_chain_digest.to_owned(),
        database_schema_identity: migration_chain_digest.to_owned(),
        active_schema_version: ACTIVE_SCHEMA_VERSION.to_owned(),
        public_release: storyos_contracts::PUBLIC_PROTOCOL_RELEASE.to_owned(),
        route_catalog_id: ROUTE_CATALOG_ID.to_owned(),
        route_catalog_sha256: lf_sha256(route_catalog),
    }
}

fn lf_sha256(text: &str) -> String {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    Sha256::digest(normalized.as_bytes()).iter().fold(
        String::from("sha256:"),
        |mut encoded, byte| {
            use std::fmt::Write as _;
            write!(encoded, "{byte:02x}").expect("writing to String cannot fail");
            encoded
        },
    )
}

fn refused(reason: StorageActivationRefusal) -> StorageActivationError {
    StorageActivationError::Refused { reason }
}

fn unavailable(
    source: impl Into<Box<dyn std::error::Error + Send + Sync>>,
) -> StorageActivationError {
    StorageActivationError::Unavailable(source.into())
}

#[cfg(test)]
#[path = "storage_activation_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "storage_upgrade_tests.rs"]
mod upgrade_tests;
