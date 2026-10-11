use std::sync::Mutex;

use storyos_application::{CredentialReference, CredentialResolver};

use super::{EnvironmentResolver, variable_name};

const CREDENTIAL_CANARY: &str = "storyos-credential-canary-env-41d9";

#[test]
fn only_a_storyos_variable_name_is_an_environment_reference() {
    assert_eq!(
        [
            "env:STORYOS_AGENT_PLAN_API_KEY",
            "env:STORYOS_",
            "env:HOME",
            "env:STORYOS_lower",
            "macos-keychain:storyos-volcengine-agent-plan/frankqdwang",
        ]
        .map(variable_name),
        [Some("STORYOS_AGENT_PLAN_API_KEY"), None, None, None, None]
    );
}

#[tokio::test]
async fn an_environment_reference_reads_only_its_storyos_variable() {
    let asked = Mutex::new(Vec::new());
    let resolver = EnvironmentResolver {
        lookup: |name: &str| {
            asked.lock().unwrap().push(name.to_owned());
            match name {
                "STORYOS_AGENT_PLAN_API_KEY" => Some(CREDENTIAL_CANARY.to_owned()),
                _ => Some(String::new()),
            }
        },
    };
    let mut resolved = Vec::new();
    for reference in [
        "env:STORYOS_AGENT_PLAN_API_KEY",
        "env:STORYOS_EMPTY",
        "env:HOME",
    ] {
        let credential = resolver
            .resolve(&CredentialReference(reference.to_owned()))
            .await;
        resolved.push(credential.map(|credential| credential.expose() == CREDENTIAL_CANARY));
    }

    assert_eq!(
        (resolved, asked.into_inner().unwrap()),
        (
            vec![Some(true), None, None],
            vec![
                "STORYOS_AGENT_PLAN_API_KEY".to_owned(),
                "STORYOS_EMPTY".to_owned()
            ]
        )
    );
}
