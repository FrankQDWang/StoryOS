use crate::PROJECT_COMMAND_CHALLENGE_RATE_POLICY_REVISION;

const AUTHOR_EDIT_POLICY_REVISION: &str =
    "storyos.project-command-challenge-rate.author-edit.fixed-window.v1";

/// The Server-derived rate budget that a new Command Challenge uses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChallengeRateClass {
    AuthorEdit,
    Shared,
}

impl ChallengeRateClass {
    pub fn for_command_kind(command_kind: &str) -> Self {
        match command_kind {
            "applyAuthorEdit" | "undoLatestAuthorAction" => Self::AuthorEdit,
            _ => Self::Shared,
        }
    }

    /// Returns each class whose revision a Challenge of the command kind can have, the issuing class first.
    ///
    /// The `author_edit` class also accepts the shared revision, so a Challenge issued before
    /// the command kind moved to that class stays usable until it expires.
    pub fn accepted_for(command_kind: &str) -> &'static [Self] {
        match Self::for_command_kind(command_kind) {
            Self::AuthorEdit => &[Self::AuthorEdit, Self::Shared],
            Self::Shared => &[Self::Shared],
        }
    }

    /// Returns the class whose policy issues a Challenge with this revision for the command kind.
    pub fn accepting(command_kind: &str, policy_revision: &str) -> Option<Self> {
        Self::accepted_for(command_kind)
            .iter()
            .copied()
            .find(|class| class.policy_revision() == policy_revision)
    }

    pub fn policy_revision(self) -> &'static str {
        match self {
            Self::AuthorEdit => AUTHOR_EDIT_POLICY_REVISION,
            Self::Shared => PROJECT_COMMAND_CHALLENGE_RATE_POLICY_REVISION,
        }
    }

    /// Returns the inclusive count of new Challenges in one rate window.
    pub fn capacity(self) -> i16 {
        match self {
            Self::AuthorEdit => 120,
            Self::Shared => 10,
        }
    }
}
