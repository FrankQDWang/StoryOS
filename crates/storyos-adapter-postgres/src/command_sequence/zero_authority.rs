//! The zero-authority rows of a project command (ADR 0076, ADR 0080).

use std::future::Future;

use storyos_application::{ProjectCommandEnvelope, ProjectCommandError};
use storyos_core::{ReasonCode, ReceiptResult};
use tokio_postgres::Client;

use super::{
    ActivitySequences, Conflict, LockedProject, NoEffect, ProjectCommand, ReceiptRefs, Refusal,
    ZeroEffect, unavailable,
};
use crate::command_replay::{CommandReplay, ReplayFault};

/// The effect and Activity rows that the zero-authority outcomes of the command `C` write.
///
/// Only a command that writes such rows implements it, and its `ProjectCommand::zero_authority`
/// returns the implementation. The sequence calls it after the Domain Receipt of a
/// zero-authority outcome, and on an exact retry of that outcome.
pub(crate) trait ZeroAuthority<C: ProjectCommand + ?Sized> {
    /// The rows that this zero-authority outcome writes (ADR 0076).
    fn zero_authority_rows(&self, _outcome: &ZeroOutcome<'_, C>) -> ZeroAuthorityRows {
        ZeroAuthorityRows::None
    }

    /// Writes the effect rows of a zero-authority outcome that declares `Effect`.
    fn write_zero_authority_effect(
        &self,
        _client: &Client,
        _envelope: &ProjectCommandEnvelope,
        _outcome: &ZeroOutcome<'_, C>,
    ) -> impl Future<Output = Result<ZeroEffect<C>, ProjectCommandError>> + Send {
        async { Err(unavailable("the command writes no zero-authority effect")) }
    }

    /// Writes the effect rows of a zero-authority outcome that declares `EffectWithActivity`, at
    /// its allocated Activity position.
    fn write_zero_authority_activity(
        &self,
        _client: &Client,
        _envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        _activity: &ActivitySequences,
    ) -> impl Future<Output = Result<ZeroAuthorityWrite<ZeroEffect<C>>, ProjectCommandError>> + Send
    {
        async { Err(unavailable("the command writes no zero-authority Activity")) }
    }

    /// Decodes the effect of a zero-authority outcome from the stored effect rows.
    fn decode_zero_authority_effect(
        &self,
        _replay: &CommandReplay,
    ) -> Result<Option<ZeroEffect<C>>, ReplayFault> {
        Ok(None)
    }
}

/// The zero-authority side of a command that writes no zero-authority rows. It has no value.
pub(crate) enum NoZeroAuthority {}

impl<C: ProjectCommand + ?Sized> ZeroAuthority<C> for NoZeroAuthority {}

/// The Domain Receipt payload and references of a zero-authority outcome.
pub(crate) enum ZeroReceipt<Z> {
    /// The payload `{"reason": code}` and no references.
    Reason,
    /// The payload fields that the command observed, which the sequence completes with `reason`.
    ///
    /// The settlement returns `effect`, unless the outcome writes effect rows, which give it.
    Observed {
        fields: serde_json::Map<String, serde_json::Value>,
        refs: ReceiptRefs,
        effect: Z,
    },
}

/// A zero-authority outcome, which a command can settle with effect rows (ADR 0076).
pub(crate) enum ZeroOutcome<'a, C: ProjectCommand + ?Sized> {
    NoEffect(&'a NoEffect<C>),
    Conflicted(&'a Conflict<C>),
    Refused(&'a Refusal<C>),
}

impl<C: ProjectCommand + ?Sized> ZeroOutcome<'_, C> {
    /// The Domain Receipt result kind of the outcome, which its reason type can give.
    pub(crate) fn receipt_result_kind(&self) -> &'static str {
        match self {
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
}

/// The rows that one zero-authority outcome writes besides its Admission, Receipt, and fence.
pub(crate) enum ZeroAuthorityRows {
    None,
    /// Effect rows without a Project Activity record.
    Effect,
    /// Effect rows and one Activity record at a position that the sequence allocates.
    EffectWithActivity,
}

/// The effect and the whole Activity payload of a zero-authority outcome that writes effect rows.
pub(crate) struct ZeroAuthorityWrite<Z> {
    pub(crate) effect: Z,
    /// The complete payload, including its `kind`. The event kind is the command Activity kind.
    pub(crate) activity: serde_json::Value,
}
