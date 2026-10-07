//! Classify one author Discard of a Refused Edit Draft.

use std::convert::Infallible;

use super::TransitionOutcome;
use crate::transition_outcome::reason_codes;

pub type CloseEditorFlowDraftResult =
    TransitionOutcome<(), Infallible, CloseEditorFlowDraftConflict, CloseEditorFlowDraftRefusal>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CloseEditorFlowDraftConflict {
    SourceBindingChanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CloseEditorFlowDraftRefusal {
    SourceDraftNotOpen,
    SourceUnavailable,
}

reason_codes!(CloseEditorFlowDraftConflict {
    SourceBindingChanged => "source_binding_changed",
});
reason_codes!(CloseEditorFlowDraftRefusal {
    SourceDraftNotOpen => "source_draft_not_open",
    SourceUnavailable => "source_unavailable",
});

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
        TransitionOutcome::Refused(CloseEditorFlowDraftRefusal::SourceUnavailable)
    } else if expected.revision != current.revision
        || expected.digest != current.digest
        || expected.reopen_event_id != current.reopen_event_id
    {
        TransitionOutcome::Conflicted(CloseEditorFlowDraftConflict::SourceBindingChanged)
    } else if closure != "open" {
        TransitionOutcome::Refused(CloseEditorFlowDraftRefusal::SourceDraftNotOpen)
    } else {
        TransitionOutcome::Applied(())
    }
}

#[cfg(test)]
#[path = "close_editor_flow_draft_tests.rs"]
mod tests;
