use std::process::Command;

fn refuse_startup(origin: &str, bind: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_storyos-server"))
        .args(["--web-root", "/__storyos_missing_web", "--bind", bind])
        .env("STORYOS_PUBLIC_ORIGIN", origin)
        .env_remove("STORYOS_DATABASE_URL")
        .env_remove("STORYOS_BOOTSTRAP_SESSIONS")
        .output()
        .expect("packaged storyos-server should execute")
}

#[test]
fn invalid_public_origin_refuses_before_storage() {
    let output = refuse_startup("http://example.com", "127.0.0.1:0");
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stdout.contains("STORYOS_SERVER_URL="), "{stdout}{stderr}");
    assert!(stderr.contains("STORYOS_PUBLIC_ORIGIN"), "{stdout}{stderr}");
    assert!(!stderr.contains("STORYOS_DATABASE_URL"), "{stdout}{stderr}");
}

#[test]
fn public_origin_refuses_a_non_loopback_listen_before_storage() {
    let output = refuse_startup("https://example.com", "0.0.0.0:3000");
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stdout.contains("STORYOS_SERVER_URL="), "{stdout}{stderr}");
    assert!(stderr.contains("STORYOS_PUBLIC_ORIGIN"), "{stdout}{stderr}");
    assert!(!stderr.contains("STORYOS_DATABASE_URL"), "{stdout}{stderr}");
}
