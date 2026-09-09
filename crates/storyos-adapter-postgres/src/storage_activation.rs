//! Packaged Release 1 Storage Activation owner.

use std::fmt;

use sha2::{Digest, Sha256};
use tokio_postgres::NoTls;

include!(concat!(env!("OUT_DIR"), "/embedded_bootstrap.rs"));

const RUNNER_REVISION: &str = "storyos-storage";
const ADVISORY_LOCK_CLASS: i32 = 0x5354_4f53;
const ADVISORY_LOCK_OBJECT: i32 = 0x4143_5431;
const PROOF_ID: &str = "release-1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageActivation {
    Active,
    AlreadyActive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageActivationRefusal {
    NonEmptyWithoutLedger,
    IdentityMismatch,
    CatalogChecksumMismatch,
}

impl StorageActivationRefusal {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NonEmptyWithoutLedger => "non_empty_without_ledger",
            Self::IdentityMismatch => "identity_mismatch",
            Self::CatalogChecksumMismatch => "catalog_checksum_mismatch",
        }
    }
}

#[derive(Debug)]
pub enum StorageActivationError {
    AdminUrlMissing,
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
            Self::Refused { reason } => write!(
                formatter,
                "STORYOS_STORAGE_ACTIVATION=Refused\nSTORYOS_STORAGE_ACTIVATION_REASON={}",
                reason.as_str()
            ),
            Self::Unavailable(source) => write!(
                formatter,
                "STORYOS_STORAGE_ACTIVATION=Refused\nSTORYOS_STORAGE_ACTIVATION_REASON=unavailable\n{source}"
            ),
        }
    }
}

impl std::error::Error for StorageActivationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::AdminUrlMissing | Self::Refused { .. } => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PackagedIdentity {
    catalog_id: String,
    catalog_checksum: String,
    migration_chain_id: String,
    migration_chain_digest: String,
    database_schema_identity: String,
    active_schema_version: String,
    public_release: String,
    route_catalog_id: String,
    route_catalog_sha256: String,
}

pub async fn activate_release1_storage(
    admin_url: &str,
) -> Result<StorageActivation, StorageActivationError> {
    if admin_url.is_empty() {
        return Err(StorageActivationError::AdminUrlMissing);
    }
    let catalog = packaged_catalog()?;
    let identity = packaged_identity(&catalog)?;
    let (client, connection) = tokio_postgres::connect(admin_url, NoTls)
        .await
        .map_err(unavailable)?;
    tokio::spawn(async move {
        let _connection_result = connection.await;
    });
    client
        .execute(
            "SELECT pg_advisory_lock($1, $2)",
            &[&ADVISORY_LOCK_CLASS, &ADVISORY_LOCK_OBJECT],
        )
        .await
        .map_err(unavailable)?;
    let outcome = run_state_machine(&client, &catalog, &identity).await;
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
    catalog: &serde_json::Value,
    identity: &PackagedIdentity,
) -> Result<StorageActivation, StorageActivationError> {
    if let Some(existing) = read_proof(client).await? {
        return if existing == *identity {
            Ok(StorageActivation::AlreadyActive)
        } else {
            Err(refused(StorageActivationRefusal::IdentityMismatch))
        };
    }
    let predecessors = catalog["migration_chain"]["supported_predecessors"]
        .as_array()
        .filter(|items| !items.is_empty());
    let edges = catalog["migration_chain"]["edges"]
        .as_array()
        .filter(|items| !items.is_empty());
    if predecessors.is_some() || edges.is_some() {
        return Err(refused(StorageActivationRefusal::IdentityMismatch));
    }
    if database_has_user_state(client).await? {
        return Err(refused(StorageActivationRefusal::NonEmptyWithoutLedger));
    }
    client.batch_execute("BEGIN").await.map_err(unavailable)?;
    match persist_activation(client, catalog, identity).await {
        Ok(()) => {
            client.batch_execute("COMMIT").await.map_err(unavailable)?;
            Ok(StorageActivation::Active)
        }
        Err(error) => {
            let _rollback = client.batch_execute("ROLLBACK").await;
            Err(error)
        }
    }
}

