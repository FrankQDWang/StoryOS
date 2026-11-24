//! The cause of a failed PostgreSQL connection in fixed words (ADR 0104 section 5).
//!
//! A message of the server or of the TLS library can name the user, the database, or the host,
//! so the text keeps only the kind of each error, the SQLSTATE, and fixed sentences.

#[cfg(test)]
#[path = "connection_cause_tests.rs"]
mod tests;

use std::error::Error;
use std::io;

use rustls::CertificateError;
use rustls::pki_types::InvalidDnsNameError;
use tokio_postgres::error::DbError;

/// Each error of the chain of `error`, joined with `: `, with no value of the connection.
pub(crate) fn connection_cause(error: &(dyn Error + 'static)) -> String {
    let mut parts = Vec::new();
    let mut next = Some(error);
    while let Some(current) = next {
        if let Some(database) = current.downcast_ref::<tokio_postgres::Error>() {
            parts.push(database.to_string());
            if let Some(refusal) = database.as_db_error() {
                parts.push(sql_state(refusal));
                break;
            }
        } else if let Some(refusal) = current.downcast_ref::<DbError>() {
            parts.push(sql_state(refusal));
            break;
        } else if let Some(tls) = tls_error(current) {
            parts.push(tls_sentence(tls));
            break;
        } else if current.is::<InvalidDnsNameError>() {
            parts.push("the host is not a valid TLS name".to_owned());
            break;
        } else {
            parts.push(current.to_string());
        }
        next = current.source();
    }
    parts.join(": ")
}

fn sql_state(refusal: &DbError) -> String {
    format!(
        "SQLSTATE {} ({}, {})",
        refusal.code().code(),
        refusal.severity(),
        refusal.routine().unwrap_or("no routine")
    )
}

/// The TLS library gives its error directly or inside an I/O error of the handshake.
fn tls_error<'a>(error: &'a (dyn Error + 'static)) -> Option<&'a rustls::Error> {
    error.downcast_ref::<rustls::Error>().or_else(|| {
        error
            .downcast_ref::<io::Error>()?
            .get_ref()?
            .downcast_ref::<rustls::Error>()
    })
}

fn tls_sentence(error: &rustls::Error) -> String {
    let rustls::Error::InvalidCertificate(certificate) = error else {
        // Only a certificate error holds a name of the peer.
        return error.to_string();
    };
    match certificate {
        CertificateError::UnknownIssuer => "the certificate authority of the server is not trusted",
        CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. } => {
            "the certificate of the server is not valid for the host name"
        }
        CertificateError::Expired | CertificateError::ExpiredContext { .. } => {
            "the certificate of the server has expired"
        }
        CertificateError::NotValidYet | CertificateError::NotValidYetContext { .. } => {
            "the certificate of the server is not valid yet"
        }
        _ => "the certificate of the server is not valid",
    }
    .to_owned()
}
