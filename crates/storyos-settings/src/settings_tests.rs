use std::path::PathBuf;

use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

use super::{ENV_FILE_VARIABLE, EnvFile, SettingsError, load_from, optional_number};

const CANARY: &str = "storyos-settings-canary-7c21";

#[derive(Debug, Deserialize)]
struct Sample {
    database_url: SecretString,
    log: Option<String>,
    #[serde(default, deserialize_with = "optional_number")]
    lease_secs: Option<u64>,
}

const KEYS: &[&str] = &["database_url", "log", "lease_secs"];

fn pairs(values: &[(&str, &str)]) -> Vec<(String, String)> {
    values
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

fn summary(sample: &Sample) -> (String, Option<String>, Option<u64>, bool) {
    (
        sample.database_url.expose_secret().to_owned(),
        sample.log.clone(),
        sample.lease_secs,
        format!("{sample:?}").contains(CANARY),
    )
}

fn undiscovered() -> Option<PathBuf> {
    panic!("a run without the real destination must not look for an env file")
}

#[test]
fn settings_take_only_their_keys_and_debug_shows_no_secret() {
    let url = format!("postgres://storyos_runtime:{CANARY}@127.0.0.1/postgres");
    let loaded = load_from::<Sample>(
        pairs(&[
            ("STORYOS_DATABASE_URL", &url),
            ("STORYOS_LEASE_SECS", "30"),
            ("STORYOS_AGENT_PLAN_API_KEY", CANARY),
            ("STORYOS_MODEL_DESTINATION", "host_fake"),
            ("HOME", "/Users/author"),
        ]),
        KEYS,
        EnvFile::RealRoute,
        undiscovered,
    )
    .unwrap();

    assert_eq!(
        (summary(&loaded.settings), loaded.env_file),
        ((url, None, Some(30), false), None)
    );
}

#[test]
fn a_refusal_names_the_variable_and_holds_no_value() {
    let refusal = |values: &[(&str, &str)]| {
        let refused = load_from::<Sample>(pairs(values), KEYS, EnvFile::RealRoute, undiscovered)
            .err()
            .unwrap();
        (
            refused.variables.clone(),
            refused.to_string().contains(CANARY),
        )
    };

    assert_eq!(
        [
            refusal(&[("STORYOS_LOG", CANARY)]),
            refusal(&[
                ("STORYOS_DATABASE_URL", "postgres://"),
                ("STORYOS_LEASE_SECS", CANARY)
            ]),
        ],
        [
            (vec!["STORYOS_DATABASE_URL".to_owned()], false),
            (vec!["STORYOS_LEASE_SECS".to_owned()], false),
        ]
    );
}

#[test]
fn only_a_real_route_worker_run_or_an_explicit_path_reads_the_env_file() {
    let path = std::env::temp_dir().join(format!("storyos-settings-{}.env", std::process::id()));
    std::fs::write(
        &path,
        format!("STORYOS_DATABASE_URL=postgres://{CANARY}@file/postgres\nSTORYOS_LOG=debug\n"),
    )
    .unwrap();
    let file = path.to_string_lossy().into_owned();
    let discovered = load_from::<Sample>(
        pairs(&[
            ("STORYOS_MODEL_DESTINATION", "volcengine_agent_plan"),
            ("STORYOS_LOG", "warn"),
        ]),
        KEYS,
        EnvFile::RealRoute,
        || Some(path.clone()),
    );
    let explicit = load_from::<Sample>(
        pairs(&[(ENV_FILE_VARIABLE, &file)]),
        KEYS,
        EnvFile::RealRoute,
        undiscovered,
    );
    let never = load_from::<Sample>(
        pairs(&[
            (ENV_FILE_VARIABLE, &file),
            ("STORYOS_MODEL_DESTINATION", "volcengine_agent_plan"),
        ]),
        KEYS,
        EnvFile::Never,
        undiscovered,
    );
    let missing = load_from::<Sample>(
        pairs(&[(ENV_FILE_VARIABLE, "/nonexistent/storyos.env")]),
        KEYS,
        EnvFile::RealRoute,
        undiscovered,
    );
    std::fs::remove_file(&path).unwrap();
    let from_file = |log: &str| {
        (
            format!("postgres://{CANARY}@file/postgres"),
            Some(log.to_owned()),
            None,
            false,
        )
    };

    assert_eq!(
        (
            discovered.map(|loaded| (summary(&loaded.settings), loaded.env_file)),
            explicit.map(|loaded| summary(&loaded.settings)),
            never.err(),
            missing.err(),
        ),
        (
            Ok((from_file("warn"), Some(path.clone()))),
            Ok(from_file("debug")),
            Some(SettingsError::new("STORYOS_DATABASE_URL", None)),
            Some(SettingsError::new(
                ENV_FILE_VARIABLE,
                Some(std::path::Path::new("/nonexistent/storyos.env"))
            )),
        )
    );
}
