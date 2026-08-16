use super::{
    CHALLENGE_REQUEST_SCHEMA_PATH, CHALLENGE_RESPONSE_SCHEMA_PATH, CHAPTER_RESPONSE_SCHEMA_PATH,
    FIXTURE_DIGEST_PLACEHOLDER, OPENAPI_PATH, PROJECT_RESPONSE_SCHEMA_PATH, RESPONSE_SCHEMA_PATH,
    REVIEW_CATALOG_PATH, fixture_corpus_bytes, release1_protocol_profile, validate_review_bindings,
};
use crate::digest::sha256_prefixed;
use crate::{
    ApplyAuthorEditEffect, AuthorEditConflictReason, AuthorEditRefusalReason,
    AuthoritativeChapterRevision, NoEffectReason,
};

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
    assert_eq!(
        openapi.contains("operationId: applyAuthorEdit"),
        crate::release1_author_edit_artifacts::IS_IMPLEMENTED
    );
    assert_eq!(
        client.contains("export async function applyAuthorEdit"),
        crate::release1_author_edit_artifacts::IS_IMPLEMENTED
    );
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
    let mut expected_references = vec![
        RESPONSE_SCHEMA_PATH,
        PROJECT_RESPONSE_SCHEMA_PATH,
        CHAPTER_RESPONSE_SCHEMA_PATH,
        CHALLENGE_REQUEST_SCHEMA_PATH,
        CHALLENGE_RESPONSE_SCHEMA_PATH,
        super::EDITOR_SESSION_CREATE_REQUEST_SCHEMA_PATH,
        super::EDITOR_SESSION_CREATE_RESPONSE_SCHEMA_PATH,
        super::EDITOR_SESSION_GET_RESPONSE_SCHEMA_PATH,
    ];
    if crate::release1_author_edit_artifacts::IS_IMPLEMENTED {
        expected_references.extend([
            crate::release1_author_edit_artifacts::REQUEST_SCHEMA_PATH,
            crate::release1_author_edit_artifacts::RESPONSE_SCHEMA_PATH,
        ]);
    }
    assert_eq!(
        resolved_references,
        expected_references.into_iter().map(str::to_owned).collect()
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
fn author_edit_response_v2_keeps_activity_only_on_the_applied_variant() {
    let generated = super::generated_files()
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    let profile = release1_protocol_profile();
    assert_eq!(
        profile.contract_revision,
        "release1-wire-catalog-2026-08-16-author-edit-response-v2"
    );
    assert_eq!(
        profile.release_identity.web_client_contract_revision,
        "storyos.web-client.release-1.v2"
    );
    assert_eq!(
        profile.release_identity.server_contract_revision,
        "storyos.server.release-1.v2"
    );
    assert_eq!(
        profile.release_identity.worker_contract_revision,
        "storyos.worker.release-1.v2"
    );
    assert_eq!(
        profile.release_identity.generated_client_revision,
        "storyos.typescript-client.release-1.v2"
    );
    let schema: serde_json::Value = serde_json::from_slice(
        &generated[crate::release1_author_edit_artifacts::RESPONSE_SCHEMA_PATH],
    )
    .expect("response schema must be JSON");
    assert_eq!(
        schema["$id"],
        "storyos.command.apply-author-edit.response.v2"
    );
    assert_eq!(
        schema["$defs"]["DomainReceipt"]["required"],
        serde_json::json!([
            "receipt_id",
            "project_scope",
            "command_kind",
            "command_digest",
            "idempotency_key",
            "producer_cause",
            "author_command_admission_id",
            "expected_heads",
            "prior_heads",
            "resulting_heads",
            "authoritative_revision_ids",
            "proposal_revision_ids",
            "authoritative_commit_ids",
            "author_action_sequence",
            "draft_artifact_refs",
            "artifact_lifecycle_event_refs",
            "condition_refs",
            "result",
            "created_at"
        ])
    );
    let outcomes = schema["$defs"]["ApplyAuthorEditEffect"]["oneOf"]
        .as_array()
        .expect("Author Edit outcomes are a union");
    assert!(
        outcomes[0]["required"]
            .as_array()
            .expect("applied required fields are an array")
            .contains(&serde_json::json!("project_activity_position"))
    );
    for outcome in schema["$defs"]["ApplyAuthorEditEffect"]["oneOf"]
        .as_array()
        .expect("Author Edit outcomes are a union")
        .iter()
        .skip(1)
    {
        assert!(
            !outcome["required"]
                .as_array()
                .expect("outcome required fields are an array")
                .contains(&serde_json::json!("project_activity_position"))
        );
        assert!(
            outcome["properties"]
                .as_object()
                .expect("outcome properties are an object")
                .get("project_activity_position")
                .is_none()
        );
        assert_eq!(outcome["additionalProperties"], false);
    }

    let applied = serde_json::to_value(ApplyAuthorEditEffect::AuthoritativeApplied {
        authoritative_revision: AuthoritativeChapterRevision {
            revision_id: "revision-1".to_owned(),
            body: "body".to_owned(),
        },
        authoritative_commit_id: "commit-1".to_owned(),
        author_action_sequence: "1".to_owned(),
        project_activity_position: "1".to_owned(),
    })
    .expect("applied effect serializes");
    assert_eq!(applied["project_activity_position"], "1");

    let zero_authority = [
        ApplyAuthorEditEffect::NoEffect {
            reason: NoEffectReason::ContentUnchanged,
        },
        ApplyAuthorEditEffect::Conflicted {
            reason: AuthorEditConflictReason::StaleAuthoritativeHead,
            current_authoritative_revision_id: "revision-1".to_owned(),
        },
        ApplyAuthorEditEffect::Refused {
            reason: AuthorEditRefusalReason::InvalidSelection,
        },
    ];
    for effect in zero_authority {
        let mut value = serde_json::to_value(effect).expect("zero-authority effect serializes");
        assert!(value.get("project_activity_position").is_none());
        value["project_activity_position"] = serde_json::json!("1");
        assert!(serde_json::from_value::<ApplyAuthorEditEffect>(value).is_err());
    }

    let declarations = crate::release1_author_edit_artifacts::typescript_type_declarations();
    let effect_declaration = declarations
        .lines()
        .find(|line| line.starts_with("export type ApplyAuthorEditEffect ="))
        .expect("generated TypeScript effect declaration exists");
    let variants = effect_declaration.split(" | ").collect::<Vec<_>>();
    assert!(variants[0].contains("project_activity_position: string"));
    assert!(
        variants
            .iter()
            .skip(1)
            .all(|variant| !variant.contains("project_activity_position"))
    );
    let generated_client = String::from_utf8(
        generated["generated/typescript/storyos-public-release-1/client.mjs"].clone(),
    )
    .expect("generated client is UTF-8");
    assert!(generated_client.contains(
        "export const GENERATED_CLIENT_REVISION = \"storyos.typescript-client.release-1.v2\";"
    ));
    let boundary: serde_json::Value =
        serde_json::from_slice(&generated[crate::release1_author_edit_artifacts::FIXTURE_PATHS[2]])
            .expect("boundary fixture is JSON");
    assert_eq!(boundary["receipt"]["result"], "refused");
    assert_eq!(boundary["effect"]["kind"], "refused");
    assert!(
        boundary["effect"]
            .get("project_activity_position")
            .is_none()
    );
    assert!(
        generated[CHAPTER_RESPONSE_SCHEMA_PATH]
            .windows(b"project_activity_position".len())
            .any(|window| window == b"project_activity_position")
    );
}
#[test]
fn changed_review_catalog_contract_is_rejected() {
    let mut bytes = std::fs::read(super::super::repository_root().join(REVIEW_CATALOG_PATH))
        .expect("review catalog should exist");
    bytes[0] = b' ';
    assert!(validate_review_bindings(&bytes).is_err());
}
