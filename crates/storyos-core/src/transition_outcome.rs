//! The Core Transition Outcome and the stable codes of its reasons.

use std::convert::Infallible;

/// The exhaustive Core classification of one admitted command.
///
/// Only `Applied` changes Authoritative State and allocates an Authoritative Commit and
/// an Author Action. The other outcomes record only a zero-authority Domain Receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransitionOutcome<A, N, C, R> {
    Applied(A),
    NoEffect(N),
    Conflicted(C),
    Refused(R),
}

/// The Domain Receipt result kind that one outcome records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiptResult {
    AuthoritativeApplied,
    NoEffect,
    Conflicted,
    Refused,
}

/// A Core reason with one stable code. Persisted Receipts and public schemas use the same text.
pub trait ReasonCode: Sized {
    fn code(&self) -> &'static str;
    fn from_code(code: &str) -> Option<Self>;
}

impl ReasonCode for Infallible {
    fn code(&self) -> &'static str {
        match *self {}
    }

    fn from_code(_code: &str) -> Option<Self> {
        None
    }
}

impl ReceiptResult {
    pub fn code(self) -> &'static str {
        match self {
            Self::AuthoritativeApplied => "authoritative_applied",
            Self::NoEffect => "no_effect",
            Self::Conflicted => "conflicted",
            Self::Refused => "refused",
        }
    }
}

impl<A, N: ReasonCode, C: ReasonCode, R: ReasonCode> TransitionOutcome<A, N, C, R> {
    pub fn receipt_result(&self) -> ReceiptResult {
        match self {
            Self::Applied(_) => ReceiptResult::AuthoritativeApplied,
            Self::NoEffect(_) => ReceiptResult::NoEffect,
            Self::Conflicted(_) => ReceiptResult::Conflicted,
            Self::Refused(_) => ReceiptResult::Refused,
        }
    }

    /// The reason code of a zero-authority outcome. `Applied` has no reason.
    pub fn reason_code(&self) -> Option<&'static str> {
        match self {
            Self::Applied(_) => None,
            Self::NoEffect(reason) => Some(reason.code()),
            Self::Conflicted(reason) => Some(reason.code()),
            Self::Refused(reason) => Some(reason.code()),
        }
    }

    /// Decodes a zero-authority outcome from its recorded result kind and reason code.
    pub fn from_zero_authority_codes(result: &str, reason: &str) -> Option<Self> {
        match result {
            "no_effect" => N::from_code(reason).map(Self::NoEffect),
            "conflicted" => C::from_code(reason).map(Self::Conflicted),
            "refused" => R::from_code(reason).map(Self::Refused),
            _ => None,
        }
    }

    /// Replaces the applied value and keeps every other outcome.
    pub fn map_applied<B>(self, applied: impl FnOnce(A) -> B) -> TransitionOutcome<B, N, C, R> {
        match self {
            Self::Applied(value) => TransitionOutcome::Applied(applied(value)),
            Self::NoEffect(reason) => TransitionOutcome::NoEffect(reason),
            Self::Conflicted(reason) => TransitionOutcome::Conflicted(reason),
            Self::Refused(reason) => TransitionOutcome::Refused(reason),
        }
    }
}

/// Implements `ReasonCode` for a fieldless reason enum from one variant-to-code table.
macro_rules! reason_codes {
    ($reason:ty { $($variant:ident => $code:literal),+ $(,)? }) => {
        impl $crate::ReasonCode for $reason {
            fn code(&self) -> &'static str {
                match self {
                    $(Self::$variant => $code,)+
                }
            }

            fn from_code(code: &str) -> Option<Self> {
                match code {
                    $($code => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

pub(crate) use reason_codes;
