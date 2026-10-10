use super::keychain_item;

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
