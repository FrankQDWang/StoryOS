use std::time::Duration;

use super::{bounded_read, keychain_item};

#[test]
fn only_a_complete_keychain_reference_names_an_item() {
    assert_eq!(
        [
            "macos-keychain:storyos-volcengine-agent-plan/frankqdwang",
            "macos-keychain:storyos-volcengine-agent-plan/",
            "macos-keychain:/frankqdwang",
            "env:STORYOS_KEY",
        ]
        .map(keychain_item),
        [
            Some(("storyos-volcengine-agent-plan", "frankqdwang")),
            None,
            None,
            None,
        ]
    );
}

#[tokio::test]
async fn a_credential_read_that_holds_is_unavailable_after_its_bound() {
    let (_hold, held) = std::sync::mpsc::channel::<()>();

    let resolved = bounded_read(
        move || {
            let _ = held.recv();
            None
        },
        Duration::from_millis(/*millis*/ 50),
    )
    .await;

    assert!(resolved.is_none());
}
