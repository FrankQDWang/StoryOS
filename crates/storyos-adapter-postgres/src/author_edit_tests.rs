use super::*;

#[test]
fn payload_digest_is_lowercase_sha256() {
    assert_eq!(
        sha256_hex(b"Base!"),
        "f284a09f33779160d1efe782e2ad3f630b0ece4127b45c9f4caf3715c980fae0"
    );
}
