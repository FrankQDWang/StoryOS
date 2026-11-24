//! The TLS connector of a PostgreSQL connection with `sslmode=require` (ADR 0104 section 5).
//!
//! The connector always verifies the certificate chain and the host name. The trust roots are the
//! certificates of the operating system, or the file of `SSL_CERT_FILE`.

#[cfg(test)]
#[path = "postgres_tls_tests.rs"]
mod tests;

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll};

use rustls::pki_types::ServerName;
use rustls::{ClientConfig, RootCertStore};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_postgres::Socket;
use tokio_postgres::tls::{ChannelBinding, MakeTlsConnect, TlsConnect, TlsStream};
use tokio_rustls::TlsConnector;

/// Makes one verified TLS connection for each host of a PostgreSQL URL.
#[derive(Clone)]
pub(crate) struct VerifiedTls {
    config: Result<Arc<ClientConfig>, rustls::Error>,
}

impl VerifiedTls {
    /// The connector with the trust roots that the process loads at its first TLS connection.
    pub(crate) fn shared() -> Self {
        static CONFIG: OnceLock<Result<Arc<ClientConfig>, rustls::Error>> = OnceLock::new();
        Self {
            config: CONFIG.get_or_init(client_config).clone(),
        }
    }
}

fn client_config() -> Result<Arc<ClientConfig>, rustls::Error> {
    let mut roots = RootCertStore::empty();
    // A certificate that does not parse is not a trust root; a store with no root refuses each peer.
    let (_added, _ignored) =
        roots.add_parsable_certificates(rustls_native_certs::load_native_certs().certs);
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(Arc::new(config))
}

impl MakeTlsConnect<Socket> for VerifiedTls {
    type Stream = VerifiedTlsStream;
    type TlsConnect = VerifiedTlsConnect;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn make_tls_connect(&mut self, domain: &str) -> Result<Self::TlsConnect, Self::Error> {
        let config = self.config.clone()?;
        let name = ServerName::try_from(domain.to_owned())?;
        Ok(VerifiedTlsConnect {
            connector: TlsConnector::from(config),
            name,
        })
    }
}

pub(crate) struct VerifiedTlsConnect {
    connector: TlsConnector,
    name: ServerName<'static>,
}

impl TlsConnect<Socket> for VerifiedTlsConnect {
    type Stream = VerifiedTlsStream;
    type Error = io::Error;
    type Future = Pin<Box<dyn Future<Output = io::Result<VerifiedTlsStream>> + Send>>;

    fn connect(self, stream: Socket) -> Self::Future {
        Box::pin(async move {
            let stream = self.connector.connect(self.name, stream).await?;
            Ok(VerifiedTlsStream(stream))
        })
    }
}

pub(crate) struct VerifiedTlsStream(tokio_rustls::client::TlsStream<Socket>);

impl TlsStream for VerifiedTlsStream {
    fn channel_binding(&self) -> ChannelBinding {
        ChannelBinding::none()
    }
}

impl AsyncRead for VerifiedTlsStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_read(context, buffer)
    }
}

impl AsyncWrite for VerifiedTlsStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(context, buffer)
    }

    fn poll_flush(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(context)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(context)
    }
}
