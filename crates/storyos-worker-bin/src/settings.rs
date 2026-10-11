use secrecy::{ExposeSecret as _, SecretString};
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
        let refused =
            |variable: &str| Err(SettingsError::new(variable, loaded.env_file.as_deref()));
        let key = loaded
            .settings
            .agent_plan_api_key
            .as_ref()
            .map(|key| key.expose_secret().is_empty());
        match (loaded.settings.model_destination.as_deref(), key) {
            (_, Some(true)) | (Some("volcengine_agent_plan"), None) => {
                return refused("STORYOS_AGENT_PLAN_API_KEY");
            }
            (None | Some("host_fake" | "volcengine_agent_plan"), _) => {}
            (Some(_), _) => return refused("STORYOS_MODEL_DESTINATION"),
        }
        if loaded
            .settings
            .database_url
            .as_ref()
            .is_some_and(|url| !storyos_adapter_postgres::database_url_parses(url.expose_secret()))
        {
            return refused("STORYOS_DATABASE_URL");
        }
        Ok(loaded)
    }
}
