//! The command kind and event kind domains of the baseline hold the kinds of the embedded Route
//! Catalog (ADR 0079).

use crate::storage_activation::ROUTE_CATALOG_JSON;
use serde_json::Value;
use std::collections::BTreeSet;
use tokio_postgres::{Client, NoTls};

async fn domain_members(client: &Client, domain: &str) -> BTreeSet<String> {
    let definition: String = client
        .query_one(
            "SELECT pg_get_constraintdef(oid) FROM pg_constraint WHERE contypid = $1::text::regtype",
            &[&format!("storyos.{domain}")],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    // The definition is CHECK ((VALUE = ANY (ARRAY['a'::text, ...]))), so each odd part is a member.
    definition
        .split('\'')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}

fn catalog_strings<'a>(entries: impl Iterator<Item = &'a Value>, field: &str) -> BTreeSet<String> {
    entries
        .map(|entry| entry[field].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
#[ignore = "run through scripts/dev-postgres.sh with"]
async fn the_command_and_event_kind_domains_equal_the_route_catalog() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (client, connection) = tokio_postgres::connect(
        &std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
            .expect("run through scripts/dev-postgres.sh with"),
        NoTls,
    )
    .await
    .unwrap();
    tokio::spawn(connection);
    let catalog: Value = serde_json::from_str(ROUTE_CATALOG_JSON).unwrap();

    assert_eq!(
        (
            domain_members(&client, "command_kind").await,
            domain_members(&client, "event_kind").await,
        ),
        (
            catalog_strings(
                catalog["operations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|operation| operation["kind"] == "command"),
                "operation_id",
            ),
            catalog_strings(catalog["events"].as_array().unwrap().iter(), "event_kind"),
        )
    );
}
