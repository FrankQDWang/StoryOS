//! The zero-authority rows of a project command (ADR 0076, ADR 0080).

use std::future::Future;

use storyos_application::{ProjectCommandEnvelope, ProjectCommandError};
use tokio_postgres::Client;

use super::{
    ActivitySequences, LockedProject, ProjectCommand, ZeroAuthorityRows, ZeroAuthorityWrite,
    ZeroEffect, ZeroOutcome, unavailable,
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
