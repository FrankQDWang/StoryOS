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
            refusal("STORYOS_AGENT_PLAN_API_KEY", String::new()),
        ],
        [
            (false, true, false),
            (false, true, false),
            (false, true, false)
        ]
    );
}

#[test]
fn an_unknown_embedding_destination_or_one_without_its_model_destination_refuses() {
    let canary = "storyos-settings-canary-worker-5e1a";
    let refusal = |embedding: String, model: &str| {
        let output = Command::new(env!("CARGO_BIN_EXE_storyos-worker"))
            .arg("--check")
            .env("STORYOS_EMBEDDING_DESTINATION", embedding)
            .env("STORYOS_MODEL_DESTINATION", model)
            .env("STORYOS_AGENT_PLAN_API_KEY", canary)
            .env_remove("STORYOS_ENV_FILE")
            .output()
            .expect("packaged storyos-worker should execute");
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        (
            output.status.success(),
            stderr.contains("STORYOS_EMBEDDING_DESTINATION"),
            stderr.contains(canary),
        )
    };

    assert_eq!(
        [
            refusal(format!("openai-{canary}"), "host_fake"),
            refusal("volcengine_agent_plan".to_owned(), "host_fake"),
            refusal("volcengine_agent_plan".to_owned(), "volcengine_agent_plan"),
            refusal("host_fake".to_owned(), "host_fake"),
        ],
        [
            (false, true, false),
            (false, true, false),
            (true, false, false),
            (true, false, false)
        ]
    );
}

#[test]
fn the_real_embedding_route_without_the_key_names_the_key_and_no_value() {
    // An empty env file, so that the process reads no other env file.
    let env_file = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("worker-embedding-{}.env", std::process::id()));
    std::fs::write(&env_file, "").unwrap();
    let canary = "storyos-settings-canary-worker-7d2f";
    let output = Command::new(env!("CARGO_BIN_EXE_storyos-worker"))
        .arg("--check")
        .env("STORYOS_EMBEDDING_DESTINATION", "volcengine_agent_plan")
        .env("STORYOS_MODEL_DESTINATION", "volcengine_agent_plan")
        .env("STORYOS_EXA_API_KEY", canary)
        .env("STORYOS_ENV_FILE", &env_file)
        .env_remove("STORYOS_AGENT_PLAN_API_KEY")
        .output()
        .expect("packaged storyos-worker should execute");
    std::fs::remove_file(&env_file).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        (
            output.status.success(),
            stderr.contains("STORYOS_AGENT_PLAN_API_KEY"),
            stderr.contains(canary),
        ),
        (false, true, false),
        "{stderr}"
    );
}
