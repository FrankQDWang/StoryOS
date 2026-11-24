use rustls::CertificateError;
use rustls::pki_types::ServerName;

use super::connection_cause;

#[test]
fn a_certificate_error_gives_a_fixed_sentence_and_no_name() {
    let name_mismatch = std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        rustls::Error::InvalidCertificate(CertificateError::NotValidForNameContext {
            expected: ServerName::try_from("db.canary.invalid").unwrap(),
            presented: vec!["canary-presented.invalid".to_owned()],
        }),
    );
    let unknown_issuer = std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        rustls::Error::InvalidCertificate(CertificateError::UnknownIssuer),
    );
    let invalid_name = ServerName::try_from("").unwrap_err();

    assert_eq!(
        [
            connection_cause(&name_mismatch),
            connection_cause(&unknown_issuer),
            connection_cause(&invalid_name),
            connection_cause(&std::io::Error::from(std::io::ErrorKind::ConnectionRefused)),
        ],
        [
            "the certificate of the server is not valid for the host name",
            "the certificate authority of the server is not trusted",
            "the host is not a valid TLS name",
            "connection refused",
        ]
    );
}
