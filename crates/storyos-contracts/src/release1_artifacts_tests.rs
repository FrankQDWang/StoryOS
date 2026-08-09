use super::{
    FIXTURE_DIGEST_PLACEHOLDER, REVIEW_CATALOG_PATH, fixture_corpus_bytes,
    release1_protocol_profile, validate_review_bindings,
};
use crate::digest::sha256_prefixed;
#[test]
fn fixture_digest_uses_the_declared_self_normalization() {
    let profile = release1_protocol_profile();
    let expected = profile.release_identity.fixture_corpus_digest.clone();
    let mut normalized = profile;
    normalized.release_identity.fixture_corpus_digest = FIXTURE_DIGEST_PLACEHOLDER.to_owned();
    assert_eq!(sha256_prefixed(fixture_corpus_bytes(&normalized)), expected);
}
#[test]
fn changed_review_catalog_contract_is_rejected() {
    let mut bytes = std::fs::read(super::super::repository_root().join(REVIEW_CATALOG_PATH))
        .expect("review catalog should exist");
    bytes[0] = b' ';
    assert!(validate_review_bindings(&bytes).is_err());
}
