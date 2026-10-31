use super::{
    CreateChapter, CreateChapterApplied, CreateChapterConflict, CreateChapterCurrent,
    CreateChapterOpen, CreateChapterPlacement, CreateChapterRefusal, CreateChapterResult,
    VolumeJoin, create_chapter,
};

fn command() -> CreateChapter {
    CreateChapter {
        placement: super::CreateChapterPlacement::Append,
        ordered_chapter_ids: Vec::new(),
        volume_join: VolumeJoin::ExactScope,
        expected_tree_revision: 2,
        current_tree_revision: 2,
        current_open: CreateChapterOpen::Empty,
        title: "Chapter A".to_owned(),
    }
}

#[test]
fn the_first_chapter_on_an_empty_active_project_becomes_current() {
    assert_eq!(
        create_chapter(&command()),
        CreateChapterResult::Applied(CreateChapterApplied {
            tree_revision: 3,
            current: CreateChapterCurrent::SelectCreated,
            order: 1,
        })
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
        CreateChapterResult::Applied(CreateChapterApplied {
            tree_revision: 4,
            current: CreateChapterCurrent::PreserveExisting,
            order: 1,
        })
    );
}

#[test]
fn a_stale_tree_revision_classifies_as_conflicted_with_zero_authority_effect() {
    let mut stale = command();
    stale.expected_tree_revision = 2;
    stale.current_tree_revision = 3;
    assert_eq!(
        create_chapter(&stale),
        CreateChapterResult::Conflicted(CreateChapterConflict::StaleTreeRevision)
    );
}

#[test]
fn an_invalid_volume_join_classifies_as_refused_with_zero_authority_effect() {
    let mut invalid = command();
    invalid.volume_join = VolumeJoin::Invalid;
    assert_eq!(
        create_chapter(&invalid),
        CreateChapterResult::Refused(CreateChapterRefusal::InvalidVolumeJoin)
    );
}

#[test]
fn an_invalid_title_classifies_as_refused_with_zero_authority_effect() {
    let mut empty = command();
    empty.title.clear();
    assert_eq!(
        create_chapter(&empty),
        CreateChapterResult::Refused(CreateChapterRefusal::InvalidTitle)
    );
    let mut too_long = command();
    too_long.title = "n".repeat(1025);
    assert_eq!(
        create_chapter(&too_long),
        CreateChapterResult::Refused(CreateChapterRefusal::InvalidTitle)
    );
}

#[test]
fn a_placement_takes_the_order_beside_its_live_anchor_and_refuses_an_absent_anchor() {
    let placed = |placement| {
        create_chapter(&CreateChapter {
            placement,
            ordered_chapter_ids: vec!["chapter-a".to_owned(), "chapter-b".to_owned()],
            current_open: CreateChapterOpen::CurrentChapter,
            ..command()
        })
    };
    let applied = |order| {
        CreateChapterResult::Applied(CreateChapterApplied {
            tree_revision: 3,
            current: CreateChapterCurrent::PreserveExisting,
            order,
        })
    };
    let anchor = |id: &str| id.to_owned();
    assert_eq!(
        [
            placed(CreateChapterPlacement::Before {
                chapter_id: anchor("chapter-a")
            }),
            placed(CreateChapterPlacement::After {
                chapter_id: anchor("chapter-a")
            }),
            placed(CreateChapterPlacement::Before {
                chapter_id: anchor("chapter-b")
            }),
            placed(CreateChapterPlacement::After {
                chapter_id: anchor("chapter-b")
            }),
            placed(CreateChapterPlacement::Append),
            placed(CreateChapterPlacement::After {
                chapter_id: anchor("chapter-removed")
            }),
        ],
        [
            applied(1),
            applied(2),
            applied(2),
            applied(3),
            applied(3),
            CreateChapterResult::Refused(CreateChapterRefusal::InvalidPlacement),
        ]
    );
}
