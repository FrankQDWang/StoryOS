use super::{
    ChapterJoin, ProjectLifecycle, ProjectPresence, UpdateChapter, UpdateChapterConflict,
    UpdateChapterNoEffect, UpdateChapterRefusal, UpdateChapterResult, update_chapter,
};

fn command() -> UpdateChapter {
    UpdateChapter {
        presence: ProjectPresence::Present,
        chapter_join: ChapterJoin::ExactScope,
        expected_chapter_revision: 2,
        current_chapter_revision: 2,
        current_lifecycle: ProjectLifecycle::Active,
        title: "Chapter B".to_owned(),
        current_title: "Chapter A".to_owned(),
        order: 2,
        current_order: 1,
        chapter_count: 2,
    }
}

#[test]
fn a_matching_revision_rename_and_reorder_classifies_as_applied() {
    assert_eq!(
        update_chapter(&command()),
        UpdateChapterResult::Applied {
            title: "Chapter B".to_owned(),
            order: 2,
            tree_revision: 3,
        }
    );
}

#[test]
fn a_stale_chapter_revision_classifies_as_conflicted_with_zero_authority_effect() {
    let mut stale = command();
    stale.current_chapter_revision = 3;
    assert_eq!(
        update_chapter(&stale),
        UpdateChapterResult::Conflicted {
            reason: UpdateChapterConflict::StaleChapterRevision,
        }
    );
}

#[test]
fn an_unchanged_title_and_order_classifies_as_no_effect() {
    let mut unchanged = command();
    unchanged.title = unchanged.current_title.clone();
    unchanged.order = unchanged.current_order;
    assert_eq!(
        update_chapter(&unchanged),
        UpdateChapterResult::NoEffect {
            reason: UpdateChapterNoEffect::Unchanged,
        }
    );
}

#[test]
fn a_wrong_scope_chapter_classifies_as_refused_with_zero_authority_effect() {
    let mut wrong_scope = command();
    wrong_scope.chapter_join = ChapterJoin::Invalid;
    assert_eq!(
        update_chapter(&wrong_scope),
        UpdateChapterResult::Refused {
            reason: UpdateChapterRefusal::InvalidChapterJoin,
        }
    );
}

#[test]
fn an_invalid_order_classifies_as_refused_with_zero_authority_effect() {
    let mut zero = command();
    zero.order = 0;
    assert_eq!(
        update_chapter(&zero),
        UpdateChapterResult::Refused {
            reason: UpdateChapterRefusal::InvalidOrder,
        }
    );
    let mut past_end = command();
    past_end.order = 3;
    assert_eq!(
        update_chapter(&past_end),
        UpdateChapterResult::Refused {
            reason: UpdateChapterRefusal::InvalidOrder,
        }
    );
}

#[test]
fn an_archived_project_classifies_as_refused_with_zero_authority_effect() {
    let mut archived = command();
    archived.current_lifecycle = ProjectLifecycle::Archived;
    assert_eq!(
        update_chapter(&archived),
        UpdateChapterResult::Refused {
            reason: UpdateChapterRefusal::ArchivedProject,
        }
    );
}

#[test]
fn a_missing_project_classifies_as_refused_with_zero_authority_effect() {
    let mut missing = command();
    missing.presence = ProjectPresence::Absent;
    assert_eq!(
        update_chapter(&missing),
        UpdateChapterResult::Refused {
            reason: UpdateChapterRefusal::MissingProject,
        }
    );
}

#[test]
fn an_invalid_title_classifies_as_refused_with_zero_authority_effect() {
    let mut empty = command();
    empty.title.clear();
    assert_eq!(
        update_chapter(&empty),
        UpdateChapterResult::Refused {
            reason: UpdateChapterRefusal::InvalidTitle,
        }
    );
    let mut too_long = command();
    too_long.title = "n".repeat(1025);
    assert_eq!(
        update_chapter(&too_long),
        UpdateChapterResult::Refused {
            reason: UpdateChapterRefusal::InvalidTitle,
        }
    );
}
