//! The startup rule of a PostgreSQL URL (ADR 0050, ADR 0104 section 5).

#[cfg(test)]
#[path = "database_url_tests.rs"]
mod tests;

use std::net::IpAddr;

use tokio_postgres::config::{Config, Host, SslMode};

/// Why a binary refuses a PostgreSQL URL at startup. It holds no part of the URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseUrlRefusal {
    Unparsable,
    TlsRequired,
    TlsHostNameMissing,
}

impl DatabaseUrlRefusal {
    /// The rule that the value breaks, for the startup message.
    pub fn rule(self) -> &'static str {
        match self {
            Self::Unparsable => "the value must be a PostgreSQL connection string",
            Self::TlsHostNameMissing => {
                "a URL with sslmode=require must give a name for each host, or only hostaddr"
            }
            Self::TlsRequired => {
                "a URL whose host is not a loopback IP address or a Unix socket must say sslmode=require"
            }
        }
    }
}

/// Accepts a URL that parses and that says `sslmode=require` when a host is not a loopback
/// IP address or a Unix socket. A binary checks it before it connects.
pub fn check_database_url(url: &str) -> Result<(), DatabaseUrlRefusal> {
    let config = url
        .parse::<Config>()
        .map_err(|_| DatabaseUrlRefusal::Unparsable)?;
    if config.get_ssl_mode() != SslMode::Require {
        return if is_local(&config) {
            Ok(())
        } else {
            Err(DatabaseUrlRefusal::TlsRequired)
        };
    }
    // The TLS connector verifies the certificate against the host name, and an empty name has none.
    // A URL with neither host nor hostaddr has no name too.
    let hosts = config.get_hosts();
    if (hosts.is_empty() && config.get_hostaddrs().is_empty())
        || hosts
            .iter()
            .any(|host| matches!(host, Host::Tcp(name) if name.is_empty()))
    {
        return Err(DatabaseUrlRefusal::TlsHostNameMissing);
    }
    Ok(())
}

fn is_local(config: &Config) -> bool {
    let hosts = config.get_hosts().iter().all(|host| match host {
        // A host name such as `localhost` is not local: a name can resolve to another address.
        Host::Tcp(name) => name
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback()),
        #[cfg(unix)]
        Host::Unix(_) => true,
    });
    hosts && config.get_hostaddrs().iter().all(IpAddr::is_loopback)
}
