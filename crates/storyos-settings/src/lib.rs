//! Typed settings of the StoryOS Server and Worker (ADR 0050). The `STORYOS_` variables of the
//! process environment take precedence over the git-ignored env file of a real-route run.

use std::fmt;
use std::path::{Path, PathBuf};

use figment::Figment;
use figment::providers::Serialized;
use figment::value::{Dict, Value};
use serde::Deserialize;
use serde::de::DeserializeOwned;

/// The variable that names the env file explicitly.
pub const ENV_FILE_VARIABLE: &str = "STORYOS_ENV_FILE";

const PREFIX: &str = "STORYOS_";
const DESTINATION_VARIABLE: &str = "STORYOS_MODEL_DESTINATION";
const REAL_DESTINATION: &str = "volcengine_agent_plan";

/// A refusal at startup. It names each wrong or missing variable, and the env file that the
/// settings read, if any. It holds no value.
#[derive(Debug, Eq, PartialEq)]
pub struct SettingsError {
    variables: Vec<String>,
    env_file: Option<PathBuf>,
}

impl SettingsError {
    /// A refusal of one variable that a binary validates after it loads its settings.
    pub fn new(variable: &str, env_file: Option<&Path>) -> Self {
        Self {
            variables: vec![variable.to_owned()],
            env_file: env_file.map(Path::to_path_buf),
        }
    }
}

impl fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "These settings are missing or invalid: {}",
            self.variables.join(", ")
        )?;
        match &self.env_file {
            Some(path) => write!(formatter, " (env file: {})", path.display()),
            None => write!(formatter, " (no env file)"),
        }
    }
}

impl std::error::Error for SettingsError {}

/// The settings of one binary, and the env file that they read.
#[derive(Debug)]
pub struct Loaded<T> {
    pub settings: T,
    pub env_file: Option<PathBuf>,
}

/// Loads `T` from the process environment and, for a real-route run, the env file. Only the
/// variables whose lowercase names without the `STORYOS_` prefix are in `keys` enter the
/// settings.
pub fn load<T: DeserializeOwned>(keys: &[&str]) -> Result<Loaded<T>, SettingsError> {
    load_from(std::env::vars(), keys, discover_env_file)
}

/// Loads `T` from `environment` in place of the process environment. `discover` finds the env
/// file when the environment does not name one.
///
/// Only an explicit `STORYOS_ENV_FILE` or `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` in
/// the environment reads an env file. Thus a test or a fake-destination run never reads the key.
pub fn load_from<T: DeserializeOwned>(
    environment: impl IntoIterator<Item = (String, String)>,
    keys: &[&str],
    discover: impl FnOnce() -> Option<PathBuf>,
) -> Result<Loaded<T>, SettingsError> {
    let environment: Vec<_> = environment.into_iter().collect();
    let value = |variable: &str| {
        environment
            .iter()
            .find(|(name, _)| name == variable)
            .map(|(_, value)| value.as_str())
    };
    let env_file = match (value(ENV_FILE_VARIABLE), value(DESTINATION_VARIABLE)) {
        (Some(path), _) => Some(PathBuf::from(path)),
        (None, Some(REAL_DESTINATION)) => discover(),
        (None, _) => None,
    };
    let file = match &env_file {
        Some(path) => read_env_file(path)?,
        None => Vec::new(),
    };
    let settings = Figment::new()
        .merge(Serialized::defaults(settings_dict(file, keys)))
        .merge(Serialized::defaults(settings_dict(environment, keys)))
        .extract()
        .map_err(|error| SettingsError {
            variables: error
                .into_iter()
                .map(|error| variable_name(&error))
                .collect(),
            env_file: env_file.clone(),
        })?;
    Ok(Loaded { settings, env_file })
}

/// The `.env` of the current checkout, or else the `.env` of the main checkout. A worktree has
/// no copy of the git-ignored file.
fn discover_env_file() -> Option<PathBuf> {
    let git = |argument: &str| {
        let output = std::process::Command::new("git")
            .args(["rev-parse", "--path-format=absolute", argument])
            .output()
            .ok()
            .filter(|output| output.status.success())?;
        let text = String::from_utf8(output.stdout).ok()?;
        Some(PathBuf::from(text.trim()))
    };
    let current = git("--show-toplevel")?.join(".env");
    if current.is_file() {
        return Some(current);
    }
    Some(git("--git-common-dir")?.parent()?.join(".env"))
}

fn read_env_file(path: &Path) -> Result<Vec<(String, String)>, SettingsError> {
    let refused = || SettingsError::new(ENV_FILE_VARIABLE, Some(path));
    dotenvy::from_path_iter(path)
        .map_err(|_| refused())?
        .collect::<Result<_, _>>()
        .map_err(|_| refused())
}

/// Each value stays a string, so a secret that looks like a number keeps its text.
fn settings_dict(pairs: Vec<(String, String)>, keys: &[&str]) -> Dict {
    pairs
        .into_iter()
        .filter_map(|(name, value)| {
            let key = name.strip_prefix(PREFIX)?.to_ascii_lowercase();
            keys.contains(&key.as_str())
                .then(|| (key, Value::from(value)))
        })
        .collect()
}

fn variable_name(error: &figment::Error) -> String {
    let key = match &error.kind {
        figment::error::Kind::MissingField(field) => field.to_string(),
        _ => error.path.last().cloned().unwrap_or_default(),
    };
    format!("{PREFIX}{}", key.to_ascii_uppercase())
}

/// Reads an optional decimal number from its string setting.
pub fn optional_number<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<u64>, D::Error> {
    Option::<String>::deserialize(deserializer)?
        .map(|text| {
            text.parse()
                .map_err(|_| serde::de::Error::custom("not a decimal number"))
        })
        .transpose()
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
