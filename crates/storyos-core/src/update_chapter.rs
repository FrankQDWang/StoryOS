//! Pure Core classification for Update Chapter (rename and reorder).

use super::{ProjectLifecycle, ProjectPresence};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChapterJoin {
    ExactScope,
    Invalid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateChapter {
    pub presence: ProjectPresence,
    pub chapter_join: ChapterJoin,
    pub expected_tree_revision: u64,
    pub current_tree_revision: u64,
    pub current_lifecycle: ProjectLifecycle,
    pub title: String,
    pub current_title: String,
    pub order: u64,
    pub current_order: u64,
    pub chapter_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateChapterResult {
    Applied {
        title: String,
        order: u64,
        tree_revision: u64,
    },
    NoEffect {
        reason: UpdateChapterNoEffect,
    },
    Conflicted {
        reason: UpdateChapterConflict,
    },
    Refused {
        reason: UpdateChapterRefusal,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateChapterNoEffect {
    Unchanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateChapterConflict {
    StaleTreeRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateChapterRefusal {
    MissingProject,
    InvalidChapterJoin,
    ArchivedProject,
    InvalidTitle,
    InvalidOrder,
}

/// Classify one Update Chapter against exact Scope, Chapter join, expected revision, title, and order.
pub fn update_chapter(command: &UpdateChapter) -> UpdateChapterResult {
    if command.presence == ProjectPresence::Absent {
        return UpdateChapterResult::Refused {
            reason: UpdateChapterRefusal::MissingProject,
        };
    }
    if command.chapter_join == ChapterJoin::Invalid {
        return UpdateChapterResult::Refused {
            reason: UpdateChapterRefusal::InvalidChapterJoin,
        };
    }
    if command.title.is_empty() || command.title.len() > 1024 {
        return UpdateChapterResult::Refused {
            reason: UpdateChapterRefusal::InvalidTitle,
        };
    }
    if command.order < 1 || command.order > command.chapter_count {
        return UpdateChapterResult::Refused {
            reason: UpdateChapterRefusal::InvalidOrder,
        };
    }
    if command.current_lifecycle == ProjectLifecycle::Archived {
        return UpdateChapterResult::Refused {
            reason: UpdateChapterRefusal::ArchivedProject,
        };
    }
    if command.expected_tree_revision != command.current_tree_revision {
        return UpdateChapterResult::Conflicted {
            reason: UpdateChapterConflict::StaleTreeRevision,
        };
    }
    if command.title == command.current_title && command.order == command.current_order {
        return UpdateChapterResult::NoEffect {
            reason: UpdateChapterNoEffect::Unchanged,
        };
    }
    UpdateChapterResult::Applied {
        title: command.title.clone(),
        order: command.order,
        tree_revision: command.current_tree_revision + 1,
    }
}

#[cfg(test)]
#[path = "update_chapter_tests.rs"]
mod tests;
