use std::time::Duration;

use storyos_application::{CredentialReference, CredentialResolver, ResolvedCredential};

const KEYCHAIN_PREFIX: &str = "macos-keychain:";

/// A read with no access prompt returns at once. This bound only stops an unexpected hold.
const KEYCHAIN_READ_LIMIT: Duration = Duration::from_secs(/*secs*/ 10);

/// Resolves a `macos-keychain:<service>/<account>` Credential Reference from the generic
/// passwords of the login keychain. Resolution is unavailable on another platform (ADR 0039).
/// It never shows an access prompt: an item that needs one is unavailable.
pub(crate) struct KeychainResolver;

impl CredentialResolver for KeychainResolver {
    async fn resolve(&self, reference: &CredentialReference) -> Option<ResolvedCredential> {
        let (service, account) = keychain_item(&reference.0)?;
        let (service, account) = (service.to_owned(), account.to_owned());
        bounded_read(
            move || generic_password(&service, &account),
            KEYCHAIN_READ_LIMIT,
        )
        .await
    }
}

/// The result of one blocking read, or `None` when it holds longer than `limit`.
async fn bounded_read(
    read: impl FnOnce() -> Option<ResolvedCredential> + Send + 'static,
    limit: Duration,
) -> Option<ResolvedCredential> {
    tokio::time::timeout(limit, tokio::task::spawn_blocking(read))
        .await
        .ok()?
        .ok()
        .flatten()
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
    // The lock turns user interaction back on when it drops.
    let _no_prompt =
        security_framework::os::macos::keychain::SecKeychain::disable_user_interaction().ok()?;
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
