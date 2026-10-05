use super::{
    CloseEditorFlowDraftConflict, CloseEditorFlowDraftRefusal, DraftCloseSource, TransitionOutcome,
    close_editor_flow_draft,
};

const SOURCE: DraftCloseSource<'static> = DraftCloseSource {
    revision: "revision-a",
    digest: "digest-a",
    reopen_event_id: None,
};

#[test]
fn an_open_retained_draft_at_the_named_source_closes() {
    assert_eq!(
        close_editor_flow_draft(&SOURCE, &SOURCE, "open", "retained"),
        TransitionOutcome::Applied(())
    );
}

#[test]
fn a_changed_source_lifecycle_conflicts() {
    let reopened = DraftCloseSource {
        reopen_event_id: Some("reopen-a"),
        ..SOURCE
    };
    assert_eq!(
        close_editor_flow_draft(&SOURCE, &reopened, "open", "retained"),
        TransitionOutcome::Conflicted(CloseEditorFlowDraftConflict::SourceBindingChanged)
    );
}

#[test]
fn a_closed_draft_is_refused_as_not_open() {
    assert_eq!(
        close_editor_flow_draft(&SOURCE, &SOURCE, "closed", "retained"),
        TransitionOutcome::Refused(CloseEditorFlowDraftRefusal::SourceDraftNotOpen)
    );
}

#[test]
fn an_archived_draft_is_refused_as_unavailable_before_its_binding_is_compared() {
    let changed = DraftCloseSource {
        revision: "revision-b",
        ..SOURCE
    };
    assert_eq!(
        close_editor_flow_draft(&SOURCE, &changed, "closed", "archived"),
        TransitionOutcome::Refused(CloseEditorFlowDraftRefusal::SourceUnavailable)
    );
}
