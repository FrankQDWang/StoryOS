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
    /// The Domain Receipt result kind of an outcome with this reason, when the reason type gives
    /// its own kind, for example `invalid`. `None` keeps the result kind of the outcome.
    const RECEIPT_RESULT: Option<&'static str> = None;

    fn code(&self) -> &'static str;
    fn from_code(code: &str) -> Option<Self>;

    /// The Domain Receipt result kind of this reason value. A reason type that has more than one
    /// result kind gives it for each value. `None` keeps the result kind of the outcome.
    fn receipt_result(&self) -> Option<&'static str> {
        Self::RECEIPT_RESULT
    }

    /// The reason code that the Domain Receipt payload records. `None` records no reason.
    fn recorded_code(&self) -> Option<&'static str> {
        Some(self.code())
    }

    /// Decodes a reason from its recorded result kind and reason code. `outcome` is the result
    /// kind of the outcome when the reason gives no kind of its own.
    fn from_receipt(result: &str, outcome: &'static str, code: Option<&str>) -> Option<Self> {
        if result != Self::RECEIPT_RESULT.unwrap_or(outcome) {
            return None;
        }
        Self::from_code(code?)
    }
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

    /// The Domain Receipt result kind of a zero-authority outcome, which its reason type can give.
    /// `Applied` gives `authoritative_applied`. The applied variant of the command can replace it.
    pub fn receipt_result_kind(&self) -> &'static str {
        match self {
            Self::Applied(_) => ReceiptResult::AuthoritativeApplied.code(),
            Self::NoEffect(reason) => reason
                .receipt_result()
                .unwrap_or(ReceiptResult::NoEffect.code()),
            Self::Conflicted(reason) => reason
                .receipt_result()
                .unwrap_or(ReceiptResult::Conflicted.code()),
            Self::Refused(reason) => reason
                .receipt_result()
                .unwrap_or(ReceiptResult::Refused.code()),
        }
    }

    /// The reason code that the Receipt of a zero-authority outcome records. `Applied` has no
    /// reason, and a reason value can record none.
    pub fn reason_code(&self) -> Option<&'static str> {
        match self {
            Self::Applied(_) => None,
            Self::NoEffect(reason) => reason.recorded_code(),
            Self::Conflicted(reason) => reason.recorded_code(),
            Self::Refused(reason) => reason.recorded_code(),
        }
    }

    /// Decodes a zero-authority outcome from its recorded result kind and reason code.
    pub fn from_zero_authority_codes(result: &str, reason: Option<&str>) -> Option<Self> {
        N::from_receipt(result, ReceiptResult::NoEffect.code(), reason)
            .map(Self::NoEffect)
            .or_else(|| {
                C::from_receipt(result, ReceiptResult::Conflicted.code(), reason)
                    .map(Self::Conflicted)
            })
            .or_else(|| {
                R::from_receipt(result, ReceiptResult::Refused.code(), reason).map(Self::Refused)
            })
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
        $crate::transition_outcome::reason_codes!($reason, None, { $($variant => $code),+ });
    };
    ($reason:ty, result $result:literal { $($variant:ident => $code:literal),+ $(,)? }) => {
        $crate::transition_outcome::reason_codes!($reason, Some($result), { $($variant => $code),+ });
    };
    ($reason:ty, $result:expr, { $($variant:ident => $code:literal),+ }) => {
        impl $crate::ReasonCode for $reason {
            const RECEIPT_RESULT: Option<&'static str> = $result;

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
