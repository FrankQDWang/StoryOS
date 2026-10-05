//! Classify one author expansion of a Refused Edit Draft to an inline edit Proposal.

use std::convert::Infallible;

use crate::transition_outcome::reason_codes;
use crate::{
    AuthorEditPrimitive, CloseEditorFlowDraftRefusal, DraftCloseSource, OpenInlineProposal,
    OpenInlineProposalResult, RefusedEditPayload, ReplacementBlock, TransitionOutcome,
    close_editor_flow_draft, open_inline_proposal,
};

/// The applied value is the replacement that the new Proposal Revision proposes.
pub type ExpandRefusedEditDraftResult = TransitionOutcome<
    Vec<ReplacementBlock>,
    Infallible,
    ExpandRefusedEditDraftConflict,
    ExpandRefusedEditDraftRefusal,
>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpandRefusedEditDraftConflict {
    SourceOrTargetChanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpandRefusedEditDraftRefusal {
    SourceDraftNotOpen,
    SourceUnavailable,
    UnsupportedPayload,
    TargetUnavailable,
}

reason_codes!(ExpandRefusedEditDraftConflict {
    SourceOrTargetChanged => "source_or_target_changed",
});
reason_codes!(ExpandRefusedEditDraftRefusal {
    SourceDraftNotOpen => "source_draft_not_open",
    SourceUnavailable => "source_unavailable",
    UnsupportedPayload => "unsupported_payload",
    TargetUnavailable => "target_unavailable",
});

/// Classify the complete retained input and an explicitly selected current target.
///
/// `retained_payload` is read only when the source Draft binding applies.
pub fn expand_refused_edit_draft<E>(
    expected: &DraftCloseSource<'_>,
    current: &DraftCloseSource<'_>,
    closure: &str,
    retention: &str,
    retained_payload: impl FnOnce() -> Result<RefusedEditPayload, E>,
    target: &OpenInlineProposal,
) -> Result<ExpandRefusedEditDraftResult, E> {
    use ExpandRefusedEditDraftRefusal as Refusal;
    let conflicted =
        TransitionOutcome::Conflicted(ExpandRefusedEditDraftConflict::SourceOrTargetChanged);
    match close_editor_flow_draft(expected, current, closure, retention) {
        TransitionOutcome::NoEffect(reason) => match reason {},
        TransitionOutcome::Conflicted(_) => return Ok(conflicted),
        TransitionOutcome::Refused(CloseEditorFlowDraftRefusal::SourceDraftNotOpen) => {
            return Ok(TransitionOutcome::Refused(Refusal::SourceDraftNotOpen));
        }
        TransitionOutcome::Refused(CloseEditorFlowDraftRefusal::SourceUnavailable) => {
            return Ok(TransitionOutcome::Refused(Refusal::SourceUnavailable));
        }
        TransitionOutcome::Applied(()) => {}
    }
    let payload = retained_payload()?;
    let [unit] = payload.author_edit_units.as_slice() else {
        return Ok(TransitionOutcome::Refused(Refusal::UnsupportedPayload));
    };
    let [AuthorEditPrimitive::ReplaceStructuredSelection { replacement }] =
        unit.normalized_primitives.as_slice()
    else {
        return Ok(TransitionOutcome::Refused(Refusal::UnsupportedPayload));
    };
    if replacement.is_empty() {
        return Ok(TransitionOutcome::Refused(Refusal::UnsupportedPayload));
    }
    Ok(match open_inline_proposal(target) {
        OpenInlineProposalResult::Applied => TransitionOutcome::Applied(replacement.clone()),
        OpenInlineProposalResult::Conflicted { .. } => conflicted,
        OpenInlineProposalResult::Refused { .. } => {
            TransitionOutcome::Refused(Refusal::TargetUnavailable)
        }
    })
}

#[cfg(test)]
#[path = "expand_refused_edit_draft_tests.rs"]
mod tests;
