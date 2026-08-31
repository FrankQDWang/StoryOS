//! Pure Core classification for author-initiated Chapter removal.

use super::{ChapterJoin, ProjectLifecycle, ProjectPresence};

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
    pub presence: ProjectPresence,
    pub chapter_join: ChapterJoin,
    pub chapter_lifecycle: ChapterRemovalLifecycle,
    pub expected_chapter_revision: u64,
    pub current_chapter_revision: u64,
    pub current_lifecycle: ProjectLifecycle,
    pub chapter_id: String,
    pub current_chapter_id: Option<String>,
    pub ordered_active_chapter_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeleteChapterResult {
    Applied {
        tree_revision: u64,
        current: DeleteChapterCurrent,
    },
    NoEffect {
        reason: DeleteChapterNoEffect,
    },
    Conflicted {
        reason: DeleteChapterConflict,
    },
    Refused {
        reason: DeleteChapterRefusal,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeleteChapterNoEffect {
    AlreadyRemoved,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeleteChapterConflict {
    StaleChapterRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeleteChapterRefusal {
    MissingProject,
    InvalidChapterJoin,
    ArchivedProject,
}

/// Classify one Chapter removal against exact Scope, join, revision, and remaining manuscript order.
pub fn delete_chapter(command: &DeleteChapter) -> DeleteChapterResult {
    if command.presence == ProjectPresence::Absent {
        return DeleteChapterResult::Refused {
            reason: DeleteChapterRefusal::MissingProject,
        };
    }
    if command.chapter_join == ChapterJoin::Invalid {
        return DeleteChapterResult::Refused {
            reason: DeleteChapterRefusal::InvalidChapterJoin,
        };
    }
    if command.current_lifecycle == ProjectLifecycle::Archived {
        return DeleteChapterResult::Refused {
            reason: DeleteChapterRefusal::ArchivedProject,
        };
    }
    if command.expected_chapter_revision != command.current_chapter_revision {
        return DeleteChapterResult::Conflicted {
            reason: DeleteChapterConflict::StaleChapterRevision,
        };
    }
    if command.chapter_lifecycle == ChapterRemovalLifecycle::Removed {
        return DeleteChapterResult::NoEffect {
            reason: DeleteChapterNoEffect::AlreadyRemoved,
        };
    }
    DeleteChapterResult::Applied {
        tree_revision: command.current_chapter_revision + 1,
        current: successor(command),
    }
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
