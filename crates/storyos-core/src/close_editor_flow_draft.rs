#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CloseEditorFlowDraftResult {
    DraftClosureChanged,
    Conflicted,
    SourceDraftNotOpen,
    SourceUnavailable,
}

/// The retained Revision and open lifecycle that a Discard names.
pub struct DraftCloseSource<'a> {
    pub revision: &'a str,
    pub digest: &'a str,
    pub reopen_event_id: Option<&'a str>,
}

/// Classify a Discard against the exact retained source before any lifecycle write.
pub fn close_editor_flow_draft(
    expected: &DraftCloseSource<'_>,
    current: &DraftCloseSource<'_>,
    closure: &str,
    retention: &str,
) -> CloseEditorFlowDraftResult {
    if retention != "retained" {
        CloseEditorFlowDraftResult::SourceUnavailable
    } else if expected.revision != current.revision
        || expected.digest != current.digest
        || expected.reopen_event_id != current.reopen_event_id
    {
        CloseEditorFlowDraftResult::Conflicted
    } else if closure != "open" {
        CloseEditorFlowDraftResult::SourceDraftNotOpen
    } else {
        CloseEditorFlowDraftResult::DraftClosureChanged
    }
}
