use std::process::Command;

#[test]
fn unknown_log_level_refuses_before_other_startup_work() {
    let output = Command::new(env!("CARGO_BIN_EXE_storyos-server"))
        .args(["--web-root", "/__storyos_missing_web"])
        .env("STORYOS_LOG", "verbose")
        .env_remove("STORYOS_DATABASE_URL")
        .output()
        .expect("packaged storyos-server should execute");
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stdout.contains("STORYOS_SERVER_URL="), "{stdout}{stderr}");
    assert!(stderr.contains("STORYOS_LOG"), "{stdout}{stderr}");
}

#[test]
fn a_short_challenge_secret_refuses_startup_with_its_name_and_no_value() {
    let short = "storyos-short-secret-canary";
    let output = Command::new(env!("CARGO_BIN_EXE_storyos-server"))
        .args(["--web-root", "/__storyos_missing_web"])
        .env("STORYOS_CHALLENGE_SECRET", short)
        .env(
            "STORYOS_BOOTSTRAP_SESSIONS",
            r#"{"session-a":"018f0000-0000-7001-8000-000000000001"}"#,
        )
        .env_remove("STORYOS_LOG")
        .env_remove("STORYOS_PUBLIC_ORIGIN")
        .env_remove("STORYOS_DATABASE_URL")
        .output()
        .expect("packaged storyos-server should execute");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(!stdout.contains("STORYOS_SERVER_URL="), "{stdout}{stderr}");
    assert!(
        stderr.contains("STORYOS_CHALLENGE_SECRET"),
        "{stdout}{stderr}"
    );
    assert!(!stderr.contains(short), "{stdout}{stderr}");
}

#[test]
fn a_malformed_database_url_refuses_startup_with_its_name_and_no_value() {
    let canary = "storyos-database-url-canary-51e0";
    let output = Command::new(env!("CARGO_BIN_EXE_storyos-server"))
        .args(["--web-root", "/__storyos_missing_web"])
        .env("STORYOS_DATABASE_URL", format!("not a url {canary}"))
        .env(
            "STORYOS_CHALLENGE_SECRET",
            "test-only-challenge-secret-that-is-at-least-thirty-two-bytes",
        )
        .env(
            "STORYOS_BOOTSTRAP_SESSIONS",
            r#"{"session-a":"018f0000-0000-7001-8000-000000000001"}"#,
        )
        .env_remove("STORYOS_LOG")
        .env_remove("STORYOS_PUBLIC_ORIGIN")
        .output()
        .expect("packaged storyos-server should execute");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("STORYOS_DATABASE_URL"), "{stderr}");
    assert!(!stderr.contains(canary), "{stderr}");
}

#[test]
fn malformed_bootstrap_sessions_refuse_startup_with_their_name_and_no_value() {
    let canary = "storyos-bootstrap-canary-0d7a";
    let output = Command::new(env!("CARGO_BIN_EXE_storyos-server"))
        .args(["--web-root", "/__storyos_missing_web"])
        .env("STORYOS_BOOTSTRAP_SESSIONS", format!("{{{canary}"))
        .env_remove("STORYOS_LOG")
        .env_remove("STORYOS_PUBLIC_ORIGIN")
        .output()
        .expect("packaged storyos-server should execute");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("STORYOS_BOOTSTRAP_SESSIONS"), "{stderr}");
    assert!(!stderr.contains(canary), "{stderr}");
}
