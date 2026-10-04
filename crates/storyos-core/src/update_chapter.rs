//! Pure Core classification for Update Chapter (rename and reorder).

use super::{ProjectLifecycle, TransitionOutcome};
use crate::transition_outcome::reason_codes;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChapterJoin {
    ExactScope,
    Invalid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateChapter {
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
pub struct UpdateChapterApplied {
    pub title: String,
    pub order: u64,
    pub tree_revision: u64,
}

pub type UpdateChapterResult = TransitionOutcome<
    UpdateChapterApplied,
    UpdateChapterNoEffect,
    UpdateChapterConflict,
    UpdateChapterRefusal,
>;

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
    InvalidChapterJoin,
    ArchivedProject,
    InvalidTitle,
    InvalidOrder,
}

reason_codes!(UpdateChapterNoEffect {
    Unchanged => "unchanged",
});

reason_codes!(UpdateChapterConflict {
    StaleTreeRevision => "stale_tree_revision",
});

reason_codes!(UpdateChapterRefusal {
    InvalidChapterJoin => "invalid_chapter_join",
    ArchivedProject => "archived_project",
    InvalidTitle => "invalid_title",
    InvalidOrder => "invalid_order",
});

/// Classify one Update Chapter against exact Scope, Chapter join, expected revision, title, and order.
pub fn update_chapter(command: &UpdateChapter) -> UpdateChapterResult {
    if command.chapter_join == ChapterJoin::Invalid {
        return UpdateChapterResult::Refused(UpdateChapterRefusal::InvalidChapterJoin);
    }
    if command.title.is_empty() || command.title.len() > 1024 {
        return UpdateChapterResult::Refused(UpdateChapterRefusal::InvalidTitle);
    }
    if command.order < 1 || command.order > command.chapter_count {
        return UpdateChapterResult::Refused(UpdateChapterRefusal::InvalidOrder);
    }
    if command.current_lifecycle == ProjectLifecycle::Archived {
        return UpdateChapterResult::Refused(UpdateChapterRefusal::ArchivedProject);
    }
    if command.expected_tree_revision != command.current_tree_revision {
        return UpdateChapterResult::Conflicted(UpdateChapterConflict::StaleTreeRevision);
    }
    if command.title == command.current_title && command.order == command.current_order {
        return UpdateChapterResult::NoEffect(UpdateChapterNoEffect::Unchanged);
    }
    UpdateChapterResult::Applied(UpdateChapterApplied {
        title: command.title.clone(),
        order: command.order,
        tree_revision: command.current_tree_revision + 1,
    })
}

#[cfg(test)]
#[path = "update_chapter_tests.rs"]
mod tests;
