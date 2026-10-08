//! The applied variant that a command declares (ADR 0044).

use crate::undo_compensation::ForwardCommand;

/// The applied variant of a command: its Receipt result kind and its Author Action disposition
/// (ADR 0044).
#[derive(Clone, Copy)]
pub(crate) enum AppliedVariant {
    /// The applied outcome writes no Author Action. Its Receipt records `authoritative_applied`.
    NoAuthorAction,
    /// The applied outcome writes a Forward Author Action of this Forward command kind. The kind
    /// gives the Receipt result kind and the Author Undo Disposition.
    Forward(ForwardCommand),
    /// The applied outcome writes a Compensation Author Action, and its Receipt records this
    /// result kind.
    Compensation(&'static str),
}

impl AppliedVariant {
    pub(crate) const fn result_kind(self) -> &'static str {
        match self {
            Self::NoAuthorAction => "authoritative_applied",
            Self::Forward(forward) => forward.result_kind(),
            Self::Compensation(result_kind) => result_kind,
        }
    }

    /// Whether each Forward variant of a command declaration names the Forward command of its
    /// command kind.
    pub(super) const fn all_name_kind(variants: &[Self], kind: &str) -> bool {
        let mut index = 0;
        while index < variants.len() {
            if !variants[index].names_kind(kind) {
                return false;
            }
            index += 1;
        }
        true
    }

    const fn names_kind(self, kind: &str) -> bool {
        match self {
            Self::NoAuthorAction | Self::Compensation(_) => true,
            Self::Forward(forward) => {
                let (expected, actual) = (forward.command_kind().as_bytes(), kind.as_bytes());
                if expected.len() != actual.len() {
                    return false;
                }
                let mut index = 0;
                while index < expected.len() {
                    if expected[index] != actual[index] {
                        return false;
                    }
                    index += 1;
                }
                true
            }
        }
    }
}
