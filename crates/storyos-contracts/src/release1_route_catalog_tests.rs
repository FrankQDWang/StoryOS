use std::collections::BTreeSet;

use super::sections::ERROR_SOURCE;
use super::{Owner, Settlement, registered_operations};
use crate::release1_operation_registry::OperationKind;

fn assert_unique<'a>(label: &str, values: impl IntoIterator<Item = &'a str>) {
    let mut seen = BTreeSet::new();
    let duplicates = values
        .into_iter()
        .filter(|value| !seen.insert(*value))
        .collect::<Vec<_>>();
    assert_eq!(duplicates, Vec::<&str>::new(), "duplicate {label}");
}

#[test]
fn registered_operations_have_unique_ids_routes_schemas_and_fixtures() {
    let operations = registered_operations().collect::<Vec<_>>();
    assert_unique(
        "operation ID",
        operations
            .iter()
            .map(|registered| registered.operation.operation_id),
    );
    let routes = operations
        .iter()
        .map(|registered| {
            format!(
                "{} {}",
                registered.operation.method, registered.operation.path
            )
        })
        .collect::<Vec<_>>();
    assert_unique("method and path", routes.iter().map(String::as_str));
    let event_schemas = crate::ProjectActivityKind::ALL
        .iter()
        .flat_map(|kind| kind.event_schemas().iter().copied());
    assert_unique(
        "schema ID",
        operations
            .iter()
            .flat_map(|registered| {
                let operation = registered.operation;
                let causes = registered.catalog.causes.into_iter().flat_map(|causes| {
                    [&causes.author, &causes.current_producer]
                        .map(|cause| [cause.request_schema, cause.response_schema])
                });
                [operation.request_schema, operation.response_schema]
                    .into_iter()
                    .chain(causes.flatten())
            })
            .chain(event_schemas),
    );
    assert_unique(
        "fixture ID",
        operations
            .iter()
            .flat_map(|registered| registered.operation.fixtures.iter().copied()),
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
