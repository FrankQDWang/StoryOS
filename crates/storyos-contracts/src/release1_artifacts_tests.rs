use super::{
    CHALLENGE_REQUEST_SCHEMA_PATH, CHALLENGE_RESPONSE_SCHEMA_PATH, CHAPTER_RESPONSE_SCHEMA_PATH,
    FIXTURE_DIGEST_PLACEHOLDER, OPENAPI_PATH, PROJECT_RESPONSE_SCHEMA_PATH, RESPONSE_SCHEMA_PATH,
    REVIEW_CATALOG_PATH, fixture_corpus_bytes, release1_protocol_profile, validate_review_bindings,
};
use crate::digest::sha256_prefixed;
use crate::{
    ApplyAuthorEditEffect, AuthorEditConflictReason, AuthorEditRefusalReason,
    AuthoritativeChapterRevision, GetChapterResponse, NoEffectReason,
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
fn apply_author_edit_outcome_query_is_one_protected_exact_get() {
    let generated = super::generated_files()
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    let request_path = "generated/json-schema/storyos-public-release-1/apply-author-edit-outcome-request.schema.json";
    let response_path = "generated/json-schema/storyos-public-release-1/apply-author-edit-outcome-response.schema.json";
    assert!(generated.contains_key(request_path));
    assert!(generated.contains_key(response_path));

    let request: serde_json::Value =
        serde_json::from_slice(&generated[request_path]).expect("request schema must be JSON");
    assert_eq!(
        request["$id"],
        "storyos.query.apply-author-edit-outcome.request.v1"
    );
    assert_eq!(
        request["required"],
        serde_json::json!(["project_id", "idempotency_key"])
    );
    assert!(request["properties"].get("nonce").is_none());

    let response: serde_json::Value =
        serde_json::from_slice(&generated[response_path]).expect("response schema must be JSON");
    assert_eq!(
        response["$id"],
        "storyos.query.apply-author-edit-outcome.response.v1"
    );
    assert_eq!(response["additionalProperties"], false);
    let outcomes = response["$defs"]["ApplyAuthorEditOutcome"]["oneOf"]
        .as_array()
        .expect("outcome must be a closed union");
    assert_eq!(outcomes.len(), 3);
    assert_eq!(
        outcomes
            .iter()
            .map(|outcome| outcome["properties"]["outcome_kind"]["const"]
                .as_str()
                .expect("outcome discriminator must be a string"))
            .collect::<Vec<_>>(),
        ["committed", "rejected", "still_unknown"]
    );
    assert!(
        response.to_string().contains("ApplyAuthorEditResponse"),
        "committed outcome must reuse response v2"
    );

    let openapi =
        String::from_utf8(generated["generated/openapi/storyos-public-release-1.yaml"].clone())
            .expect("OpenAPI must be UTF-8");
    assert!(openapi.contains(
        "/api/v1/projects/{project_id}/manuscript/author-edit-outcomes/{idempotency_key}:\n    get:"
    ));
    assert!(openapi.contains("operationId: getApplyAuthorEditOutcome"));
    assert!(openapi.contains("- name: X-StoryOS-Anti-Forgery"));

    let client = String::from_utf8(
        generated["generated/typescript/storyos-public-release-1/client.mjs"].clone(),
    )
    .expect("generated client must be UTF-8");
    assert!(client.contains("export async function getApplyAuthorEditOutcome"));
    assert!(client.contains("queryHeaders: { \"x-storyos-anti-forgery\": antiForgery }"));
    let function = client
        .split("export async function getApplyAuthorEditOutcome")
        .nth(1)
        .expect("outcome client function exists")
        .split("\n}\n")
        .next()
        .expect("outcome client function closes");
    assert!(!function.contains("antiForgery)}"));
    assert!(!function.contains("nonce"));
}

#[test]
fn snapshot_and_activity_stream_are_generated_from_the_release_1_contract() {
    let generated = super::generated_files()
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    let openapi =
        String::from_utf8(generated["generated/openapi/storyos-public-release-1.yaml"].clone())
            .expect("OpenAPI must be UTF-8");
    assert!(openapi.contains("/api/v1/projects/{project_id}/snapshots/{snapshot_id}:"));
    assert!(openapi.contains("operationId: getSnapshot"));
    assert!(openapi.contains("/api/v1/projects/{project_id}/activity:"));
    assert!(openapi.contains("operationId: activityStream"));
    assert!(openapi.contains("text/event-stream"));
    assert!(openapi.contains("activity-stream-response.schema.json"));

    let activity_schema = serde_json::from_slice::<serde_json::Value>(
        &generated["generated/json-schema/storyos-public-release-1/activity-stream-response.schema.json"],
    )
    .expect("activity stream response schema is JSON");
    assert_eq!(
        activity_schema["required"],
        serde_json::json!(["id", "event", "data"])
    );
    assert_eq!(
        activity_schema["properties"]["event"]["const"],
        "storyos.project-activity"
    );
    assert!(activity_schema["properties"].get("data_schema").is_none());
    assert_eq!(
        activity_schema["properties"]["data"]["$ref"],
        "#/$defs/ProjectActivityEvent"
    );
    assert!(openapi.contains(
        "x-storyos-implemented-slice: getProtocolProfile,getProject,getChapter,createProjectChallenge,createProject,listProjects,updateProject,archiveProject,createVolume,updateVolume,createChapter,updateChapter,createProjectCommandChallenge,createEditorSession,getEditorSession,applyAuthorEdit,getApplyAuthorEditOutcome,getSnapshot,getManuscriptTree,activityStream,takeOverProjectWriter"
    ));

    let client = String::from_utf8(
        generated["generated/typescript/storyos-public-release-1/client.mjs"].clone(),
    )
    .expect("generated client must be UTF-8");
    assert!(client.contains("export async function getSnapshot"));
    assert!(client.contains("export async function getManuscriptTree"));
    assert!(client.contains("export async function listProjects"));
    assert!(client.contains("export async function activityStream"));
    assert!(client.contains("async function queryText"));
    assert!(client.contains("text/event-stream"));
}

#[test]
fn take_over_project_writer_wire_is_generated_without_stage1_coverage() {
    assert!(
        !crate::stage1_selection::OPERATION_IDS.contains(&"takeOverProjectWriter"),
        "Stage 1 coverage stays closed; #58 publishes generated wire only"
    );

    let generated = super::generated_files()
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    let request_path = "generated/json-schema/storyos-public-release-1/take-over-project-writer-request.schema.json";
    let response_path = "generated/json-schema/storyos-public-release-1/take-over-project-writer-response.schema.json";
    assert!(generated.contains_key(request_path));
    assert!(generated.contains_key(response_path));

    let request: serde_json::Value =
        serde_json::from_slice(&generated[request_path]).expect("request schema must be JSON");
    assert_eq!(
        request["$id"],
        "storyos.command.take-over-project-writer.request.v1"
    );
    assert_eq!(request["additionalProperties"], false);
    assert_eq!(
        request["required"],
        serde_json::json!([
            "command_schema",
            "client_contract_revision",
            "security_policy_revision",
            "correlation_id",
            "editor_session_id",
            "observed_writer_generation",
            "editor_contract_revision"
        ])
    );
    assert!(request["properties"].get("writer_generation").is_none());
    assert_eq!(
        request["properties"]["observed_writer_generation"]["type"],
        "string"
    );

    let response: serde_json::Value =
        serde_json::from_slice(&generated[response_path]).expect("response schema must be JSON");
    assert_eq!(
        response["$id"],
        "storyos.command.take-over-project-writer.response.v1"
    );
    assert_eq!(response["additionalProperties"], false);
    assert_eq!(
        response["required"],
        serde_json::json!([
            "schema_id",
            "correlation_id",
            "project_scope",
            "command_id",
            "author_command_admission_id",
            "receipt",
            "result"
        ])
    );
    let results = response["$defs"]["TakeOverProjectWriterResult"]["oneOf"]
        .as_array()
        .expect("takeover result must be a closed union");
    assert_eq!(
        results
            .iter()
            .map(|variant| variant["properties"]["kind"]["const"]
                .as_str()
                .expect("result discriminator must be a string"))
            .collect::<Vec<_>>(),
        ["takeover_applied", "takeover_compare_failed"]
    );
    assert_eq!(
        results[0]["required"],
        serde_json::json!([
            "kind",
            "prior_editor_session_id",
            "prior_writer_generation",
            "resulting_editor_session_id",
            "resulting_writer_generation",
            "resulting_snapshot_id",
            "resulting_snapshot_activity_position",
            "resulting_heads"
        ])
    );
    assert!(results[0]["properties"].get("base_snapshot").is_none());
    assert_eq!(
        results[1]["required"],
        serde_json::json!([
            "kind",
            "observed_writer_generation",
            "current_writer_generation",
            "current_writer_projection",
            "current_snapshot_id",
            "current_snapshot_activity_position",
            "current_heads",
            "reason"
        ])
    );
    assert_eq!(
        response["$defs"]["TakeoverCompareFailedReason"]["enum"],
        serde_json::json!([
            "writer_generation_advanced_after_admission",
            "requester_became_current_after_admission"
        ])
    );
    assert_eq!(
        response["$defs"]["DomainReceiptCommandKind"]["enum"],
        serde_json::json!([
            "applyAuthorEdit",
            "takeOverProjectWriter",
            "createProject",
            "updateProject",
            "archiveProject",
            "createVolume",
            "createChapter",
            "updateVolume",
            "updateChapter"
        ])
    );

    let editor_session: serde_json::Value =
        serde_json::from_slice(&generated[super::EDITOR_SESSION_CREATE_RESPONSE_SCHEMA_PATH])
            .expect("editor session schema must be JSON");
    assert_eq!(
        editor_session["$defs"]["EditorReadOnlyReason"]["enum"],
        serde_json::json!([
            "secondary_session",
            "superseded_by_takeover",
            "binding_invalid"
        ])
    );

    let openapi =
        String::from_utf8(generated["generated/openapi/storyos-public-release-1.yaml"].clone())
            .expect("OpenAPI must be UTF-8");
    assert!(openapi.contains(
        "/api/v1/projects/{project_id}/editor-sessions/{editor_session_id}/takeovers:\n    post:"
    ));
    assert!(openapi.contains("operationId: takeOverProjectWriter"));
    assert!(openapi.contains("- name: Idempotency-Key"));
    assert!(openapi.contains("- name: X-StoryOS-Anti-Forgery"));
    assert!(!openapi.contains("x-storyos-implemented-slice: getProtocolProfile,getProject,getChapter,createProjectCommandChallenge,createEditorSession,getEditorSession,applyAuthorEdit,getApplyAuthorEditOutcome,getSnapshot,activityStream\n"));
    assert!(openapi.contains("takeOverProjectWriter"));

    let client = String::from_utf8(
        generated["generated/typescript/storyos-public-release-1/client.mjs"].clone(),
    )
    .expect("generated client must be UTF-8");
    assert!(client.contains("export async function digestTakeOverProjectWriter"));
    assert!(client.contains("export async function takeOverProjectWriter"));
    assert!(client.contains("storyos.command.takeOverProjectWriter.jcs.v1"));
    let function = client
        .split("export async function takeOverProjectWriter")
        .nth(1)
        .expect("takeover client function exists")
        .split("\n}\n")
        .next()
        .expect("takeover client function closes");
    assert!(function.contains("editorSessionId"));
    assert!(function.contains("idempotency-key"));
    assert!(function.contains("x-storyos-anti-forgery"));

    let positive: serde_json::Value = serde_json::from_slice(
        &generated["generated/golden-wire/storyos-public-release-1/take-over-project-writer.json"],
    )
    .expect("positive fixture must be JSON");
    assert_eq!(positive["receipt"]["command_kind"], "takeOverProjectWriter");
    assert_eq!(positive["receipt"]["result"], "no_effect");
    assert_eq!(
        positive["receipt"]["author_action_sequence"],
        serde_json::Value::Null
    );
    assert_eq!(
        positive["receipt"]["authoritative_revision_ids"],
        serde_json::json!([])
    );
    assert_eq!(
        positive["receipt"]["authoritative_commit_ids"],
        serde_json::json!([])
    );
    assert_eq!(positive["result"]["kind"], "takeover_applied");
    assert!(positive["result"].get("base_snapshot").is_none());
}

#[test]
fn apply_author_edit_unknown_observation_cannot_disable_reconciliation() {
    let admitted = serde_json::json!({
        "observation_kind": "admission_committed",
        "command_id": "018f0000-0000-7001-8000-000000000031",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000032",
        "reconciliation_required": true
    });
    assert_eq!(
        serde_json::from_value::<crate::ApplyAuthorEditUnknownObservation>(admitted.clone())
            .expect("the required observation must deserialize"),
        crate::ApplyAuthorEditUnknownObservation::AdmissionCommitted {
            command_id: "018f0000-0000-7001-8000-000000000031".to_owned(),
            author_command_admission_id: "018f0000-0000-7001-8000-000000000032".to_owned(),
            reconciliation_required: crate::ReconciliationRequired,
        }
    );
    assert_eq!(
        serde_json::to_value(
            crate::ApplyAuthorEditUnknownObservation::AdmissionCommitted {
                command_id: "018f0000-0000-7001-8000-000000000031".to_owned(),
                author_command_admission_id: "018f0000-0000-7001-8000-000000000032".to_owned(),
                reconciliation_required: crate::ReconciliationRequired,
            }
        )
        .expect("required reconciliation observation must serialize"),
        admitted
    );
    let mut disabled = admitted.clone();
    disabled["reconciliation_required"] = serde_json::json!(false);
    assert!(serde_json::from_value::<crate::ApplyAuthorEditUnknownObservation>(disabled).is_err());
    let mut surplus = admitted;
    surplus["receipt_id"] = serde_json::json!("018f0000-0000-7001-8000-000000000033");
    assert!(serde_json::from_value::<crate::ApplyAuthorEditUnknownObservation>(surplus).is_err());

    assert_eq!(
        serde_json::to_value(crate::ApplyAuthorEditOutcome::Rejected {
            reason: crate::ApplyAuthorEditRejectionReason::ChallengeExpiredUnconsumed,
        })
        .expect("rejected outcome serializes"),
        serde_json::json!({
            "outcome_kind": "rejected",
            "reason": "challenge_expired_unconsumed"
        })
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
        crate::release1_create_project_artifacts::CHALLENGE_REQUEST_SCHEMA_PATH,
        crate::release1_create_project_artifacts::CHALLENGE_RESPONSE_SCHEMA_PATH,
        crate::release1_create_project_artifacts::REQUEST_SCHEMA_PATH,
        crate::release1_create_project_artifacts::RESPONSE_SCHEMA_PATH,
        crate::release1_list_projects_artifacts::RESPONSE_SCHEMA_PATH,
        crate::release1_update_project_artifacts::REQUEST_SCHEMA_PATH,
        crate::release1_update_project_artifacts::RESPONSE_SCHEMA_PATH,
        crate::release1_archive_project_artifacts::REQUEST_SCHEMA_PATH,
        crate::release1_archive_project_artifacts::RESPONSE_SCHEMA_PATH,
        crate::release1_create_volume_artifacts::REQUEST_SCHEMA_PATH,
        crate::release1_create_volume_artifacts::RESPONSE_SCHEMA_PATH,
        crate::release1_create_chapter_artifacts::REQUEST_SCHEMA_PATH,
        crate::release1_create_chapter_artifacts::RESPONSE_SCHEMA_PATH,
        crate::release1_update_volume_artifacts::REQUEST_SCHEMA_PATH,
        crate::release1_update_volume_artifacts::RESPONSE_SCHEMA_PATH,
        crate::release1_update_chapter_artifacts::REQUEST_SCHEMA_PATH,
        crate::release1_update_chapter_artifacts::RESPONSE_SCHEMA_PATH,
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
    expected_references.push(crate::release1_author_edit_outcome_artifacts::RESPONSE_SCHEMA_PATH);
    expected_references.push(crate::release1_snapshot_artifacts::SNAPSHOT_RESPONSE_SCHEMA_PATH);
    expected_references.push(crate::release1_manuscript_tree_artifacts::RESPONSE_SCHEMA_PATH);
    expected_references
        .push(crate::release1_snapshot_artifacts::ACTIVITY_STREAM_RESPONSE_SCHEMA_PATH);
    expected_references.push(crate::release1_takeover_artifacts::REQUEST_SCHEMA_PATH);
    expected_references.push(crate::release1_takeover_artifacts::RESPONSE_SCHEMA_PATH);
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
fn chapter_fixtures_conform_to_the_typed_response_contract() {
    let generated = super::generated_files()
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    for path in [
        super::CHAPTER_FIXTURE_PATHS[0],
        super::CHAPTER_FIXTURE_PATHS[2],
    ] {
        let _: GetChapterResponse = serde_json::from_slice(&generated[path])
            .expect("positive and boundary Chapter fixtures must satisfy GetChapterResponse");
    }
    assert!(
        serde_json::from_slice::<GetChapterResponse>(&generated[super::CHAPTER_FIXTURE_PATHS[1]],)
            .is_err(),
        "the invalid Chapter fixture must not satisfy GetChapterResponse",
    );
}

#[test]
fn author_edit_response_v2_keeps_activity_only_on_the_applied_variant() {
    let generated = super::generated_files()
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    let profile = release1_protocol_profile();
    assert_eq!(
        profile.contract_revision,
        "release1-wire-catalog-2026-08-17-author-edit-outcome-v1"
    );
    assert_eq!(
        profile.release_identity.web_client_contract_revision,
        "storyos.web-client.release-1.v3"
    );
    assert_eq!(
        profile.release_identity.server_contract_revision,
        "storyos.server.release-1.v3"
    );
    assert_eq!(
        profile.release_identity.worker_contract_revision,
        "storyos.worker.release-1.v3"
    );
    assert_eq!(
        profile.release_identity.generated_client_revision,
        "storyos.typescript-client.release-1.v9"
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
        "export const GENERATED_CLIENT_REVISION = \"storyos.typescript-client.release-1.v9\";"
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
