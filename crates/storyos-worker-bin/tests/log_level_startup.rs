use std::process::Command;

#[test]
fn unknown_log_level_refuses_before_the_check_mode() {
    let output = Command::new(env!("CARGO_BIN_EXE_storyos-worker"))
        .arg("--check")
        .env("STORYOS_LOG", "verbose")
        .output()
        .expect("packaged storyos-worker should execute");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("STORYOS_LOG"),
        "{output:?}"
    );
}
