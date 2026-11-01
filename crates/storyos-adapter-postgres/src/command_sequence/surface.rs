//! The declared surface of a project command: the facts and the steps that the sequence reads
//! from each command (ADR 0080).

use std::future::Future;
use std::pin::Pin;

use storyos_application::{ProjectCommandEnvelope, ProjectCommandError, ProjectCommandSettlement};
use storyos_core::TransitionOutcome;
use tokio_postgres::Client;

use super::{
    Admission, AppliedVariant, CommandError, CommandResponse, LockedProject, NoZeroAuthority,
    ProjectActionClass, ReceiptHeads, ReceiptRefs, ReplayEffect, SelectsRecords, SettlementProfile,
    SettlementShape, ZeroAuthority, ZeroReceipt,
};
use crate::command_replay::{CommandReplay, ReplayFault};

/// The declared facts of one command that the sequence reads (ADR 0080). `E` is the settlement
/// error of the command.
pub(crate) struct CommandSpec<E = ProjectCommandError> {
    pub(crate) kind: storyos_contracts::CommandKind,
    /// The closed set of applied variants. A command of one variant names only it.
    pub(crate) applied: &'static [AppliedVariant],
    pub(crate) activity_kind: &'static str,
    pub(crate) replay_effect: ReplayEffect,
    pub(crate) response: CommandResponse,
    /// The refusal of a serialization failure, a unique violation, or a deadlock of the first-use
    /// transaction, including its commit. `None` keeps such a failure a store fault.
    pub(crate) contention_refusal: Option<fn() -> E>,
    /// The Domain Receipt payload of an applied outcome that the default `applied_receipt` gives.
    pub(crate) receipt_payload: &'static str,
}

/// The parameters of the settlement type of one command (ADR 0080).
pub(crate) type Effect<C> = <<C as ProjectCommand>::Settlement as SettlementShape>::Effect;
pub(crate) type NoEffect<C> = <<C as ProjectCommand>::Settlement as SettlementShape>::NoEffect;
pub(crate) type Conflict<C> = <<C as ProjectCommand>::Settlement as SettlementShape>::Conflict;
pub(crate) type Refusal<C> = <<C as ProjectCommand>::Settlement as SettlementShape>::Refusal;
pub(crate) type ZeroEffect<C> = <<C as ProjectCommand>::Settlement as SettlementShape>::ZeroEffect;

/// A Core Transition Outcome whose applied value carries the locked facts that `apply` reuses.
pub(crate) type Classified<C> = TransitionOutcome<
    (<C as ProjectCommand>::Applied, <C as ProjectCommand>::Plan),
    NoEffect<C>,
    Conflict<C>,
    Refusal<C>,
>;

/// The Core Transition Outcome of one command and the facts that the sequence records with it.
pub(crate) struct Classification<C: ProjectCommand + ?Sized> {
    pub(crate) outcome: Classified<C>,
    pub(crate) admission: Admission,
    pub(crate) heads: ReceiptHeads,
    pub(crate) zero_receipt: ZeroReceipt<ZeroEffect<C>>,
}

impl<C: ProjectCommand + ?Sized> Classification<C> {
    /// An explicit project command whose Receipt has empty head arrays.
    pub(crate) fn project_command(outcome: Classified<C>) -> Self {
        Self {
            outcome,
            admission: Admission::Project(ProjectActionClass::ExplicitProjectCommand),
            heads: ReceiptHeads::default(),
            zero_receipt: ZeroReceipt::Reason,
        }
    }
}

/// The scope sequences that one command's settlement profile allocates.
pub(crate) type ProfileSequences<C> =
    <<C as ProjectCommand>::Profile as SettlementProfile>::Sequences;

/// The applied writes that one command returns for its settlement profile.
pub(crate) type ProfileWrite<C> =
    <<C as ProjectCommand>::Profile as SettlementProfile>::Write<Effect<C>>;

/// The applied value that one command's settlement profile returns after its authority records.
pub(crate) type ProfileApplied<C> =
    <<C as ProjectCommand>::Profile as SettlementProfile>::Applied<Effect<C>>;

/// The Domain Receipt variant, payload, and references of one applied outcome.
pub(crate) struct AppliedReceipt {
    pub(crate) variant: AppliedVariant,
    pub(crate) payload: String,
    pub(crate) refs: ReceiptRefs,
}

/// The future of a write that runs after the profile records.
pub(crate) type AuthorityWrite<'c> =
    Pin<Box<dyn Future<Output = Result<(), ProjectCommandError>> + Send + 'c>>;

