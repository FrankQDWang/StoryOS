//! SELECT-only Release 1 Storage Activation proof gate for Server and Worker.

use std::fmt;

use tokio_postgres::NoTls;

use crate::storage_activation::{
    StorageActivationError, packaged_catalog, packaged_identity, read_proof,
};

#[derive(Debug)]
pub enum StorageActivationProofError {
    MissingOrInactive,
    IdentityMismatch,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for StorageActivationProofError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingOrInactive => {
                write!(
                    formatter,
                    "Release 1 Storage Activation is missing or not Active"
                )
            }
            Self::IdentityMismatch => {
                write!(
                    formatter,
                    "Release 1 Storage Activation identity does not match"
                )
            }
            Self::Unavailable(source) => write!(
                formatter,
                "Release 1 Storage Activation is unavailable: {source}"
            ),
        }
    }
}

impl std::error::Error for StorageActivationProofError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::MissingOrInactive | Self::IdentityMismatch => None,
        }
    }
}

pub async fn require_release1_storage_activation_proof(
    database_url: &str,
) -> Result<(), StorageActivationProofError> {
    if database_url.is_empty() {
        return Err(StorageActivationProofError::MissingOrInactive);
    }
    let catalog = packaged_catalog().map_err(catalog_error)?;
    let identity = packaged_identity(&catalog).map_err(catalog_error)?;
    let (client, connection) = tokio_postgres::connect(database_url, NoTls)
        .await
        .map_err(unavailable)?;
    tokio::spawn(async move {
        let _connection_result = connection.await;
    });
    let proof = match read_proof(&client).await {
        Ok(proof) => proof,
        Err(StorageActivationError::Unavailable(source)) => {
            return Err(StorageActivationProofError::Unavailable(source));
        }
        Err(StorageActivationError::AdminUrlMissing | StorageActivationError::Refused { .. }) => {
            return Err(StorageActivationProofError::MissingOrInactive);
        }
    };
    match proof {
        Some(existing) if existing == identity => Ok(()),
        Some(_) => Err(StorageActivationProofError::IdentityMismatch),
        None => Err(StorageActivationProofError::MissingOrInactive),
    }
}

fn catalog_error(error: StorageActivationError) -> StorageActivationProofError {
    match error {
        StorageActivationError::Unavailable(source) => {
            StorageActivationProofError::Unavailable(source)
        }
        StorageActivationError::AdminUrlMissing | StorageActivationError::Refused { .. } => {
            unavailable(error)
        }
    }
}

fn unavailable(
    source: impl Into<Box<dyn std::error::Error + Send + Sync>>,
) -> StorageActivationProofError {
    StorageActivationProofError::Unavailable(source.into())
}
