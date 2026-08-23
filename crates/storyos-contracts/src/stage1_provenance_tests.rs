use std::fs;

use crate::repository_root;

use super::{provenance_evidence, verify_provenance_evidence};

fn temp_workspace(members: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "storyos-provenance-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("temp workspace");
    fs::write(
        dir.join("Cargo.toml"),
        format!("[workspace]\nmembers = [\n{members}\n]\n"),
    )
    .expect("temp Cargo.toml");
    dir
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
