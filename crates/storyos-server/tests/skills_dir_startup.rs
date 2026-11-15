use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Starts the Server with valid other settings, so the start reaches the Skill directory read.
fn start_with_skills_dir(skills_dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_storyos-server"))
        .args(["--web-root", "/__storyos_missing_web"])
        .env("STORYOS_SKILLS_DIR", skills_dir)
        .env(
            "STORYOS_CHALLENGE_SECRET",
            "test-only-challenge-secret-that-is-at-least-thirty-two-bytes",
        )
        .env(
            "STORYOS_BOOTSTRAP_SESSIONS",
            r#"{"session-a":"018f0000-0000-7001-8000-000000000001"}"#,
        )
        .env(
            "STORYOS_DATABASE_URL",
            "postgres://storyos_runtime@127.0.0.1:1/storyos",
        )
        .env_remove("STORYOS_LOG")
        .env_remove("STORYOS_PUBLIC_ORIGIN")
        .env_remove("STORYOS_MODEL_DESTINATION")
        .env_remove("STORYOS_MODEL_QUALIFICATION_AUTHORIZATION")
        .output()
        .expect("packaged storyos-server should execute")
}

fn refused_stderr(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(1), "{stdout}{stderr}");
    assert!(!stdout.contains("STORYOS_SERVER_URL="), "{stdout}{stderr}");
    stderr
}

#[test]
fn an_invalid_skill_package_stops_the_start_with_the_package_and_the_rule() {
    let root = std::env::temp_dir().join(format!("storyos-skills-{}", uuid::Uuid::now_v7()));
    fs::create_dir_all(root.join("plot-ideas")).unwrap();
    fs::write(
        root.join("plot-ideas/SKILL.md"),
        "---\nname: plot-notes\ndescription: Use it for plot notes.\n---\n",
    )
    .unwrap();
    let stderr = refused_stderr(&start_with_skills_dir(&root));
    fs::remove_dir_all(&root).unwrap();
    assert!(stderr.contains("plot-ideas"), "{stderr}");
    assert!(stderr.contains("name_directory_mismatch"), "{stderr}");
}

#[test]
fn a_catalog_text_above_32000_tokens_stops_the_start_with_the_rule() {
    let root = std::env::temp_dir().join(format!("storyos-skills-{}", uuid::Uuid::now_v7()));
    let description = "d".repeat(1024);
    for index in 0..31 {
        let name = format!("skill-{index:02}");
        fs::create_dir_all(root.join(&name)).unwrap();
        fs::write(
            root.join(&name).join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {description}\n---\n"),
        )
        .unwrap();
    }
    let stderr = refused_stderr(&start_with_skills_dir(&root));
    fs::remove_dir_all(&root).unwrap();
    assert!(stderr.contains("skill_catalog_limit_exceeded"), "{stderr}");
}

#[test]
fn a_skills_dir_that_is_not_a_directory_stops_the_start_with_its_name() {
    let stderr = refused_stderr(&start_with_skills_dir(Path::new(
        "/__storyos_missing_skills",
    )));
    assert!(stderr.contains("STORYOS_SKILLS_DIR"), "{stderr}");
}

#[test]
fn the_fixture_skill_directory_passes_and_the_start_goes_on_to_the_web_assets() {
    let fixtures: PathBuf =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/skill-packages");
    let stderr = refused_stderr(&start_with_skills_dir(&fixtures));
    assert!(!stderr.contains("STORYOS_SKILLS_DIR"), "{stderr}");
}
