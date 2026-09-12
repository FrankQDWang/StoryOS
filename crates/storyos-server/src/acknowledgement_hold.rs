// Release-package HTTP tests cannot use cfg(test).
pub(super) async fn hold_first_acknowledgement_if_requested(idempotency_key: &str) {
    let Ok(expected) = std::env::var("STORYOS_TEST_ACK_HOLD_IDEMPOTENCY_KEY") else {
        return;
    };
    if expected != idempotency_key {
        return;
    }
    let Ok(path) = std::env::var("STORYOS_TEST_ACK_HOLD_PATH") else {
        return;
    };
    let path = std::path::PathBuf::from(path);
    while path.exists() {
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
}
