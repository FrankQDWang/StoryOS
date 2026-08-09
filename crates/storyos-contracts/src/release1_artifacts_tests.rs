use super::{
    FIXTURE_DIGEST_PLACEHOLDER, REVIEW_CATALOG_PATH, fixture_corpus_bytes,
    release1_protocol_profile, validate_review_bindings,
};
use crate::digest::sha256_prefixed;

#[test]
fn controlled_project_queries_are_generated_from_the_release_1_contract() {
    let generated = super::generated_files()
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    let openapi =
        String::from_utf8(generated["generated/openapi/storyos-public-release-1.yaml"].clone())
            .expect("OpenAPI must be UTF-8");
    let client = String::from_utf8(
        generated["generated/typescript/storyos-public-release-1/client.mjs"].clone(),
    )
    .expect("generated client must be UTF-8");

    assert!(openapi.contains("/api/v1/projects/{project_id}:"));
    assert!(openapi.contains("operationId: getProject"));
    assert!(openapi.contains("/api/v1/projects/{project_id}/chapters/{chapter_id}:"));
    assert!(openapi.contains("operationId: getChapter"));
    assert!(client.contains("export async function getProject"));
    assert!(client.contains("export async function getChapter"));
}

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
