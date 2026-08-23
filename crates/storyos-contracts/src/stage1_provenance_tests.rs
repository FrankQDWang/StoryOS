use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::repository_root;

use super::{provenance_evidence, verify_provenance_evidence};

static TEMP_WORKSPACE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn temp_workspace(members: &str) -> std::path::PathBuf {
    let now_nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    temp_workspace_at(members, now_nanos)
}

fn temp_workspace_at(members: &str, now_nanos: u128) -> std::path::PathBuf {
    let workspace_sequence = TEMP_WORKSPACE_SEQUENCE.fetch_add(/*val*/ 1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "storyos-provenance-{}-{now_nanos}-{workspace_sequence}",
        std::process::id(),
    ));
    fs::create_dir(&dir).expect("temp workspace");
    fs::write(
        dir.join("Cargo.toml"),
        format!("[workspace]\nmembers = [\n{members}\n]\n"),
    )
    .expect("temp Cargo.toml");
    dir
}

#[test]
fn equal_clock_values_get_distinct_temp_workspaces() {
    let now_nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();

    let first = temp_workspace_at("", now_nanos);
    let second = temp_workspace_at("", now_nanos);

    assert_ne!(first, second);
}

#[test]
fn current_repository_proves_production_shaped_provenance() {
    provenance_evidence(repository_root())
        .expect("current workspace should prove S1-REQ-006 and S1-EVD-006");
}

#[test]
fn reference_workspace_member_is_rejected() {
    let dir = temp_workspace("  \"crates/storyos-adapter-postgres\",\n  \".reference/codex\",\n");
    let evidence = provenance_evidence(repository_root()).expect("complete evidence");
    let error = verify_provenance_evidence(&dir, &evidence)
        .expect_err("`.reference/**` workspace member should fail closed");
    assert!(error.contains("forbidden tree"), "{error}");
}

#[test]
fn missing_production_journey_is_rejected() {
    let dir = temp_workspace(
        "  \"crates/storyos-adapter-postgres\",\n  \"crates/storyos-application\",\n  \"crates/storyos-contracts\",\n  \"crates/storyos-core\",\n  \"crates/storyos-server\",\n",
    );
    let evidence = provenance_evidence(repository_root()).expect("complete evidence");
    let error = verify_provenance_evidence(&dir, &evidence)
        .expect_err("missing S1-JRN-001 journey should fail closed");
    assert!(
        error.contains("browser-exact-dist/s1-jrn-001.integration.test.ts"),
        "{error}"
    );
}

#[test]
fn windows_prototype_workspace_member_is_rejected() {
    let dir = temp_workspace("  \"crates/storyos-adapter-postgres\",\n  \"prototypes\\\\lab\",\n");
    let evidence = provenance_evidence(repository_root()).expect("complete evidence");
    let error = verify_provenance_evidence(&dir, &evidence)
        .expect_err("Windows prototypes path should fail closed");
    assert!(error.contains("forbidden tree"), "{error}");
}

#[test]
fn journey_missing_postgresql_marker_is_rejected() {
    let dir = temp_workspace(
        "  \"crates/storyos-adapter-postgres\",\n  \"crates/storyos-application\",\n  \"crates/storyos-contracts\",\n  \"crates/storyos-core\",\n  \"crates/storyos-server\",\n",
    );
    let journey = dir.join("apps/web/test/browser-exact-dist/s1-jrn-001.integration.test.ts");
    fs::create_dir_all(journey.parent().expect("journey parent")).expect("journey dir");
    fs::write(
        &journey,
        "test(\"S1-JRN-001 runs on the Vite production page and storyos-server\", () => {});\n",
    )
    .expect("journey file");
    let evidence = provenance_evidence(repository_root()).expect("complete evidence");
    let error = verify_provenance_evidence(&dir, &evidence)
        .expect_err("journey without PostgreSQL should fail closed");
    assert!(error.contains("PostgreSQL"), "{error}");
}
