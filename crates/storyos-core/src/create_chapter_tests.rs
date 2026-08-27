use super::{
    CreateChapter, CreateChapterConflict, CreateChapterCurrent, CreateChapterOpen,
    CreateChapterRefusal, CreateChapterResult, ProjectLifecycle, ProjectPresence, VolumeJoin,
    create_chapter,
};

fn command() -> CreateChapter {
    CreateChapter {
        presence: ProjectPresence::Present,
        volume_join: VolumeJoin::ExactScope,
        expected_tree_revision: 2,
        current_tree_revision: 2,
        current_lifecycle: ProjectLifecycle::Active,
        current_open: CreateChapterOpen::Empty,
        title: "Chapter A".to_owned(),
    }
}

#[test]
fn the_first_chapter_on_an_empty_active_project_becomes_current() {
    assert_eq!(
        create_chapter(&command()),
        CreateChapterResult::Applied {
            tree_revision: 3,
            current: CreateChapterCurrent::SelectCreated,
        }
    );
}

#[test]
fn a_later_chapter_preserves_the_existing_current_chapter() {
    let mut later = command();
    later.expected_tree_revision = 3;
    later.current_tree_revision = 3;
    later.current_open = CreateChapterOpen::CurrentChapter;
    later.title = "Chapter B".to_owned();
    assert_eq!(
        create_chapter(&later),
        CreateChapterResult::Applied {
            tree_revision: 4,
            current: CreateChapterCurrent::PreserveExisting,
        }
    );
}

#[test]
fn a_stale_tree_revision_classifies_as_conflicted_with_zero_authority_effect() {
    let mut stale = command();
    stale.expected_tree_revision = 2;
    stale.current_tree_revision = 3;
    assert_eq!(
        create_chapter(&stale),
        CreateChapterResult::Conflicted {
            reason: CreateChapterConflict::StaleTreeRevision,
        }
    );
}

#[test]
fn an_invalid_volume_join_classifies_as_refused_with_zero_authority_effect() {
    let mut invalid = command();
    invalid.volume_join = VolumeJoin::Invalid;
    assert_eq!(
        create_chapter(&invalid),
        CreateChapterResult::Refused {
            reason: CreateChapterRefusal::InvalidVolumeJoin,
        }
    );
}

#[test]
fn an_archived_project_classifies_as_refused_with_zero_authority_effect() {
    let mut archived = command();
    archived.current_lifecycle = ProjectLifecycle::Archived;
    assert_eq!(
        create_chapter(&archived),
        CreateChapterResult::Refused {
            reason: CreateChapterRefusal::ArchivedProject,
        }
    );
}

#[test]
fn a_missing_project_classifies_as_refused_with_zero_authority_effect() {
    let mut missing = command();
    missing.presence = ProjectPresence::Absent;
    assert_eq!(
        create_chapter(&missing),
        CreateChapterResult::Refused {
            reason: CreateChapterRefusal::MissingProject,
        }
    );
}

#[test]
fn an_invalid_title_classifies_as_refused_with_zero_authority_effect() {
    let mut empty = command();
    empty.title.clear();
    assert_eq!(
        create_chapter(&empty),
        CreateChapterResult::Refused {
            reason: CreateChapterRefusal::InvalidTitle,
        }
    );
    let mut too_long = command();
    too_long.title = "n".repeat(1025);
    assert_eq!(
        create_chapter(&too_long),
        CreateChapterResult::Refused {
            reason: CreateChapterRefusal::InvalidTitle,
        }
    );
}
