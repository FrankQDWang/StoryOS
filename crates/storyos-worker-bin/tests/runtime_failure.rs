use std::process::Command;

const PASSWORD: &str = "storyos-canary-worker-password";

#[test]
fn a_runtime_error_writes_one_event_and_no_error_text() {
    let output = Command::new(env!("CARGO_BIN_EXE_storyos-worker"))
        .arg("--once")
        .env(
            "STORYOS_DATABASE_URL",
            format!("postgres://storyos_runtime:{PASSWORD}@127.0.0.1:1/postgres"),
        )
        .env_remove("STORYOS_LOG")
        .output()
        .expect("packaged storyos-worker should execute");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(!stderr.contains(PASSWORD), "{stderr}");
    assert!(stderr.lines().all(|line| line.starts_with('{')), "{stderr}");
    assert!(
        stderr.contains(r#""stage":"storage_activation""#),
        "{stderr}"
    );
}
