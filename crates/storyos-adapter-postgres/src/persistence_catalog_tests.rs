//! The generated persistence catalog: one JSON description of the activated `storyos` schema for
//! readers and reviewers (ADR 0079). No runtime code reads the file. The test compares the
//! committed file with the activated schema, and `STORYOS_UPDATE_GENERATED=1` rewrites the file.

use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tokio_postgres::{Client, NoTls};

const CATALOG_PATH: &str = "generated/persistence-catalog/release-1-schema.json";

type Named = BTreeMap<String, Value>;

#[derive(Default)]
struct Table {
    columns: Named,
    constraints: Named,
    indexes: Named,
    policies: Named,
    triggers: Named,
    row_level_security: Value,
    owner: Value,
    privileges: Value,
}

fn catalog_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(CATALOG_PATH)
}

fn sorted(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values
}

async fn tables(client: &Client) -> BTreeMap<String, Table> {
    let mut tables = BTreeMap::<String, Table>::new();
    for row in client
        .query(
            "SELECT c.relname::text, c.relrowsecurity, c.relforcerowsecurity,
                    pg_get_userbyid(c.relowner)::text, c.relacl::text[]
               FROM pg_class AS c
              WHERE c.relnamespace = 'storyos'::regnamespace AND c.relkind IN ('r', 'p')",
            &[],
        )
        .await
        .unwrap()
    {
        let table = tables.entry(row.get(0)).or_default();
        table.row_level_security =
            json!({ "enabled": row.get::<_, bool>(1), "forced": row.get::<_, bool>(2) });
        table.owner = json!(row.get::<_, String>(3));
        table.privileges = json!(row.get::<_, Option<Vec<String>>>(4).map(sorted));
    }
    for row in client
        .query(
            "SELECT c.relname::text, a.attname::text, format_type(a.atttypid, a.atttypmod),
                    a.attnotnull, pg_get_expr(d.adbin, d.adrelid)
               FROM pg_attribute AS a
               JOIN pg_class AS c ON c.oid = a.attrelid
               LEFT JOIN pg_attrdef AS d ON (d.adrelid, d.adnum) = (a.attrelid, a.attnum)
              WHERE c.relnamespace = 'storyos'::regnamespace AND c.relkind IN ('r', 'p')
                AND a.attnum > 0 AND NOT a.attisdropped",
            &[],
        )
        .await
        .unwrap()
    {
        let column = json!({
            "type": row.get::<_, String>(2),
            "nullable": !row.get::<_, bool>(3),
            "default": row.get::<_, Option<String>>(4),
        });
        table_of(&mut tables, row.get(0))
            .columns
            .insert(row.get(1), column);
    }
    for row in client
        .query(
            "SELECT c.relname::text, k.conname::text, pg_get_constraintdef(k.oid)
               FROM pg_constraint AS k
               JOIN pg_class AS c ON c.oid = k.conrelid
              WHERE c.relnamespace = 'storyos'::regnamespace",
            &[],
        )
        .await
        .unwrap()
    {
        table_of(&mut tables, row.get(0))
            .constraints
            .insert(row.get(1), json!(row.get::<_, String>(2)));
    }
    for row in client
        .query(
            "SELECT tablename::text, indexname::text, indexdef
               FROM pg_indexes WHERE schemaname = 'storyos'",
            &[],
        )
        .await
        .unwrap()
    {
        table_of(&mut tables, row.get(0))
            .indexes
            .insert(row.get(1), json!(row.get::<_, String>(2)));
    }
    for row in client
        .query(
            "SELECT tablename::text, policyname::text, permissive, roles::text[], cmd, qual,
                    with_check
               FROM pg_policies WHERE schemaname = 'storyos'",
            &[],
        )
        .await
        .unwrap()
    {
        let policy = json!({
            "permissive": row.get::<_, String>(2),
            "roles": sorted(row.get(3)),
            "command": row.get::<_, String>(4),
            "using": row.get::<_, Option<String>>(5),
            "with_check": row.get::<_, Option<String>>(6),
        });
        table_of(&mut tables, row.get(0))
            .policies
            .insert(row.get(1), policy);
    }
    for row in client
        .query(
            "SELECT c.relname::text, t.tgname::text, pg_get_triggerdef(t.oid)
               FROM pg_trigger AS t
               JOIN pg_class AS c ON c.oid = t.tgrelid
              WHERE c.relnamespace = 'storyos'::regnamespace AND NOT t.tgisinternal",
            &[],
        )
        .await
        .unwrap()
    {
        table_of(&mut tables, row.get(0))
            .triggers
            .insert(row.get(1), json!(row.get::<_, String>(2)));
    }
    tables
}

fn table_of(tables: &mut BTreeMap<String, Table>, name: String) -> &mut Table {
    tables
        .get_mut(&name)
        .unwrap_or_else(|| panic!("{name} is not a table of the storyos schema"))
}

