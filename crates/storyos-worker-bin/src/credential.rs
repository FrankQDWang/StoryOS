use secrecy::SecretString;
use storyos_application::{CredentialReference, CredentialResolver, ResolvedCredential};
use storyos_core::AGENT_PLAN_CREDENTIAL_REFERENCE;

/// Resolves the Credential Reference of the Agent Plan key from the Worker settings (ADR 0050).
/// Each other reference, and an absent key, is unavailable.
pub(crate) struct SettingsCredentials {
    pub(crate) agent_plan_api_key: Option<SecretString>,
}

impl CredentialResolver for SettingsCredentials {
    async fn resolve(&self, reference: &CredentialReference) -> Option<ResolvedCredential> {
        if reference.0 != AGENT_PLAN_CREDENTIAL_REFERENCE {
            return None;
        }
        self.agent_plan_api_key.clone().map(ResolvedCredential::new)
    }
}
