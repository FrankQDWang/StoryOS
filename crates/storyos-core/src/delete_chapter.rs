//! Pure Core classification for author-initiated Chapter removal.

use super::{ChapterJoin, ProjectLifecycle, TransitionOutcome};
use crate::transition_outcome::reason_codes;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChapterRemovalLifecycle {
    Active,
    Removed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeleteChapterCurrent {
    PreserveExisting,
    SelectSuccessor { chapter_id: String },
    Empty,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteChapter {
    pub chapter_join: ChapterJoin,
    pub chapter_lifecycle: ChapterRemovalLifecycle,
    pub expected_tree_revision: u64,
    pub current_tree_revision: u64,
    pub current_lifecycle: ProjectLifecycle,
    pub chapter_id: String,
    pub current_chapter_id: Option<String>,
    pub ordered_active_chapter_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteChapterApplied {
    pub tree_revision: u64,
    pub current: DeleteChapterCurrent,
}

pub type DeleteChapterResult = TransitionOutcome<
    DeleteChapterApplied,
    DeleteChapterNoEffect,
    DeleteChapterConflict,
    DeleteChapterRefusal,
>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeleteChapterNoEffect {
    AlreadyRemoved,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeleteChapterConflict {
    StaleTreeRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeleteChapterRefusal {
    InvalidChapterJoin,
    ArchivedProject,
}

reason_codes!(DeleteChapterNoEffect {
    AlreadyRemoved => "already_removed",
});

reason_codes!(DeleteChapterConflict {
    StaleTreeRevision => "stale_tree_revision",
});

reason_codes!(DeleteChapterRefusal {
    InvalidChapterJoin => "invalid_chapter_join",
    ArchivedProject => "archived_project",
});

/// Classify one Chapter removal against exact Scope, join, revision, and remaining manuscript order.
pub fn delete_chapter(command: &DeleteChapter) -> DeleteChapterResult {
    if command.chapter_join == ChapterJoin::Invalid {
        return DeleteChapterResult::Refused(DeleteChapterRefusal::InvalidChapterJoin);
    }
    if command.current_lifecycle == ProjectLifecycle::Archived {
        return DeleteChapterResult::Refused(DeleteChapterRefusal::ArchivedProject);
    }
    if command.expected_tree_revision != command.current_tree_revision {
        return DeleteChapterResult::Conflicted(DeleteChapterConflict::StaleTreeRevision);
    }
    if command.chapter_lifecycle == ChapterRemovalLifecycle::Removed {
        return DeleteChapterResult::NoEffect(DeleteChapterNoEffect::AlreadyRemoved);
    }
    DeleteChapterResult::Applied(DeleteChapterApplied {
        tree_revision: command.current_tree_revision + 1,
        current: successor(command),
    })
}

fn successor(command: &DeleteChapter) -> DeleteChapterCurrent {
    if command.current_chapter_id.as_deref() != Some(command.chapter_id.as_str()) {
        return DeleteChapterCurrent::PreserveExisting;
    }
    let Some(index) = command
        .ordered_active_chapter_ids
        .iter()
        .position(|chapter_id| chapter_id == &command.chapter_id)
    else {
        return DeleteChapterCurrent::Empty;
    };
    if let Some(next) = command.ordered_active_chapter_ids.get(index + 1) {
        return DeleteChapterCurrent::SelectSuccessor {
            chapter_id: next.clone(),
        };
    }
    if index > 0 {
        return DeleteChapterCurrent::SelectSuccessor {
            chapter_id: command.ordered_active_chapter_ids[index - 1].clone(),
        };
    }
    DeleteChapterCurrent::Empty
}

#[cfg(test)]
#[path = "delete_chapter_tests.rs"]
mod tests;
