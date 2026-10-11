//! The typed settings of the packaged Server (ADR 0050). The Server never reads the env file or
//! the Agent Plan API key.

use secrecy::ExposeSecret as _;
use secrecy::SecretString;
use serde::Deserialize;
use storyos_settings::{EnvFile, Loaded, SettingsError, optional_number};

const KEYS: &[&str] = &[
    "log",
    "database_url",
    "public_origin",
    "bootstrap_sessions",
    "challenge_secret",
    "model_destination",
    "model_qualification_authorization",
    "worker",
    "export_lease_ttl_secs",
    "test_allow_multiple_bootstrap_sessions",
];

/// The `STORYOS_` settings of the Server. Each secret is a `SecretString`.
#[derive(Debug, Deserialize)]
pub struct ServerSettings {
    pub log: Option<String>,
    pub database_url: Option<SecretString>,
    pub public_origin: Option<String>,
    pub bootstrap_sessions: Option<String>,
    pub challenge_secret: Option<SecretString>,
    pub model_destination: Option<String>,
    /// The `https` link to the author authorization of the runtime qualification (ADR 0049).
    pub model_qualification_authorization: Option<String>,
    /// `0` turns off the in-process Worker loop.
    pub worker: Option<String>,
    #[serde(default, deserialize_with = "optional_number")]
    pub export_lease_ttl_secs: Option<u64>,
    pub test_allow_multiple_bootstrap_sessions: Option<String>,
}

impl ServerSettings {
    pub fn load() -> Result<Loaded<Self>, SettingsError> {
        storyos_settings::load(KEYS, EnvFile::Never)
    }

    /// The command challenge secret. A missing secret, or one shorter than 32 bytes, stops startup.
    pub fn required_challenge_secret(&self) -> Result<SecretString, SettingsError> {
        self.challenge_secret
            .clone()
            .filter(|secret| secret.expose_secret().len() >= 32)
            .ok_or_else(|| SettingsError::new("STORYOS_CHALLENGE_SECRET", None))
    }

    /// The deployment model destination (ADR 0048). An absent value offers no assistance.
    pub fn model_destination(
        &self,
    ) -> Result<Option<storyos_core::DeploymentDestination>, SettingsError> {
        let refused = |variable: &str| SettingsError::new(variable, None);
        let authorization = self.model_qualification_authorization.clone();
        match (self.model_destination.as_deref(), &authorization) {
            (None, None) => Ok(None),
            (Some("host_fake"), None) => Ok(Some(storyos_core::DeploymentDestination::HostFake)),
            (Some("volcengine_agent_plan"), _)
                if authorization.as_deref().is_none_or(|link| {
                    link.starts_with("https://") && (9..=500).contains(&link.len())
                }) =>
            {
                Ok(Some(
                    storyos_core::DeploymentDestination::VolcengineAgentPlan {
                        qualification_authorization: authorization,
                    },
                ))
            }
            (Some("host_fake" | "volcengine_agent_plan"), _) => {
                Err(refused("STORYOS_MODEL_QUALIFICATION_AUTHORIZATION"))
            }
            (_, _) => Err(refused("STORYOS_MODEL_DESTINATION")),
        }
    }

    pub fn in_process_worker(&self) -> bool {
        self.worker.as_deref() != Some("0")
    }
}
