use std::process::Command;

const CHILD: &str = "STORYOS_TEST_PANIC_HOOK_CHILD";
const MESSAGE: &str = "storyos-canary-panic-message";

#[test]
fn a_panic_writes_its_location_and_not_its_message() {
    if std::env::var_os(CHILD).is_some() {
        storyos_adapter_diagnostics::install(/*level*/ None).expect("the default level installs");
        panic!("{MESSAGE}");
    }
    let output = Command::new(std::env::current_exe().expect("the test binary path"))
        .args([
            "--exact",
            "a_panic_writes_its_location_and_not_its_message",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .output()
        .expect("the test binary runs");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(!stderr.contains(MESSAGE), "{stderr}");
    let event = stderr
        .lines()
        .find(|line| line.contains("\"panic\""))
        .unwrap_or_else(|| panic!("no panic event in {stderr}"));
    assert!(event.contains("\"level\":\"ERROR\""), "{event}");
    assert!(event.contains("tests/panic_hook.rs:"), "{event}");
}