async fn domains(client: &Client) -> Named {
    let mut domains = Named::new();
    for row in client
        .query(
            "SELECT t.typname::text, format_type(t.typbasetype, t.typtypmod), t.typnotnull,
                    t.typdefault
               FROM pg_type AS t
              WHERE t.typnamespace = 'storyos'::regnamespace AND t.typtype = 'd'",
            &[],
        )
        .await
        .unwrap()
    {
        let mut constraints = Named::new();
        for constraint in client
            .query(
                "SELECT k.conname::text, pg_get_constraintdef(k.oid)
                   FROM pg_constraint AS k
                   JOIN pg_type AS t ON t.oid = k.contypid
                  WHERE t.typnamespace = 'storyos'::regnamespace AND t.typname = $1",
                &[&row.get::<_, String>(0)],
            )
            .await
            .unwrap()
        {
            constraints.insert(constraint.get(0), json!(constraint.get::<_, String>(1)));
        }
        domains.insert(
            row.get(0),
            json!({
                "type": row.get::<_, String>(1),
                "nullable": !row.get::<_, bool>(2),
                "default": row.get::<_, Option<String>>(3),
                "constraints": constraints,
            }),
        );
    }
    domains
}

async fn functions(client: &Client) -> Vec<String> {
    sorted(
        client
            .query(
                "SELECT p.proname || '(' || pg_get_function_identity_arguments(p.oid) || ')'
                   FROM pg_proc AS p WHERE p.pronamespace = 'storyos'::regnamespace",
                &[],
            )
            .await
            .unwrap()
            .iter()
            .map(|row| row.get(0))
            .collect(),
    )
}

/// Gives the catalog file text of the schema that `client` reads: keys in name order at each
/// level, two-space indentation, and one trailing newline.
async fn catalog_text(client: &Client) -> String {
    let tables = tables(client)
        .await
        .into_iter()
        .map(|(name, table)| {
            let value = json!({
                "columns": table.columns,
                "constraints": table.constraints,
                "indexes": table.indexes,
                "owner": table.owner,
                "policies": table.policies,
                "privileges": table.privileges,
                "row_level_security": table.row_level_security,
                "triggers": table.triggers,
            });
            (name, value)
        })
        .collect::<Named>();
    let catalog = json!({
        "schema": "storyos",
        "description": "Generated from the activated schema by make generate-persistence-catalog. Do not edit.",
        "tables": tables,
        "domains": domains(client).await,
        "functions": functions(client).await,
    });
    serde_json::to_string_pretty(&catalog).unwrap() + "\n"
}

/// Gives the first line where `actual` differs from `expected`, or `None` when they are equal.
fn first_difference(expected: &str, actual: &str) -> Option<String> {
    let mut expected_lines = expected.lines();
    let mut actual_lines = actual.lines();
    for line in 1.. {
        match (expected_lines.next(), actual_lines.next()) {
            (None, None) => {
                return (expected != actual).then(|| "the final newline differs".to_owned());
            }
            (left, right) if left == right => {}
            (left, right) => {
                return Some(format!(
                    "line {line}: the committed file has {}, the activated schema gives {}",
                    left.map_or("no line".to_owned(), |text| format!("`{}`", text.trim())),
                    right.map_or("no line".to_owned(), |text| format!("`{}`", text.trim())),
                ));
            }
        }
    }
    unreachable!()
}

#[test]
fn the_first_difference_names_the_line_and_both_texts() {
    assert_eq!(first_difference("a\nb\n", "a\nb\n"), None);
    assert_eq!(
        first_difference("a\n  b\nc\n", "a\n  x\nc\n").as_deref(),
        Some("line 2: the committed file has `b`, the activated schema gives `x`")
    );
    assert_eq!(
        first_difference("a\n", "a\nb\n").as_deref(),
        Some("line 2: the committed file has no line, the activated schema gives `b`")
    );
    assert_eq!(
        first_difference("a\n", "a").as_deref(),
        Some("the final newline differs")
    );
}

#[tokio::test]
#[ignore = "run through scripts/dev-postgres.sh with"]
async fn the_committed_persistence_catalog_matches_the_activated_schema() {
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
    // Each definition names its objects relative to the search path, so the path is fixed.
    client
        .batch_execute("SET search_path = pg_catalog")
        .await
        .unwrap();
    let actual = catalog_text(&client).await;
    let path = catalog_path();
    if std::env::var("STORYOS_UPDATE_GENERATED").as_deref() == Ok("1") {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_default();
    if let Some(difference) = first_difference(&expected, &actual) {
        panic!(
            "{CATALOG_PATH} differs from the activated schema at {difference}; \
             run make generate-persistence-catalog"
        );
    }
}
