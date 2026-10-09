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
