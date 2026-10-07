use super::{
    ChapterJoin, ProjectLifecycle, SetCurrentChapter, SetCurrentChapterConflict,
    SetCurrentChapterNoEffect, SetCurrentChapterRefusal, TransitionOutcome, set_current_chapter,
};

const CURRENT: &str = "018f0000-0000-7001-8000-000000000003";
const TARGET: &str = "018f0000-0000-7001-8000-000000000803";
const HEAD: &str = "018f0000-0000-7001-8000-000000000805";

fn command() -> SetCurrentChapter {
    SetCurrentChapter {
        chapter_join: ChapterJoin::ExactScope,
        current_lifecycle: ProjectLifecycle::Active,
        current_chapter_id: Some(CURRENT.to_owned()),
        expected_current_chapter_id: CURRENT.to_owned(),
        target_chapter_id: TARGET.to_owned(),
        expected_target_revision_id: HEAD.to_owned(),
        current_target_revision_id: HEAD.to_owned(),
    }
}

#[test]
fn a_matching_current_chapter_and_target_head_classifies_as_applied() {
    assert_eq!(
        set_current_chapter(&command()),
        TransitionOutcome::Applied(TARGET.to_owned())
    );
}

#[test]
fn an_already_current_target_classifies_as_no_effect() {
    let mut already = command();
    already.target_chapter_id = CURRENT.to_owned();
    assert_eq!(
        set_current_chapter(&already),
        TransitionOutcome::NoEffect(SetCurrentChapterNoEffect::AlreadyCurrent)
    );
}

#[test]
fn a_stale_current_chapter_classifies_as_conflicted_with_zero_authority_effect() {
    let mut stale = command();
    stale.expected_current_chapter_id = TARGET.to_owned();
    assert_eq!(
        set_current_chapter(&stale),
        TransitionOutcome::Conflicted(SetCurrentChapterConflict::StaleCurrentChapter)
    );
}

#[test]
fn a_wrong_target_head_classifies_as_conflicted_with_zero_authority_effect() {
    let mut wrong = command();
    wrong.expected_target_revision_id = "018f0000-0000-7001-8000-000000000999".to_owned();
    assert_eq!(
        set_current_chapter(&wrong),
        TransitionOutcome::Conflicted(SetCurrentChapterConflict::WrongTargetHead)
    );
}

#[test]
fn an_archived_project_classifies_as_refused_with_zero_authority_effect() {
    let mut archived = command();
    archived.current_lifecycle = ProjectLifecycle::Archived;
    assert_eq!(
        set_current_chapter(&archived),
        TransitionOutcome::Refused(SetCurrentChapterRefusal::ArchivedProject)
    );
}

#[test]
fn an_invalid_chapter_join_classifies_as_refused_with_zero_authority_effect() {
    let mut invalid = command();
    invalid.chapter_join = ChapterJoin::Invalid;
    assert_eq!(
        set_current_chapter(&invalid),
        TransitionOutcome::Refused(SetCurrentChapterRefusal::InvalidChapterJoin)
    );
}

#[test]
fn an_empty_project_classifies_as_refused_with_zero_authority_effect() {
    let mut empty = command();
    empty.current_chapter_id = None;
    assert_eq!(
        set_current_chapter(&empty),
        TransitionOutcome::Refused(SetCurrentChapterRefusal::EmptyProject)
    );
}
