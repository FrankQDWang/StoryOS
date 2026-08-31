use super::{
    ChapterJoin, ChapterRemovalLifecycle, DeleteChapter, DeleteChapterConflict,
    DeleteChapterCurrent, DeleteChapterNoEffect, DeleteChapterRefusal, DeleteChapterResult,
    ProjectLifecycle, ProjectPresence, delete_chapter,
};

fn command() -> DeleteChapter {
    DeleteChapter {
        presence: ProjectPresence::Present,
        chapter_join: ChapterJoin::ExactScope,
        chapter_lifecycle: ChapterRemovalLifecycle::Active,
        expected_chapter_revision: 4,
        current_chapter_revision: 4,
        current_lifecycle: ProjectLifecycle::Active,
        chapter_id: "b".to_owned(),
        current_chapter_id: Some("a".to_owned()),
        ordered_active_chapter_ids: vec!["a".to_owned(), "b".to_owned(), "c".to_owned()],
    }
}

#[test]
fn removing_a_non_current_chapter_preserves_the_current_chapter() {
    assert_eq!(
        delete_chapter(&command()),
        DeleteChapterResult::Applied {
            tree_revision: 5,
            current: DeleteChapterCurrent::PreserveExisting,
        }
    );
}

#[test]
fn removing_the_current_chapter_selects_the_next_remaining_chapter() {
    let mut current = command();
    current.chapter_id = "b".to_owned();
    current.current_chapter_id = Some("b".to_owned());
    assert_eq!(
        delete_chapter(&current),
        DeleteChapterResult::Applied {
            tree_revision: 5,
            current: DeleteChapterCurrent::SelectSuccessor {
                chapter_id: "c".to_owned(),
            },
        }
    );
}

#[test]
fn removing_the_last_current_chapter_selects_the_previous_remaining_chapter() {
    let mut last = command();
    last.chapter_id = "c".to_owned();
    last.current_chapter_id = Some("c".to_owned());
    assert_eq!(
        delete_chapter(&last),
        DeleteChapterResult::Applied {
            tree_revision: 5,
            current: DeleteChapterCurrent::SelectSuccessor {
                chapter_id: "b".to_owned(),
            },
        }
    );
}

#[test]
fn removing_the_only_current_chapter_opens_an_explicit_empty_state() {
    let only = DeleteChapter {
        chapter_id: "a".to_owned(),
        current_chapter_id: Some("a".to_owned()),
        ordered_active_chapter_ids: vec!["a".to_owned()],
        ..command()
    };
    assert_eq!(
        delete_chapter(&only),
        DeleteChapterResult::Applied {
            tree_revision: 5,
            current: DeleteChapterCurrent::Empty,
        }
    );
}

#[test]
fn an_already_removed_chapter_classifies_as_no_effect() {
    let mut removed = command();
    removed.chapter_lifecycle = ChapterRemovalLifecycle::Removed;
    removed.ordered_active_chapter_ids = vec!["a".to_owned(), "c".to_owned()];
    assert_eq!(
        delete_chapter(&removed),
        DeleteChapterResult::NoEffect {
            reason: DeleteChapterNoEffect::AlreadyRemoved,
        }
    );
}

#[test]
fn a_stale_chapter_revision_classifies_as_conflicted_with_zero_authority_effect() {
    let mut stale = command();
    stale.expected_chapter_revision = 3;
    assert_eq!(
        delete_chapter(&stale),
        DeleteChapterResult::Conflicted {
            reason: DeleteChapterConflict::StaleChapterRevision,
        }
    );
}

#[test]
fn an_invalid_chapter_join_classifies_as_refused_with_zero_authority_effect() {
    let mut invalid = command();
    invalid.chapter_join = ChapterJoin::Invalid;
    assert_eq!(
        delete_chapter(&invalid),
        DeleteChapterResult::Refused {
            reason: DeleteChapterRefusal::InvalidChapterJoin,
        }
    );
}

#[test]
fn an_archived_project_classifies_as_refused_with_zero_authority_effect() {
    let mut archived = command();
    archived.current_lifecycle = ProjectLifecycle::Archived;
    assert_eq!(
        delete_chapter(&archived),
        DeleteChapterResult::Refused {
            reason: DeleteChapterRefusal::ArchivedProject,
        }
    );
}

#[test]
fn a_missing_project_classifies_as_refused_with_zero_authority_effect() {
    let mut missing = command();
    missing.presence = ProjectPresence::Absent;
    assert_eq!(
        delete_chapter(&missing),
        DeleteChapterResult::Refused {
            reason: DeleteChapterRefusal::MissingProject,
        }
    );
}
