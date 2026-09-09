//! Foundation Validation Public Origin for packaged `storyos-server`.

use std::error::Error;
use std::fmt;
use std::net::SocketAddr;

use crate::request_origin::TupleOrigin;

/// Allowed site held when the operator configures a public Origin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackagedPublicOrigin {
    pub allowed_host: String,
    pub allowed_origin: String,
}

/// Whether `storyos_session` includes the `Secure` attribute.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SessionCookieSecure {
    /// Local HTTP profile: the browser must accept the cookie on `http`.
    #[default]
    Omit,
    /// Public HTTPS profile: the browser must not send the cookie on a cleartext hop.
    Include,
}

/// Packaged Server transport profile selected at startup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PackagedTransportPlan {
    /// Bind-derived Host and Origin, printed `http` URL, cookie without `Secure`.
    LocalHttp,
    /// Configured `https` Origin, derived Host, printed Origin, cookie `Secure`.
    PublicHttps(PackagedPublicOrigin),
}

/// Why packaged startup refused a public-origin profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackagedTransportError {
    InvalidPublicOrigin,
    NonLoopbackListen,
}

impl fmt::Display for PackagedTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPublicOrigin => {
                write!(formatter, "STORYOS_PUBLIC_ORIGIN must be one https Origin")
            }
            Self::NonLoopbackListen => {
                write!(
                    formatter,
                    "STORYOS_PUBLIC_ORIGIN requires a loopback listen"
                )
            }
        }
    }
}

impl Error for PackagedTransportError {}

/// Select the local HTTP profile or the public HTTPS profile.
pub fn packaged_transport_plan(
    public_origin: Option<&str>,
    bind_address: &str,
) -> Result<PackagedTransportPlan, PackagedTransportError> {
    let Some(raw) = public_origin else {
        return Ok(PackagedTransportPlan::LocalHttp);
    };
    if raw.is_empty() {
        return Err(PackagedTransportError::InvalidPublicOrigin);
    }
    let origin =
        TupleOrigin::from_allowed_origin(raw).ok_or(PackagedTransportError::InvalidPublicOrigin)?;
    if !origin.is_https() || !origin.is_dns_domain() {
        return Err(PackagedTransportError::InvalidPublicOrigin);
    }
    let loopback_tcp = bind_address
        .parse::<SocketAddr>()
        .is_ok_and(|address| address.ip().is_loopback());
    // `localhost` is a loopback name. An absolute path is a unix socket; this
    // ticket does not add a UnixListener, but the listen check must not refuse it.
    let loopback_name = bind_address
        .rsplit_once(':')
        .map_or(bind_address, |(host, _)| host)
        .eq_ignore_ascii_case("localhost");
    let unix_socket = bind_address.starts_with('/');
    if !loopback_tcp && !loopback_name && !unix_socket {
        return Err(PackagedTransportError::NonLoopbackListen);
    }
    Ok(PackagedTransportPlan::PublicHttps(PackagedPublicOrigin {
        allowed_host: origin
            .https_allowed_host()
            .ok_or(PackagedTransportError::InvalidPublicOrigin)?,
        allowed_origin: origin.ascii_serialization(),
    }))
}

#[cfg(test)]
#[path = "public_origin_tests.rs"]
mod tests;
