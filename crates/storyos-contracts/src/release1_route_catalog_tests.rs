use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::sections::ERROR_SOURCE;
use super::{Owner, Settlement, registered_operations, route_catalog_bytes};
use crate::release1::REQUIRED_CAPABILITIES;
use crate::release1_operation_registry::OperationKind;

/// The registry serves fewer statuses for these queries than their error profile names.
const NARROWER_THAN_ERROR_PROFILE: [&str; 2] = ["getProject", "getChapter"];

fn catalog() -> Value {
    serde_json::from_slice(&route_catalog_bytes()).expect("route catalog must be JSON")
}

fn array(value: &Value) -> &Vec<Value> {
    value.as_array().expect("catalog value must be an array")
}

fn text(value: &Value) -> &str {
    value.as_str().expect("catalog value must be a string")
}

fn assert_unique<'a>(label: &str, values: impl IntoIterator<Item = &'a str>) {
    let mut seen = BTreeSet::new();
    let duplicates = values
        .into_iter()
        .filter(|value| !seen.insert(*value))
        .collect::<Vec<_>>();
    assert_eq!(duplicates, Vec::<&str>::new(), "duplicate {label}");
}

/// Each settlement of one operation: the operation itself and each of its cause profiles.
fn settlements(operation: &Value) -> impl Iterator<Item = &Value> {
    std::iter::once(operation).chain(
        operation["cause_profiles"]
            .as_object()
            .into_iter()
            .flat_map(|causes| causes.values()),
    )
}

#[test]
fn catalog_ids_routes_schemas_and_fixtures_are_unique() {
    let catalog = catalog();
    let operations = array(&catalog["operations"]);
    let events = array(&catalog["events"]);
    assert_unique(
        "operation ID",
        operations
            .iter()
            .map(|operation| text(&operation["operation_id"])),
    );
    let routes = operations
        .iter()
        .map(|operation| {
            format!(
                "{} {}",
                text(&operation["method"]),
                text(&operation["path"])
            )
        })
        .collect::<Vec<_>>();
    assert_unique("method and path", routes.iter().map(String::as_str));
    let operation_schemas = operations.iter().flat_map(|operation| {
        let causes = settlements(operation)
            .skip(1)
            .flat_map(|cause| [&cause["request_schema"], &cause["response_schema"]]);
        [
            &operation["schemas"]["request"],
            &operation["schemas"]["response"],
        ]
        .into_iter()
        .chain(causes)
    });
    assert_unique(
        "schema ID",
        operation_schemas
            .chain(events.iter().map(|event| &event["schema_id"]))
            .chain([&catalog["release_identity"]["schema_id"]])
            .map(text),
    );
    let operation_fixtures = operations.iter().flat_map(|operation| {
        std::iter::once(&operation["fixtures"]["positive"])
            .chain(array(&operation["fixtures"]["negative"]))
    });
    let event_fixtures = events
        .iter()
        .flat_map(|event| [&event["positive_fixture"], &event["negative_fixture"]]);
    assert_unique(
        "fixture ID",
        operation_fixtures.chain(event_fixtures).map(text),
    );
}

#[test]
fn accepted_commands_name_their_own_registered_get_settlement_query() {
    let settlement_queries = registered_operations()
        .filter_map(|registered| match registered.catalog.binding.settlement {
            Settlement::Accepted { query, .. } => {
                assert!(matches!(registered.kind, OperationKind::Command));
                Some(query)
            }
            Settlement::Read
            | Settlement::Challenge
            | Settlement::Stream
            | Settlement::Committed
            | Settlement::CommittedOrReconfirmation
            | Settlement::ConditionalByCause => None,
        })
        .collect::<Vec<_>>();
    assert!(!settlement_queries.is_empty());
    for query in &settlement_queries {
        let registered = registered_operations()
            .find(|registered| std::ptr::eq(registered.operation, *query))
            .unwrap_or_else(|| panic!("{} must be registered", query.operation_id));
        assert!(matches!(registered.kind, OperationKind::Query));
        assert_eq!(query.method, "GET", "{}", query.operation_id);
    }
    assert_unique(
        "settlement query",
        settlement_queries.iter().map(|query| query.operation_id),
    );
}

