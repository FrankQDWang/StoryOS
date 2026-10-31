use super::{OPENAPI_PATH, check_contract_graph, contract_graph_bytes, release1_protocol_profile};
use crate::release1::HttpMethod;
use crate::release1_chapter_query_artifacts::CHAPTER_RESPONSE_SCHEMA_PATH;
use crate::{
    ApplyAuthorEditEffect, AuthorEditConflictReason, AuthorEditRefusalReason,
    AuthoritativeChapterRevision, NoEffectReason,
};

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
    let expected_references = crate::release1_operation_registry::RELEASE1_OPERATIONS
        .iter()
        .flat_map(|artifacts| {
            let schema_paths = (artifacts.schemas)()
                .into_iter()
                .map(|schema| (schema.schema_id, schema.path))
                .collect::<std::collections::BTreeMap<_, _>>();
            artifacts
                .operations
                .iter()
                .flat_map(move |registered| {
                    let operation = registered.operation;
                    let request =
                        (operation.method != HttpMethod::Get).then_some(operation.request_schema);
                    request
                        .into_iter()
                        .chain([operation.response_schema])
                        .map(|schema_id| schema_paths[schema_id].to_owned())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(resolved_references, expected_references);
}

#[test]
fn author_edit_response_v2_keeps_activity_only_on_the_applied_variant() {
    let generated = super::generated_files()
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    let profile = release1_protocol_profile();
    assert_eq!(
        profile.contract_revision,
        "release1-wire-catalog-2026-09-27-expiry-rebuild"
    );
    assert_eq!(
        profile.release_identity.web_client_contract_revision,
        "storyos.web-client.release-1.v3"
    );
    assert_eq!(
        profile.release_identity.server_contract_revision,
        "storyos.server.release-1.v7"
    );
    assert_eq!(
        profile.release_identity.worker_contract_revision,
        "storyos.worker.release-1.v6"
    );
    assert_eq!(
        profile.release_identity.generated_client_revision,
        "storyos.typescript-client.release-1.v32"
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
    let authoritative = outcomes
        .iter()
        .find(|outcome| outcome["properties"]["kind"]["const"] == "authoritative_applied")
        .expect("authoritative outcome exists");
    assert!(
        authoritative["required"]
            .as_array()
            .expect("applied required fields are an array")
            .contains(&serde_json::json!("project_activity_position"))
    );
    for outcome in outcomes
        .iter()
        .filter(|outcome| outcome["properties"]["kind"]["const"] != "authoritative_applied")
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
            blocks: vec![crate::ManuscriptBlock {
                manuscript_block_id: "block-1".to_owned(),
                block_kind: crate::ManuscriptBlockKind::Paragraph,
                text: "body".to_owned(),
            }],
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
    assert_eq!(
        variants
            .iter()
            .filter(|variant| variant.contains("project_activity_position"))
            .count(),
        1
    );
    assert!(
        variants
            .iter()
            .any(|variant| variant.contains("authoritative_applied")
                && variant.contains("project_activity_position: string"))
    );
    let generated_client = String::from_utf8(
        generated["generated/typescript/storyos-public-release-1/client.mjs"].clone(),
    )
    .expect("generated client is UTF-8");
    assert!(generated_client.contains(
        "export const GENERATED_CLIENT_REVISION = \"storyos.typescript-client.release-1.v32\";"
    ));
    assert!(
        generated[CHAPTER_RESPONSE_SCHEMA_PATH]
            .windows(b"project_activity_position".len())
            .any(|window| window == b"project_activity_position")
    );
}
#[test]
fn changed_contract_graph_is_rejected() {
    let mut graph = contract_graph_bytes();
    assert!(check_contract_graph(&graph).is_ok());
    graph[0] = b' ';
    assert!(check_contract_graph(&graph).is_err());
}

#[test]
fn every_registered_operation_matches_its_generated_wire() {
    let generated = super::generated_files()
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    let text = |path: &str| String::from_utf8(generated[path].clone()).expect("UTF-8 artifact");
    let openapi = text(OPENAPI_PATH);
    let client = text("generated/typescript/storyos-public-release-1/client.mjs");
    let declaration = text("generated/typescript/storyos-public-release-1/client.d.mts");
    let mut client_positions = Vec::new();
    let mut declaration_positions = Vec::new();
    let graph: serde_json::Value =
        serde_json::from_slice(&contract_graph_bytes()).expect("contract graph must be JSON");
    let implemented_slice = openapi
        .lines()
        .find_map(|line| line.strip_prefix("  x-storyos-implemented-slice: "))
        .expect("OpenAPI names the runtime-implemented slice")
        .split(',')
        .collect::<Vec<_>>();
    for artifacts in crate::release1_operation_registry::RELEASE1_OPERATIONS {
        let schema_paths = (artifacts.schemas)()
            .into_iter()
            .map(|schema| (schema.schema_id, schema.path))
            .collect::<std::collections::BTreeMap<_, _>>();
        for registered in artifacts.operations {
            let operation = registered.operation;
            let id = operation.operation_id;
            let statuses = operation
                .responses
                .iter()
                .map(|(status, _)| *status)
                .collect::<Vec<_>>();

            let schema_ref = |schema_id: &str| {
                schema_paths[schema_id]
                    .strip_prefix("generated/")
                    .expect("schema is a generated artifact")
            };
            for schema_id in [operation.request_schema, operation.response_schema] {
                let schema: serde_json::Value =
                    serde_json::from_slice(&generated[schema_paths[schema_id]])
                        .expect("generated schema must be JSON");
                assert_eq!(schema["$id"], schema_id);
            }
            let request_schema: serde_json::Value =
                serde_json::from_slice(&generated[schema_paths[operation.request_schema]])
                    .expect("generated schema must be JSON");
            assert_eq!(request_schema["additionalProperties"], false, "{id}");
            assert!(request_schema["properties"].get("nonce").is_none(), "{id}");
            let response_schema: serde_json::Value =
                serde_json::from_slice(&generated[schema_paths[operation.response_schema]])
                    .expect("generated schema must be JSON");
            if response_schema.get("oneOf").is_none() {
                assert_eq!(response_schema["additionalProperties"], false, "{id}");
            }
            assert!(implemented_slice.contains(&id), "{id}");

            let path_key = format!("\n  {}:\n", operation.path);
            let path_start = openapi
                .find(&path_key)
                .unwrap_or_else(|| panic!("OpenAPI must contain the path of {id}"));
            let method_key = format!(
                "    {}:\n      operationId: {id}\n",
                operation.method.as_str().to_ascii_lowercase()
            );
            let path_item = &openapi[path_start + path_key.len()..];
            let path_item = &path_item[..path_item.find("\n  /").unwrap_or(path_item.len())];
            let method_start = path_item
                .find(&method_key)
                .unwrap_or_else(|| panic!("OpenAPI must contain the method of {id}"));
            let method_lines = path_item[method_start + method_key.len()..]
                .lines()
                .take_while(|line| line.starts_with("      "))
                .collect::<Vec<_>>();
            let responses_at = method_lines
                .iter()
                .position(|line| *line == "      responses:")
                .unwrap_or_else(|| panic!("OpenAPI must list the responses of {id}"));
            let references = |lines: &[&str]| {
                lines
                    .iter()
                    .filter_map(|line| line.trim().strip_prefix("$ref: '../")?.strip_suffix('\''))
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            };
            let request_references = references(&method_lines[..responses_at]);
            if operation.method == HttpMethod::Get {
                assert_eq!(request_references, Vec::<String>::new(), "{id}");
            } else {
                assert_eq!(
                    request_references,
                    [schema_ref(operation.request_schema)],
                    "{id}"
                );
            }
            let mut referenced_statuses = Vec::new();
            let mut status = None;
            for line in &method_lines[responses_at..] {
                if let Some(code) = line
                    .strip_prefix("        '")
                    .and_then(|rest| rest.strip_suffix("':"))
                {
                    status = Some(code);
                }
                if let Some(reference) = references(&[line]).pop() {
                    assert_eq!(reference, schema_ref(operation.response_schema), "{id}");
                    referenced_statuses
                        .push(status.unwrap_or_else(|| panic!("{id}: reference outside a status")));
                }
            }
            assert!(!referenced_statuses.is_empty(), "{id}");
            assert!(
                referenced_statuses
                    .iter()
                    .all(|status| status.starts_with('2')),
                "{id}: {referenced_statuses:?}"
            );
            let openapi_statuses = method_lines[responses_at..]
                .iter()
                .filter_map(|line| {
                    line.strip_prefix("        '")?
                        .strip_suffix("':")?
                        .parse()
                        .ok()
                })
                .collect::<Vec<u16>>();
            assert_eq!(openapi_statuses, statuses, "{id}");

            client_positions.push(client.find(&format!("export async function {id}(")));
            let function = client
                .split(&format!("export async function {id}("))
                .nth(1)
                .unwrap_or_else(|| panic!("the client must contain {id}"))
                .split("\n}\n")
                .next()
                .expect("a client function closes");
            for (openapi_text, client_text) in [
                (
                    "- name: X-StoryOS-Anti-Forgery",
                    "\"x-storyos-anti-forgery\"",
                ),
                ("- name: Idempotency-Key", "\"idempotency-key\""),
                ("text/event-stream", "text/event-stream"),
            ] {
                assert_eq!(
                    function.contains(client_text),
                    method_lines.iter().any(|line| line.contains(openapi_text)),
                    "{id}: {openapi_text}"
                );
            }
            let url = function
                .split_once("path: `")
                .map_or("", |(_, rest)| rest.split('`').next().unwrap_or(rest));
            assert!(
                !url.contains("antiForgery") && !function.contains("nonce"),
                "{id}: the Challenge proof stays out of the URL"
            );
            declaration_positions.push(declaration.find(&format!("export declare function {id}(")));
            let graph_entry = graph["operations"]
                .as_array()
                .expect("contract graph operations must be an array")
                .iter()
                .find(|entry| entry["operation_id"] == id);
            match registered.graph {
                crate::release1_operation_registry::ContractGraphEntry::Preconditions(
                    preconditions,
                ) => {
                    let graph_entry =
                        graph_entry.unwrap_or_else(|| panic!("contract graph must contain {id}"));
                    assert_eq!(graph_entry["kind"], registered.kind.as_str(), "{id}");
                    assert_eq!(
                        graph_entry["preconditions"],
                        serde_json::json!(preconditions),
                        "{id}"
                    );
                }
                crate::release1_operation_registry::ContractGraphEntry::Absent(_) => {
                    assert!(graph_entry.is_none(), "{id}");
                }
            }
        }
    }
    for positions in [client_positions, declaration_positions] {
        assert!(positions.iter().all(Option::is_some));
        assert!(
            positions.is_sorted(),
            "generated functions must follow the registry order"
        );
    }
}
