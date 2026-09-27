use crate::{
    AuthorEditPrimitive, CloseEditorFlowDraftResult, DraftCloseSource, OpenInlineProposal,
    OpenInlineProposalResult, RefusedEditPayload, ReplacementBlock, close_editor_flow_draft,
    open_inline_proposal,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpandRefusedEditDraftResult {
    ProposalCreated { replacement: Vec<ReplacementBlock> },
    Conflicted,
    SourceDraftNotOpen,
    SourceUnavailable,
    UnsupportedPayload,
    TargetUnavailable,
}

/// Classify the complete retained input and an explicitly selected current target.
pub fn expand_refused_edit_draft(
    expected: &DraftCloseSource<'_>,
    current: &DraftCloseSource<'_>,
    closure: &str,
    retention: &str,
    payload: &RefusedEditPayload,
    target: &OpenInlineProposal,
) -> ExpandRefusedEditDraftResult {
    use ExpandRefusedEditDraftResult as Result;
    match close_editor_flow_draft(expected, current, closure, retention) {
        CloseEditorFlowDraftResult::Conflicted => return Result::Conflicted,
        CloseEditorFlowDraftResult::SourceDraftNotOpen => return Result::SourceDraftNotOpen,
        CloseEditorFlowDraftResult::SourceUnavailable => return Result::SourceUnavailable,
        CloseEditorFlowDraftResult::DraftClosureChanged => {}
    }
    let [unit] = payload.author_edit_units.as_slice() else {
        return Result::UnsupportedPayload;
    };
    let [AuthorEditPrimitive::ReplaceStructuredSelection { replacement }] =
        unit.normalized_primitives.as_slice()
    else {
        return Result::UnsupportedPayload;
    };
    if replacement.is_empty() {
        return Result::UnsupportedPayload;
    }
    match open_inline_proposal(target) {
        OpenInlineProposalResult::Applied => Result::ProposalCreated {
            replacement: replacement.clone(),
        },
        OpenInlineProposalResult::Conflicted { .. } => Result::Conflicted,
        OpenInlineProposalResult::Refused { .. } => Result::TargetUnavailable,
    }
}
