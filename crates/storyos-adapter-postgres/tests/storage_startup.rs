use std::net::TcpListener;
use std::process::Command;

const PASSWORD: &str = "storyos-canary-storage-password";

fn activate(admin_url: &str) -> (bool, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_storyos-storage"))
        .env("STORYOS_STORAGE_ADMIN_URL", admin_url)
        .output()
        .expect("packaged storyos-storage should execute");
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    (output.status.success(), text)
}

#[test]
fn a_host_that_is_not_loopback_without_sslmode_require_refuses_with_the_rule_and_no_url() {
    let host = "storyos-canary-host.example.invalid";
    let (success, text) = activate(&format!(
        "postgres://storyos_canary_admin:{PASSWORD}@{host}:6543/storyos_canary_db?sslmode=prefer"
    ));

    assert!(!success, "{text}");
    assert_eq!(
        text.lines().collect::<Vec<_>>(),
        [
            "STORYOS_STORAGE_ACTIVATION=Refused",
            "STORYOS_STORAGE_ACTIVATION_REASON=admin_url_invalid",
            "STORYOS_STORAGE_ADMIN_URL: a URL whose host is not a loopback address must say sslmode=require",
        ]
    );
}

#[test]
fn a_port_with_no_postgresql_prints_the_cause_and_no_url() {
    // A port that was free a moment ago has no listener.
    let port = TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .expect("a free local port")
        .port();
    let (success, text) = activate(&format!(
        "postgres://storyos_canary_admin:{PASSWORD}@127.0.0.1:{port}/storyos_canary_db"
    ));
    let lines = text.lines().collect::<Vec<_>>();

    assert!(!success, "{text}");
    assert_eq!(
        lines[..2],
        [
            "STORYOS_STORAGE_ACTIVATION=Refused",
            "STORYOS_STORAGE_ACTIVATION_REASON=unavailable",
        ],
        "{text}"
    );
    assert_eq!(lines.len(), 3, "{text}");
    assert!(
        lines[2].starts_with("error connecting to server: "),
        "{text}"
    );
    for part in [
        "postgres://",
        PASSWORD,
        "storyos_canary_admin",
        "storyos_canary_db",
        &port.to_string(),
    ] {
        assert!(!text.contains(part), "{part} in {text}");
    }
}
