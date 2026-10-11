use storyos_application::{CredentialReference, CredentialResolver, ResolvedCredential};

use crate::keychain::KeychainResolver;

const ENVIRONMENT_PREFIX: &str = "env:";

/// Resolves each Credential Reference form that the Worker supports: the Keychain item of
/// ADR 0048 and the `env:STORYOS_<NAME>` variable of ADR 0050.
pub(crate) struct WorkerCredentials<E> {
    pub(crate) keychain: KeychainResolver,
    pub(crate) environment: EnvironmentResolver<E>,
}

impl<E: Fn(&str) -> Option<String> + Sync> CredentialResolver for WorkerCredentials<E> {
    async fn resolve(&self, reference: &CredentialReference) -> Option<ResolvedCredential> {
        if reference.0.starts_with(ENVIRONMENT_PREFIX) {
            self.environment.resolve(reference).await
        } else {
            self.keychain.resolve(reference).await
        }
    }
}

/// Reads the value of an `env:STORYOS_<NAME>` reference from the Worker process environment
/// through `lookup`. Only a `STORYOS_` variable name is a reference (ADR 0050).
pub(crate) struct EnvironmentResolver<E> {
    pub(crate) lookup: E,
}

impl<E: Fn(&str) -> Option<String> + Sync> CredentialResolver for EnvironmentResolver<E> {
    async fn resolve(&self, reference: &CredentialReference) -> Option<ResolvedCredential> {
        let value = (self.lookup)(variable_name(&reference.0)?)?;
        (!value.is_empty()).then(|| ResolvedCredential::new(value))
    }
}

/// The variable name of an environment Credential Reference.
fn variable_name(reference: &str) -> Option<&str> {
    reference.strip_prefix(ENVIRONMENT_PREFIX).filter(|name| {
        name.strip_prefix("STORYOS_")
            .is_some_and(|rest| !rest.is_empty())
            && name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    })
}

#[cfg(test)]
#[path = "credential_tests.rs"]
mod tests;
