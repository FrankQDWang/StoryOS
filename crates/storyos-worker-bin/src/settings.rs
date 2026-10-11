use secrecy::SecretString;
use serde::Deserialize;
use storyos_settings::{EnvFile, Loaded, SettingsError, optional_number};

const KEYS: &[&str] = &[
    "log",
    "database_url",
    "export_lease_ttl_secs",
    "agent_plan_api_key",
    "model_destination",
];

/// The `STORYOS_` settings of the Worker (ADR 0050). Each secret is a `SecretString`.
#[derive(Debug, Deserialize)]
pub(crate) struct WorkerSettings {
    pub(crate) log: Option<String>,
    pub(crate) database_url: Option<SecretString>,
    #[serde(default, deserialize_with = "optional_number")]
    pub(crate) export_lease_ttl_secs: Option<u64>,
    /// The operator-quota key of the Agent Plan route. Only the Worker takes it.
    pub(crate) agent_plan_api_key: Option<SecretString>,
    /// `volcengine_agent_plan` marks a real-route run, which needs the key.
    pub(crate) model_destination: Option<String>,
}

impl WorkerSettings {
    /// Loads the settings. A real-route run without the key refuses with the env file that it
    /// read.
    pub(crate) fn load() -> Result<Loaded<Self>, SettingsError> {
        let loaded: Loaded<Self> = storyos_settings::load(KEYS, EnvFile::RealRoute)?;
        if loaded.settings.model_destination.as_deref() == Some("volcengine_agent_plan")
            && loaded.settings.agent_plan_api_key.is_none()
        {
            return Err(SettingsError::new(
                "STORYOS_AGENT_PLAN_API_KEY",
                loaded.env_file.as_deref(),
            ));
        }
        Ok(loaded)
    }
}
