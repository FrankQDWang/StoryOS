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

#[test]
fn a_wrong_setting_names_its_variable_and_no_value() {
    let canary = "storyos-settings-canary-worker-3b8e";
    let output = Command::new(env!("CARGO_BIN_EXE_storyos-worker"))
        .arg("--check")
        .env("STORYOS_EXPORT_LEASE_TTL_SECS", canary)
        .output()
        .expect("packaged storyos-worker should execute");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("STORYOS_EXPORT_LEASE_TTL_SECS"), "{stderr}");
    assert!(!stderr.contains(canary), "{stderr}");
}

#[test]
fn an_unknown_destination_or_a_malformed_database_url_names_its_variable_and_no_value() {
    let canary = "storyos-settings-canary-worker-9c4d";
    let refusal = |name: &str, value: String| {
        let output = Command::new(env!("CARGO_BIN_EXE_storyos-worker"))
            .arg("--once")
            .env(name, &value)
            .env_remove("STORYOS_ENV_FILE")
            .env_remove(if name == "STORYOS_DATABASE_URL" {
                "STORYOS_MODEL_DESTINATION"
            } else {
                "STORYOS_DATABASE_URL"
            })
            .output()
            .expect("packaged storyos-worker should execute");
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        (
            output.status.success(),
            stderr.contains(name),
            stderr.contains(canary),
        )
    };

    assert_eq!(
        [
            refusal("STORYOS_MODEL_DESTINATION", format!("volcengine-{canary}")),
            refusal("STORYOS_DATABASE_URL", format!("not a url {canary}")),
        ],
        [(false, true, false), (false, true, false)]
    );
}