async fn persist_activation(
    client: &tokio_postgres::Client,
    catalog: &serde_json::Value,
    identity: &PackagedIdentity,
) -> Result<(), StorageActivationError> {
    for source in EMBEDDED_SOURCES {
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
                    AND runtime.rolpassword IS NULL
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
            &[],
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
    let attempt_id = 1_i64;
    client
        .execute(
            "INSERT INTO storyos.schema_migrations (
               schema_version, migration_id, checksum, release_identity, runner_revision,
               started_at, finished_at, status
             ) VALUES ($1, $2, $3, $4, $5, clock_timestamp(), clock_timestamp(), 'applied')",
            &[
                &identity.active_schema_version,
                &identity.migration_chain_id,
                &identity.migration_chain_digest,
                &identity.public_release,
                &RUNNER_REVISION,
            ],
        )
        .await
        .map_err(unavailable)?;
    for phase in catalog["bootstrap_phases"]
        .as_array()
        .ok_or_else(|| refused(StorageActivationRefusal::CatalogChecksumMismatch))?
    {
        let phase_id = required_str(phase, "phase_id")?;
        let class = required_str(phase, "class")?;
        let postcondition = required_str(phase, "postcondition")?;
        client
            .execute(
                "INSERT INTO storyos.migration_phases (
                   attempt_id, phase_id, class, postcondition, disposition, recorded_at
                 ) VALUES ($1, $2, $3, $4, 'passed', clock_timestamp())",
                &[&attempt_id, &phase_id, &class, &postcondition],
            )
            .await
            .map_err(unavailable)?;
    }
    for source in EMBEDDED_SOURCES {
        client
            .execute(
                "INSERT INTO storyos.migration_phase_checksums (
                   attempt_id, source_path, checksum
                 ) VALUES ($1, $2, $3)",
                &[&attempt_id, &source.path, &source.lf_sha256],
            )
            .await
            .map_err(unavailable)?;
    }
    client
        .execute(
            "INSERT INTO storyos.storage_activation_proofs (
               proof_id, phase, catalog_id, catalog_checksum, migration_chain_id,
               migration_chain_digest, database_schema_identity, active_schema_version,
               public_release, route_catalog_id, route_catalog_sha256, activated_at
             ) VALUES (
               $1, 'active', $2, $3, $4, $5, $6, $7, $8, $9, $10, clock_timestamp()
             )",
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

async fn database_has_user_state(
    client: &tokio_postgres::Client,
) -> Result<bool, StorageActivationError> {
    Ok(client
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = 'storyos')
                    OR EXISTS (
                      SELECT 1
                      FROM pg_class AS cls
                      JOIN pg_namespace AS ns ON ns.oid = cls.relnamespace
                      WHERE ns.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
                        AND ns.nspname NOT LIKE 'pg_temp_%'
                        AND ns.nspname NOT LIKE 'pg_toast_temp_%'
                        AND cls.relkind IN ('r', 'p', 'v', 'm', 'S')
                    )",
            &[],
        )
        .await
        .map_err(unavailable)?
        .get(0))
}

pub(crate) fn packaged_catalog() -> Result<serde_json::Value, StorageActivationError> {
    let catalog: serde_json::Value = serde_json::from_str(CATALOG_JSON).map_err(unavailable)?;
    let sources = catalog["migration_chain"]["bootstrap"]["sources"]
        .as_array()
        .ok_or_else(|| refused(StorageActivationRefusal::CatalogChecksumMismatch))?;
    if sources.len() != EMBEDDED_SOURCES.len() {
        return Err(refused(StorageActivationRefusal::CatalogChecksumMismatch));
    }
    for (source, embedded) in sources.iter().zip(EMBEDDED_SOURCES) {
        let path = required_str(source, "path")?;
        let checksum = required_str(source, "lf_sha256")?;
        if path != embedded.path
            || checksum != embedded.lf_sha256
            || lf_sha256(embedded.sql) != embedded.lf_sha256
        {
            return Err(refused(StorageActivationRefusal::CatalogChecksumMismatch));
        }
    }
    Ok(catalog)
}

pub(crate) fn packaged_identity(
    catalog: &serde_json::Value,
) -> Result<PackagedIdentity, StorageActivationError> {
    let schema = &catalog["schema_identity"];
    let binding = &schema["protocol_binding"];
    Ok(PackagedIdentity {
        catalog_id: required_str(catalog, "catalog_id")?,
        catalog_checksum: required_str(
            &catalog["migration_chain"]["bootstrap"],
            "manifest_sha256",
        )?,
        migration_chain_id: required_str(schema, "migration_chain_id")?,
        migration_chain_digest: required_str(schema, "migration_chain_digest")?,
        database_schema_identity: required_str(schema, "database_schema_identity")?,
        active_schema_version: required_str(schema, "active_schema_version")?,
        public_release: required_str(binding, "public_release")?,
        route_catalog_id: required_str(binding, "route_catalog_id")?,
        route_catalog_sha256: required_str(binding, "route_catalog_sha256")?,
    })
}

fn required_str(value: &serde_json::Value, key: &str) -> Result<String, StorageActivationError> {
    value[key]
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| refused(StorageActivationRefusal::CatalogChecksumMismatch))
}

fn lf_sha256(sql: &str) -> String {
    let normalized = sql.replace("\r\n", "\n").replace('\r', "\n");
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
