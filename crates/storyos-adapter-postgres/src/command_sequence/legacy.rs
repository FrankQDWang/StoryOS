//! The project command surface before ADR 0080, which the commands leave in migration batches.
//!
//! Blanket implementations give each command of this surface the `ProjectCommand` surface, so
//! that the one sequence settles a command of either surface. The contract ticket of ADR 0080
//! deletes this module.

use std::future::Future;

use storyos_application::{ProjectCommandEnvelope, ProjectCommandError, ProjectCommandSettlement};
use storyos_contracts::CommandKind;
use storyos_core::ReasonCode;
use tokio_postgres::Client;

use super::{
    ActivitySequences, AppliedReceipt, AppliedVariant, Classification, CommandError, CommandSpec,
    CommandWrite, LockedProject, ProjectCommand, ReceiptRefs, ReplayEffect, ResponseRecord,
    SelectsRecords, SettlementProfile, ZeroAuthority, ZeroAuthorityRows, ZeroAuthorityWrite,
    ZeroOutcome, unavailable,
};
use crate::command_replay::{CommandReplay, ReplayFault};

/// The `CommandSpec` of a command on this surface, without the facts of ADR 0080.
pub(crate) struct LegacySpec {
    pub(crate) kind: CommandKind,
    /// The closed set of applied variants. A command of one variant names only it.
    pub(crate) applied: &'static [AppliedVariant],
    pub(crate) activity_kind: &'static str,
    pub(crate) replay_effect: ReplayEffect,
}

/// One project command on the surface before ADR 0080: its declared profiles, its locked facts,
/// its own effect rows, and its applied effect decoder.
///
/// Implementations never run transaction control (ADR 0031) and never write the Admission,
/// Receipt, settlement link, authority, or idempotency rows.
pub(crate) trait LegacyProjectCommand: Sync {
    const SPEC: LegacySpec;
    /// The settlement error. A command that refuses before its Admission declares
    /// `RefusableCommandError` with its refusal type. The other commands declare
    /// `ProjectCommandError`.
    type Error: CommandError;
    type Profile: SettlementProfile;
    type Response: ResponseRecord;
    /// The effect of a zero-authority outcome that writes effect rows. Other commands use `()`.
    type ZeroEffect: Send;
    type Applied: Send;
    type Plan: Send + SelectsRecords<Self::Profile>;
    type Effect: Send;
    type NoEffect: ReasonCode + Send + Sync;
    type Conflict: ReasonCode + Send + Sync;
    type Refusal: ReasonCode + Send + Sync;
    /// The refusal of a serialization failure, a unique violation, or a deadlock of the first-use
    /// transaction, including its commit. `None` keeps such a failure a store fault.
    const CONTENTION_REFUSAL: Option<fn() -> Self::Error> = None;

    /// Locks the command facts and classifies them through Core. A refusal before Admission
    /// rolls back the transaction.
    fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> impl Future<Output = Result<Classification<Self>, <Self as LegacyProjectCommand>::Error>> + Send
    where
        Self: ProjectCommand;

    /// The error of an Admission insert that inserts no row. It runs in the transaction of the
    /// insert. The default is an invalid Command Challenge.
    fn diagnose_missing_admission(
        &self,
        _client: &Client,
        _envelope: &ProjectCommandEnvelope,
    ) -> impl Future<Output = Self::Error> + Send {
        async { ProjectCommandError::InvalidChallenge.into() }
    }

    /// The applied variant of an applied outcome. A command of one variant keeps the default.
    fn applied_variant(&self, _applied: &Self::Applied, _plan: &Self::Plan) -> AppliedVariant {
        Self::SPEC.applied[0]
    }

    /// The Domain Receipt payload of an applied outcome.
    fn applied_receipt_payload(&self, _applied: &Self::Applied, _plan: &Self::Plan) -> String {
        "{}".to_owned()
    }

    /// The Domain Receipt references of an applied outcome.
    fn applied_receipt_refs(&self, _applied: &Self::Applied, _plan: &Self::Plan) -> ReceiptRefs {
        ReceiptRefs::default()
    }

    /// Writes the effect rows of an applied outcome after its profile sequences are allocated.
    fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
        sequences: &<Self::Profile as SettlementProfile>::Sequences,
        plan: Self::Plan,
        applied: Self::Applied,
    ) -> impl Future<
        Output = Result<
            <Self::Profile as SettlementProfile>::Write<Self::Effect>,
            ProjectCommandError,
        >,
    > + Send;

    /// Writes the applied effect rows that refer to the authority records of the profile.
    ///
    /// The sequence calls this after the profile writes those records.
    fn apply_after_authority(
        &self,
        _client: &Client,
        _envelope: &ProjectCommandEnvelope,
        _applied: &<Self::Profile as SettlementProfile>::Applied<Self::Effect>,
    ) -> impl Future<Output = Result<(), ProjectCommandError>> + Send {
        async { Ok(()) }
    }

    /// Checks the stored facts that bind the acknowledgement to this request. The sequence runs
    /// it before every other replay check, so a binding conflict comes before damaged evidence.
    fn check_replay_binding(&self, _replay: &CommandReplay) -> Result<(), ReplayFault> {
        Ok(())
    }

    /// Decodes the applied effect from the stored acknowledgement evidence.
    fn decode(&self, replay: &CommandReplay) -> Result<Self::Effect, ReplayFault>;

    /// The rows that this zero-authority outcome writes (ADR 0076).
    fn zero_authority_rows(&self, _outcome: &ZeroOutcome<'_, Self>) -> ZeroAuthorityRows
    where
        Self: ProjectCommand,
    {
        ZeroAuthorityRows::None
    }

    /// Writes the effect rows of a zero-authority outcome that declares `Effect`.
    fn write_zero_authority_effect(
        &self,
        _client: &Client,
        _envelope: &ProjectCommandEnvelope,
        _outcome: &ZeroOutcome<'_, Self>,
    ) -> impl Future<Output = Result<Self::ZeroEffect, ProjectCommandError>> + Send
    where
        Self: ProjectCommand,
    {
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
    ) -> impl Future<Output = Result<ZeroAuthorityWrite<Self::ZeroEffect>, ProjectCommandError>> + Send
    {
        async { Err(unavailable("the command writes no zero-authority Activity")) }
    }

    /// Decodes the effect of a zero-authority outcome from the stored effect rows.
    fn decode_zero_authority_effect(
        &self,
        _replay: &CommandReplay,
    ) -> Result<Option<Self::ZeroEffect>, ReplayFault> {
        Ok(None)
    }
}

