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

    /// Returns the class whose policy issues a Challenge with this revision for the command kind.
    ///
    /// The `author_edit` class also accepts the shared revision, so a Challenge issued before
    /// the command kind moved to that class stays usable until it expires.
    pub fn accepting(command_kind: &str, policy_revision: &str) -> Option<Self> {
        let class = Self::for_command_kind(command_kind);
        if policy_revision == class.policy_revision() {
            return Some(class);
        }
        match class {
            Self::AuthorEdit => {
                (policy_revision == Self::Shared.policy_revision()).then_some(Self::Shared)
            }
            Self::Shared => None,
        }
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