#[test]
fn commands_follow_their_cause_settlement_and_activity_rules() {
    let catalog = catalog();
    for operation in array(&catalog["operations"]) {
        let id = text(&operation["operation_id"]);
        assert_eq!(
            operation["action_class"] == "conditional_by_cause",
            operation.get("cause_profiles").is_some(),
            "{id}: only a conditional command has cause profiles"
        );
        for settlement in settlements(operation) {
            if matches!(
                text(&settlement["action_class"]),
                "direct_editor_action" | "explicit_editor_command"
            ) {
                assert_eq!(settlement["settlement"]["mode"], "sync", "{id}");
            }
        }
        if operation["kind"] != "command" {
            assert_eq!(
                (
                    &operation["activity"]["success"],
                    &operation["activity"]["no_effect"]
                ),
                (&serde_json::json!([]), &serde_json::json!([])),
                "{id}: only a command maps Project Activity"
            );
        }
    }
}

#[test]
fn capabilities_cover_the_required_set_with_served_operations() {
    let catalog = catalog();
    let served = array(&catalog["operations"])
        .iter()
        .map(|operation| text(&operation["operation_id"]))
        .collect::<BTreeSet<_>>();
    let capabilities = array(&catalog["release1_capability_coverage"]["capabilities"]);
    let ids = capabilities
        .iter()
        .map(|capability| text(&capability["capability_id"]))
        .collect::<Vec<_>>();
    assert_unique("capability ID", ids.iter().copied());
    assert_eq!(
        ids.into_iter().collect::<BTreeSet<_>>(),
        REQUIRED_CAPABILITIES.into_iter().collect()
    );
    for capability in capabilities {
        let id = text(&capability["capability_id"]);
        let required = array(&capability["required_operations"])
            .iter()
            .map(text)
            .collect::<Vec<_>>();
        assert!(!required.is_empty(), "{id}");
        assert_unique(id, required.iter().copied());
        assert!(
            required.iter().all(|operation| served.contains(operation)),
            "{id}"
        );
        for (operation, actions) in capability["operation_actions"]
            .as_object()
            .into_iter()
            .flatten()
        {
            assert!(required.contains(&operation.as_str()), "{id}: {operation}");
            assert!(!array(actions).is_empty(), "{id}: {operation}");
            assert_unique(id, array(actions).iter().map(text));
        }
    }
}

#[test]
fn error_profiles_use_stable_errors_and_served_statuses() {
    let protocol = std::fs::read_to_string(
        crate::repository_root()
            .join("docs/foundation/versioned-command-query-artifact-event-protocol.md"),
    )
    .expect("protocol document must exist");
    let section = protocol
        .split_once("### 11.2 Required error semantics")
        .and_then(|(_, rest)| rest.split_once("### 11.3"))
        .expect("protocol section 11.2 must exist")
        .0;
    let stable_errors = section
        .lines()
        .filter_map(|line| {
            let mut cells = line.split('|').map(str::trim).skip(1);
            let code = cells.next()?.strip_prefix('`')?.strip_suffix('`')?;
            Some((code, cells.next()?.parse::<u64>().ok()?))
        })
        .collect::<BTreeMap<_, _>>();
    let catalog = catalog();
    let profiles = catalog["error_profiles"]
        .as_object()
        .expect("error profiles must be an object");
    for (profile, codes) in profiles {
        for code in array(codes) {
            assert!(stable_errors.contains_key(text(code)), "{profile}: {code}");
        }
    }
    let mut narrower = Vec::new();
    for operation in array(&catalog["operations"]) {
        for settlement in settlements(operation) {
            let served = array(&settlement["settlement"]["http_statuses"]);
            let profile = text(&settlement["error_profile"]);
            if array(&profiles[profile])
                .iter()
                .any(|code| !served.contains(&Value::from(stable_errors[text(code)])))
            {
                narrower.push(text(&operation["operation_id"]));
            }
        }
    }
    assert_eq!(narrower, NARROWER_THAN_ERROR_PROFILE);
}

#[test]
fn every_owner_and_the_error_source_resolve_to_a_tracked_document_anchor() {
    let root = crate::repository_root();
    for source in Owner::ALL
        .map(Owner::source)
        .into_iter()
        .chain([ERROR_SOURCE])
    {
        let (path, anchor) = source.split_once('#').unwrap_or((source, ""));
        let document = std::fs::read_to_string(root.join(path))
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        assert!(
            anchor.is_empty()
                || document
                    .lines()
                    .filter(|line| line.starts_with('#'))
                    .any(|heading| heading_anchor(heading.trim_start_matches('#')) == anchor),
            "{source} names no heading of its document"
        );
    }
}

/// The GitHub anchor of one Markdown heading.
fn heading_anchor(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter(|character| character.is_alphanumeric() || matches!(character, ' ' | '-' | '_'))
        .collect::<String>()
        .split([' ', '-'])
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
