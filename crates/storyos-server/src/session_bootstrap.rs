use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use uuid::Uuid;

/// Eight-hour Client Session Binding lifetime used for Foundation-local cookies.
pub const CLIENT_SESSION_BINDING_LIFETIME_SECS: u64 = 8 * 60 * 60;

/// Test-only allowance that loads more than one mapping and disables issuance.
pub const TEST_ALLOW_MULTIPLE_BOOTSTRAP_SESSIONS: &str =
    "STORYOS_TEST_ALLOW_MULTIPLE_BOOTSTRAP_SESSIONS";

/// Whether the packaged Server issues `storyos_session` on the HTML document GET.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TrustedLocalSessionBootstrap {
    /// Isolation tests and API-only hosts do not issue a cookie.
    #[default]
    Disabled,
    /// The single configured handle is the cookie value.
    IssueTheSingleConfiguredHandle,
}

/// Why packaged production startup refused the listener.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackagedSessionBootstrapError {
    Missing,
    InvalidJson,
    EmptyHandle,
    InvalidHandle,
    InvalidUser,
    MappingCount,
}

impl fmt::Display for PackagedSessionBootstrapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => {
                write!(
                    formatter,
                    "STORYOS_BOOTSTRAP_SESSIONS must contain exactly one handle"
                )
            }
            Self::InvalidJson => write!(formatter, "STORYOS_BOOTSTRAP_SESSIONS is not valid JSON"),
            Self::EmptyHandle => {
                write!(formatter, "STORYOS_BOOTSTRAP_SESSIONS has an empty handle")
            }
            Self::InvalidHandle => {
                write!(
                    formatter,
                    "STORYOS_BOOTSTRAP_SESSIONS has a handle that cannot be a cookie value"
                )
            }
            Self::InvalidUser => {
                write!(
                    formatter,
                    "STORYOS_BOOTSTRAP_SESSIONS has an invalid User UUID"
                )
            }
            Self::MappingCount => write!(
                formatter,
                "STORYOS_BOOTSTRAP_SESSIONS must contain exactly one handle"
            ),
        }
    }
}

impl Error for PackagedSessionBootstrapError {}

/// Whether more than one bootstrap mapping may start the packaged Server.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MultipleMappingAllowance {
    /// Production: zero or more than one mapping refuses the listener.
    Refuse,
    /// Isolation tests may load more than one mapping and do not issue a cookie.
    IsolationTestsDisableIssuance,
}

/// Parse Foundation-local bootstrap mappings for the packaged Server.
pub fn packaged_session_mappings(
    raw: Option<&str>,
    multiple_mapping_allowance: MultipleMappingAllowance,
) -> Result<(HashMap<String, String>, TrustedLocalSessionBootstrap), PackagedSessionBootstrapError>
{
    let Some(raw) = raw.filter(|value| !value.is_empty()) else {
        return Err(PackagedSessionBootstrapError::Missing);
    };
    let mappings: HashMap<String, String> =
        serde_json::from_str(raw).map_err(|_| PackagedSessionBootstrapError::InvalidJson)?;
    if mappings.is_empty() {
        return Err(PackagedSessionBootstrapError::Missing);
    }
    for (handle, owner_user_id) in &mappings {
        if handle.is_empty() {
            return Err(PackagedSessionBootstrapError::EmptyHandle);
        }
        if !handle.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'_' || byte == b'-'
        }) {
            return Err(PackagedSessionBootstrapError::InvalidHandle);
        }
        Uuid::parse_str(owner_user_id).map_err(|_| PackagedSessionBootstrapError::InvalidUser)?;
    }
    match mappings.len() {
        1 => {
            let bootstrap = if multiple_mapping_allowance
                == MultipleMappingAllowance::IsolationTestsDisableIssuance
            {
                TrustedLocalSessionBootstrap::Disabled
            } else {
                TrustedLocalSessionBootstrap::IssueTheSingleConfiguredHandle
            };
            Ok((mappings, bootstrap))
        }
        _ if multiple_mapping_allowance
            == MultipleMappingAllowance::IsolationTestsDisableIssuance =>
        {
            Ok((mappings, TrustedLocalSessionBootstrap::Disabled))
        }
        _ => Err(PackagedSessionBootstrapError::MappingCount),
    }
}

#[cfg(test)]
#[path = "session_bootstrap_tests.rs"]
mod tests;
