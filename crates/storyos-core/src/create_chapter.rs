//! Pure Core classification for Create Chapter.

use super::{ProjectLifecycle, ProjectPresence};

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
    pub presence: ProjectPresence,
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
pub enum CreateChapterResult {
    Applied {
        tree_revision: u64,
        current: CreateChapterCurrent,
        order: u64,
    },
    Conflicted {
        reason: CreateChapterConflict,
    },
    Refused {
        reason: CreateChapterRefusal,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreateChapterConflict {
    StaleTreeRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreateChapterRefusal {
    MissingProject,
    ArchivedProject,
    InvalidTitle,
    InvalidVolumeJoin,
    InvalidPlacement,
}

/// Classify one Create Chapter against exact Scope, Volume join, lifecycle, tree revision, and title.
pub fn create_chapter(command: &CreateChapter) -> CreateChapterResult {
    if command.presence == ProjectPresence::Absent {
        return CreateChapterResult::Refused {
            reason: CreateChapterRefusal::MissingProject,
        };
    }
    if command.title.is_empty() || command.title.len() > 1024 {
        return CreateChapterResult::Refused {
            reason: CreateChapterRefusal::InvalidTitle,
        };
    }
    if command.current_lifecycle == ProjectLifecycle::Archived {
        return CreateChapterResult::Refused {
            reason: CreateChapterRefusal::ArchivedProject,
        };
    }
    if command.volume_join == VolumeJoin::Invalid {
        return CreateChapterResult::Refused {
            reason: CreateChapterRefusal::InvalidVolumeJoin,
        };
    }
    if command.expected_tree_revision != command.current_tree_revision {
        return CreateChapterResult::Conflicted {
            reason: CreateChapterConflict::StaleTreeRevision,
        };
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
        return CreateChapterResult::Refused {
            reason: CreateChapterRefusal::InvalidPlacement,
        };
    };
    CreateChapterResult::Applied {
        tree_revision: command.current_tree_revision + 1,
        order,
        current: match command.current_open {
            CreateChapterOpen::Empty => CreateChapterCurrent::SelectCreated,
            CreateChapterOpen::CurrentChapter => CreateChapterCurrent::PreserveExisting,
        },
    }
}
