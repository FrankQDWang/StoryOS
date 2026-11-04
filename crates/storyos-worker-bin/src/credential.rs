use secrecy::SecretString;
use storyos_application::{CredentialReference, CredentialResolver, ResolvedCredential};
use storyos_core::{AGENT_PLAN_CREDENTIAL_REFERENCE, EXA_CREDENTIAL_REFERENCE};

/// Resolves the Credential References of the Agent Plan key and the Exa key from the Worker
/// settings (ADR 0050). Each other reference, and an absent key, is unavailable.
pub(crate) struct SettingsCredentials {
    pub(crate) agent_plan_api_key: Option<SecretString>,
    pub(crate) exa_api_key: Option<SecretString>,
}

impl CredentialResolver for SettingsCredentials {
    async fn resolve(&self, reference: &CredentialReference) -> Option<ResolvedCredential> {
        let key = match reference.0.as_str() {
            AGENT_PLAN_CREDENTIAL_REFERENCE => &self.agent_plan_api_key,
            EXA_CREDENTIAL_REFERENCE => &self.exa_api_key,
            _ => return None,
        };
        key.clone().map(ResolvedCredential::new)
    }
}