/// A write of the effect rows that refer to the authority records of the profile. The sequence
/// runs it after the profile writes those records.
pub(crate) type AfterAuthority<'c, C> = Box<
    dyn for<'a> FnOnce(
            &'a C,
            &'a Client,
            &'a ProjectCommandEnvelope,
            &'a ProfileApplied<C>,
        ) -> AuthorityWrite<'a>
        + Send
        + 'c,
>;

/// The applied writes that one command returns.
pub(crate) struct CommandWrite<'c, C: ProjectCommand + ?Sized> {
    /// The writes that the settlement profile records.
    pub(crate) profile: ProfileWrite<C>,
    pub(crate) after_authority: Option<AfterAuthority<'c, C>>,
}

impl<'c, C: ProjectCommand + ?Sized> CommandWrite<'c, C> {
    /// The applied writes of a command that runs `write` after the profile records.
    pub(crate) fn after_authority(
        profile: ProfileWrite<C>,
        write: impl for<'a> FnOnce(
            &'a C,
            &'a Client,
            &'a ProjectCommandEnvelope,
            &'a ProfileApplied<C>,
        ) -> AuthorityWrite<'a>
        + Send
        + 'c,
    ) -> Self {
        Self {
            profile,
            after_authority: Some(Box::new(write)),
        }
    }
}

/// One project command: its declared profile, its settlement type, its locked facts, its own
/// effect rows, and its applied effect decoder (ADR 0080).
///
/// Implementations never run transaction control (ADR 0031) and never write the Admission,
/// Receipt, settlement link, authority, or idempotency rows. `settle_project_command` writes
/// those rows in one fixed order.
pub(crate) trait ProjectCommand: Sync {
    const SPEC: CommandSpec<Self::Error>;
    /// The settlement error. A command that refuses before its Admission declares
    /// `RefusableCommandError` with its refusal type. The other commands declare
    /// `ProjectCommandError`.
    type Error: CommandError;
    type Profile: SettlementProfile;
    type Plan: Send + SelectsRecords<Self::Profile>;
    type Applied: Send;
    /// The settlement type that the application crate declares for the command.
    type Settlement: SettlementShape;

    /// Locks the command facts and classifies them through Core. A refusal before Admission
    /// rolls back the transaction.
    fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> impl Future<Output = Result<Classification<Self>, Self::Error>> + Send;

    /// The error of an Admission insert that inserts no row. It runs in the transaction of the
    /// insert. The default is an invalid Command Challenge.
    fn diagnose_missing_admission(
        &self,
        _client: &Client,
        _envelope: &ProjectCommandEnvelope,
    ) -> impl Future<Output = Self::Error> + Send {
        async { ProjectCommandError::InvalidChallenge.into() }
    }

    /// The Domain Receipt of an applied outcome. The default gives the first applied variant, the
    /// constant payload of the `CommandSpec`, and no references.
    fn applied_receipt(&self, _applied: &Self::Applied, _plan: &Self::Plan) -> AppliedReceipt {
        AppliedReceipt {
            variant: Self::SPEC.applied[0],
            payload: Self::SPEC.receipt_payload.to_owned(),
            refs: ReceiptRefs::default(),
        }
    }

    /// Writes the effect rows of an applied outcome after its profile sequences are allocated.
    fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
        sequences: &ProfileSequences<Self>,
        plan: Self::Plan,
        applied: Self::Applied,
    ) -> impl Future<Output = Result<CommandWrite<'_, Self>, ProjectCommandError>> + Send;

    /// Checks the stored facts that bind the acknowledgement to this request. The sequence runs
    /// it before every other replay check, so a binding conflict comes before damaged evidence.
    fn check_replay_binding(&self, _replay: &CommandReplay) -> Result<(), ReplayFault> {
        Ok(())
    }

    /// Decodes the applied effect from the stored acknowledgement evidence.
    fn decode(&self, replay: &CommandReplay) -> Result<Effect<Self>, ReplayFault>;

    /// The zero-authority rows of the command. The default writes none.
    fn zero_authority(&self) -> Option<&(impl ZeroAuthority<Self> + Sync)> {
        None::<&NoZeroAuthority>
    }
}

pub(crate) type SettledCommand<C> = ProjectCommandSettlement<
    ProfileApplied<C>,
    NoEffect<C>,
    Conflict<C>,
    Refusal<C>,
    <<C as ProjectCommand>::Settlement as SettlementShape>::Response,
    ZeroEffect<C>,
>;