/// The settlement type of one command on this surface, from its associated types.
type LegacySettlement<T> = ProjectCommandSettlement<
    <<T as LegacyProjectCommand>::Profile as SettlementProfile>::Applied<
        <T as LegacyProjectCommand>::Effect,
    >,
    <T as LegacyProjectCommand>::NoEffect,
    <T as LegacyProjectCommand>::Conflict,
    <T as LegacyProjectCommand>::Refusal,
    <<T as LegacyProjectCommand>::Response as ResponseRecord>::Response,
    <T as LegacyProjectCommand>::ZeroEffect,
>;

impl<T: LegacyProjectCommand> ProjectCommand for T {
    const SPEC: CommandSpec<<T as LegacyProjectCommand>::Error> = CommandSpec {
        kind: <T as LegacyProjectCommand>::SPEC.kind,
        applied: <T as LegacyProjectCommand>::SPEC.applied,
        activity_kind: <T as LegacyProjectCommand>::SPEC.activity_kind,
        replay_effect: <T as LegacyProjectCommand>::SPEC.replay_effect,
        response: <T::Response as ResponseRecord>::RESPONSE,
        contention_refusal: T::CONTENTION_REFUSAL,
        // `applied_receipt` reads the payload hook of this surface.
        receipt_payload: "{}",
    };
    type Error = <T as LegacyProjectCommand>::Error;
    type Profile = <T as LegacyProjectCommand>::Profile;
    type Plan = <T as LegacyProjectCommand>::Plan;
    type Applied = <T as LegacyProjectCommand>::Applied;
    type Settlement = LegacySettlement<T>;

    fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> impl Future<Output = Result<Classification<Self>, Self::Error>> + Send {
        LegacyProjectCommand::classify(self, client, envelope, project)
    }

    fn diagnose_missing_admission(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
    ) -> impl Future<Output = Self::Error> + Send {
        LegacyProjectCommand::diagnose_missing_admission(self, client, envelope)
    }

    fn applied_receipt(&self, applied: &Self::Applied, plan: &Self::Plan) -> AppliedReceipt {
        AppliedReceipt {
            variant: self.applied_variant(applied, plan),
            payload: self.applied_receipt_payload(applied, plan),
            refs: self.applied_receipt_refs(applied, plan),
        }
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
        sequences: &<Self::Profile as SettlementProfile>::Sequences,
        plan: Self::Plan,
        applied: Self::Applied,
    ) -> Result<CommandWrite<'_, Self>, ProjectCommandError> {
        let profile =
            LegacyProjectCommand::apply(self, client, envelope, project, sequences, plan, applied)
                .await?;
        Ok(CommandWrite::after_authority(
            profile,
            |command: &Self, client, envelope, applied| {
                Box::pin(command.apply_after_authority(client, envelope, applied))
            },
        ))
    }

    fn check_replay_binding(&self, replay: &CommandReplay) -> Result<(), ReplayFault> {
        LegacyProjectCommand::check_replay_binding(self, replay)
    }

    fn decode(
        &self,
        replay: &CommandReplay,
    ) -> Result<<T as LegacyProjectCommand>::Effect, ReplayFault> {
        LegacyProjectCommand::decode(self, replay)
    }

    fn zero_authority(&self) -> Option<&(impl ZeroAuthority<Self> + Sync)> {
        Some(self)
    }
}

impl<T: LegacyProjectCommand> ZeroAuthority<T> for T {
    fn zero_authority_rows(&self, outcome: &ZeroOutcome<'_, T>) -> ZeroAuthorityRows {
        LegacyProjectCommand::zero_authority_rows(self, outcome)
    }

    fn write_zero_authority_effect(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        outcome: &ZeroOutcome<'_, T>,
    ) -> impl Future<Output = Result<T::ZeroEffect, ProjectCommandError>> + Send {
        LegacyProjectCommand::write_zero_authority_effect(self, client, envelope, outcome)
    }

    fn write_zero_authority_activity(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
        activity: &ActivitySequences,
    ) -> impl Future<Output = Result<ZeroAuthorityWrite<T::ZeroEffect>, ProjectCommandError>> + Send
    {
        LegacyProjectCommand::write_zero_authority_activity(
            self, client, envelope, project, activity,
        )
    }

    fn decode_zero_authority_effect(
        &self,
        replay: &CommandReplay,
    ) -> Result<Option<T::ZeroEffect>, ReplayFault> {
        LegacyProjectCommand::decode_zero_authority_effect(self, replay)
    }
}
