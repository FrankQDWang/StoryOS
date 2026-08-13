use super::{
    CHALLENGE_REQUEST_SCHEMA_PATH, CHALLENGE_RESPONSE_SCHEMA_PATH, CHAPTER_RESPONSE_SCHEMA_PATH,
    FIXTURE_DIGEST_PLACEHOLDER, OPENAPI_PATH, PROJECT_RESPONSE_SCHEMA_PATH, RESPONSE_SCHEMA_PATH,
    REVIEW_CATALOG_PATH, fixture_corpus_bytes, release1_protocol_profile, validate_review_bindings,
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
    assert!(openapi.contains("operationId: createProjectCommandChallenge"));
    assert!(openapi.contains("Retry-After:"));
    assert!(openapi.contains("maximum: 60"));
    assert!(client.contains("export async function createProjectCommandChallenge"));
    assert!(client.contains("this.retryAfterSeconds = details.retryAfterSeconds"));
    assert!(client.contains("response.headers.get(\"retry-after\")"));
    assert!(openapi.contains("operationId: createEditorSession"));
    assert!(openapi.contains("operationId: getEditorSession"));
    assert!(openapi.contains("- name: Idempotency-Key"));
    assert!(openapi.contains("- name: X-StoryOS-Anti-Forgery"));
    assert!(client.contains("export async function createEditorSession"));
    assert!(client.contains("export async function getEditorSession"));
    assert!(client.contains("x-storyos-anti-forgery"));
}

#[test]
fn generated_openapi_file_references_resolve_from_the_openapi_directory() {
    let generated = super::generated_files()
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    let openapi =
        String::from_utf8(generated[OPENAPI_PATH].clone()).expect("OpenAPI must be UTF-8");
    let repo_root = super::super::repository_root();
    let openapi_path = repo_root.join(OPENAPI_PATH);
    let openapi_directory = openapi_path
        .parent()
        .expect("OpenAPI artifact path must have a parent");
    let references = openapi.lines().filter_map(|line| {
        line.trim()
            .strip_prefix("$ref: ")
            .map(|reference| reference.trim_matches('\''))
    });

    let mut resolved_references = std::collections::BTreeSet::new();
    for reference in references {
        let resolved = openapi_directory.join(reference);
        let resolved = resolved.canonicalize().unwrap_or_else(|error| {
            panic!(
                "OpenAPI reference {reference} must resolve from {}: {error}",
                openapi_directory.display()
            )
        });
        let relative = resolved
            .strip_prefix(repo_root)
            .expect("OpenAPI schema references must remain inside the repository")
            .to_str()
            .expect("generated artifact paths must be UTF-8")
            .to_owned();
        assert!(
            generated.contains_key(relative.as_str()),
            "OpenAPI reference {reference} must resolve to a generated artifact"
        );
        resolved_references.insert(relative);
    }
    assert_eq!(
        resolved_references,
        [
            RESPONSE_SCHEMA_PATH,
            PROJECT_RESPONSE_SCHEMA_PATH,
            CHAPTER_RESPONSE_SCHEMA_PATH,
            CHALLENGE_REQUEST_SCHEMA_PATH,
            CHALLENGE_RESPONSE_SCHEMA_PATH,
            super::EDITOR_SESSION_CREATE_REQUEST_SCHEMA_PATH,
            super::EDITOR_SESSION_CREATE_RESPONSE_SCHEMA_PATH,
            super::EDITOR_SESSION_GET_RESPONSE_SCHEMA_PATH,
        ]
        .map(str::to_owned)
        .into_iter()
        .collect()
    );
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
