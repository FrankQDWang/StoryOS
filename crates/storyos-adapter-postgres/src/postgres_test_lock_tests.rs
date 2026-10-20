//! Damaged evidence tests drop or disable database checks for a short time. Each other
//! PostgreSQL test of the lib target shares their database and process. Thus it holds the test
//! lock from its first statement to its end.

use std::path::{Path, PathBuf};

const TEST_LOCK: &str = "AUTHOR_EDIT_TEST_LOCK";

fn rust_files(directory: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(rust_files(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    files.sort();
    files
}

fn crate_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn shown(path: &Path) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    path.strip_prefix(root).unwrap().display().to_string()
}

/// Tells if `statement` is `let _test_guard = <path>AUTHOR_EDIT_TEST_LOCK.lock().await`.
fn holds_the_lock(statement: &str) -> bool {
    let compact = statement
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    compact
        .strip_prefix("let_test_guard=")
        .and_then(|value| value.strip_suffix(&format!("{TEST_LOCK}.lock().await")))
        .is_some_and(|path| {
            path.chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == ':')
        })
}

/// Gives `file:line name` for each ignored test whose first statement does not take the lock.
/// A file that drops `_test_guard` early gives each of its ignored tests.
fn tests_without_the_lock_first(path: &Path) -> Vec<String> {
    let source = std::fs::read_to_string(path).unwrap();
    let drops_the_guard = source.contains("drop(_test_guard)");
    let lines = source.lines().collect::<Vec<_>>();
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if !line.trim_start().starts_with("#[ignore") {
            continue;
        }
        let function = (index..lines.len())
            .find(|&candidate| lines[candidate].contains("fn "))
            .expect("an ignore attribute is followed by its test function");
        let body = lines[function..].join("\n");
        let name = body["fn ".len() + body.find("fn ").unwrap()..]
            .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
            .next()
            .unwrap()
            .to_owned();
        let first_statement = body[body.find('{').unwrap() + 1..]
            .split(';')
            .next()
            .unwrap();
        if drops_the_guard || !holds_the_lock(first_statement) {
            found.push(format!("{}:{} {name}", shown(path), function + 1));
        }
    }
    found
}

#[test]
fn every_postgresql_test_of_the_lib_target_takes_the_test_lock_first() {
    let found = rust_files(&crate_path("src"))
        .iter()
        .flat_map(|path| tests_without_the_lock_first(path))
        .collect::<Vec<_>>();
    assert_eq!(
        found,
        Vec::<String>::new(),
        "make `let _test_guard = crate::author_edit::tests::{TEST_LOCK}.lock().await;` \
         the first statement of each test, and do not drop it early"
    );
}

#[test]
fn no_integration_target_drops_or_disables_a_database_check() {
    let found = rust_files(&crate_path("tests"))
        .iter()
        .filter(|path| {
            let source = std::fs::read_to_string(path).unwrap().to_uppercase();
            source.contains("DROP CONSTRAINT") || source.contains("DISABLE TRIGGER")
        })
        .map(|path| shown(path))
        .collect::<Vec<_>>();
    assert_eq!(
        found,
        Vec::<String>::new(),
        "write damaged evidence tests in the lib target under {TEST_LOCK}"
    );
}
