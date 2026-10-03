//! Pure Core classification for Create Chapter.

use super::{ProjectLifecycle, TransitionOutcome};
use crate::transition_outcome::reason_codes;
use std::convert::Infallible;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VolumeJoin {
    ExactScope,
    Invalid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreateChapterOpen {
    Empty,
    CurrentChapter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreateChapterCurrent {
    SelectCreated,
    PreserveExisting,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreateChapterPlacement {
    Append,
    Before { chapter_id: String },
    After { chapter_id: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateChapter {
    pub volume_join: VolumeJoin,
    pub expected_tree_revision: u64,
    pub current_tree_revision: u64,
    pub current_lifecycle: ProjectLifecycle,
    pub current_open: CreateChapterOpen,
    pub title: String,
    pub placement: CreateChapterPlacement,
    pub ordered_chapter_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateChapterApplied {
    pub tree_revision: u64,
    pub current: CreateChapterCurrent,
    pub order: u64,
}

pub type CreateChapterResult = TransitionOutcome<
    CreateChapterApplied,
    Infallible,
    CreateChapterConflict,
    CreateChapterRefusal,
>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreateChapterConflict {
    StaleTreeRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreateChapterRefusal {
    ArchivedProject,
    InvalidTitle,
    InvalidVolumeJoin,
    InvalidPlacement,
}

reason_codes!(CreateChapterConflict {
    StaleTreeRevision => "stale_tree_revision",
});

reason_codes!(CreateChapterRefusal {
    ArchivedProject => "archived_project",
    InvalidTitle => "invalid_title",
    InvalidVolumeJoin => "invalid_volume_join",
    InvalidPlacement => "invalid_placement",
});

/// Classify one Create Chapter against exact Scope, Volume join, lifecycle, tree revision, and title.
pub fn create_chapter(command: &CreateChapter) -> CreateChapterResult {
    if command.title.is_empty() || command.title.len() > 1024 {
        return CreateChapterResult::Refused(CreateChapterRefusal::InvalidTitle);
    }
    if command.current_lifecycle == ProjectLifecycle::Archived {
        return CreateChapterResult::Refused(CreateChapterRefusal::ArchivedProject);
    }
    if command.volume_join == VolumeJoin::Invalid {
        return CreateChapterResult::Refused(CreateChapterRefusal::InvalidVolumeJoin);
    }
    if command.expected_tree_revision != command.current_tree_revision {
        return CreateChapterResult::Conflicted(CreateChapterConflict::StaleTreeRevision);
    }
    let order = match &command.placement {
        CreateChapterPlacement::Append => Some(command.ordered_chapter_ids.len() as u64 + 1),
        CreateChapterPlacement::Before { chapter_id } => command
            .ordered_chapter_ids
            .iter()
            .position(|id| id == chapter_id)
            .map(|index| index as u64 + 1),
        CreateChapterPlacement::After { chapter_id } => command
            .ordered_chapter_ids
            .iter()
            .position(|id| id == chapter_id)
            .map(|index| index as u64 + 2),
    };
    let Some(order) = order else {
        return CreateChapterResult::Refused(CreateChapterRefusal::InvalidPlacement);
    };
    CreateChapterResult::Applied(CreateChapterApplied {
        tree_revision: command.current_tree_revision + 1,
        order,
        current: match command.current_open {
            CreateChapterOpen::Empty => CreateChapterCurrent::SelectCreated,
            CreateChapterOpen::CurrentChapter => CreateChapterCurrent::PreserveExisting,
        },
    })
}
