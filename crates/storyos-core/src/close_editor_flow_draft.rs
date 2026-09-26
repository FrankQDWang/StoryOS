#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CloseEditorFlowDraftResult {
    DraftClosureChanged,
    Conflicted,
    SourceDraftNotOpen,
    SourceUnavailable,
}

/// Classify a Discard against the exact retained source before any lifecycle write.
pub fn close_editor_flow_draft(
    expected_revision: &str,
    expected_digest: &str,
    current_revision: &str,
    current_digest: &str,
    closure: &str,
    retention: &str,
) -> CloseEditorFlowDraftResult {
    if retention != "retained" {
        CloseEditorFlowDraftResult::SourceUnavailable
    } else if expected_revision != current_revision || expected_digest != current_digest {
        CloseEditorFlowDraftResult::Conflicted
    } else if closure != "open" {
        CloseEditorFlowDraftResult::SourceDraftNotOpen
    } else {
        CloseEditorFlowDraftResult::DraftClosureChanged
    }
}
