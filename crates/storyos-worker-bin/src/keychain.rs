use storyos_application::{CredentialReference, CredentialResolver, ResolvedCredential};

const KEYCHAIN_PREFIX: &str = "macos-keychain:";

/// Resolves a `macos-keychain:<service>/<account>` Credential Reference from the generic
/// passwords of the login keychain. Resolution is unavailable on another platform (ADR 0039).
pub(crate) struct KeychainResolver;

impl CredentialResolver for KeychainResolver {
    async fn resolve(&self, reference: &CredentialReference) -> Option<ResolvedCredential> {
        let (service, account) = keychain_item(&reference.0)?;
        let (service, account) = (service.to_owned(), account.to_owned());
        tokio::task::spawn_blocking(move || generic_password(&service, &account))
            .await
            .ok()
            .flatten()
    }
}

/// The service and the account of a Keychain Credential Reference.
fn keychain_item(reference: &str) -> Option<(&str, &str)> {
    reference
        .strip_prefix(KEYCHAIN_PREFIX)?
        .split_once('/')
        .filter(|(service, account)| !service.is_empty() && !account.is_empty())
}

#[cfg(target_os = "macos")]
fn generic_password(service: &str, account: &str) -> Option<ResolvedCredential> {
    let bytes = security_framework::passwords::get_generic_password(service, account).ok()?;
    String::from_utf8(bytes).ok().map(ResolvedCredential::new)
}

#[cfg(not(target_os = "macos"))]
fn generic_password(_service: &str, _account: &str) -> Option<ResolvedCredential> {
    None
}

#[cfg(test)]
#[path = "keychain_tests.rs"]
mod tests;
