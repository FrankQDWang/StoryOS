//! The shared test support from `command_sequence_tests/rollback_tests.rs`.

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::tests::Effect;
use crate::command_sequence::{
    ActivitySequences, Admission, AdmittedCommand, AdmittedRefusal, AppliedReceipt, Classification,
    CommandSpec, CommandWrite, LockedProject, ProfileSequences, ProjectCommand, ZeroAuthority,
    ZeroAuthorityRows, ZeroAuthorityWrite, ZeroEffect, ZeroOutcome, settle_project_command,
    unavailable,
};
use crate::shared_tests::command_sequence_support::{CommandCall, SequenceError, settlement_rows};
use crate::statement_cache::Client;
use std::fmt::Debug;
use std::marker::PhantomData;
use storyos_application::{ProjectCommandEnvelope, ProjectCommandError};
use storyos_contracts::CommandKind;
use storyos_core::ReceiptResult;

#[derive(Clone, Copy, Debug)]
pub(crate) enum FailurePoint {
    Classify,
    Apply,
    AfterAuthority,
    /// A serialization failure of the store after the classify step.
    ContendedClassify,
    NoFailure,
}

/// The failure that `Failing` injects, identified by its step.
#[derive(Debug)]
pub(crate) struct Injected(&'static str);

impl std::fmt::Display for Injected {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "injected {} failure", self.0)
    }
}

impl std::error::Error for Injected {}

/// The command spec that `Failing` declares for the wrapped command `C`.
pub(crate) trait DeclaredSpec<C: ProjectCommand>: Send + Sync {
    const SPEC: CommandSpec<C::Error>;
}

/// The spec of the wrapped command.
pub(crate) struct WrappedSpec;

impl<C: ProjectCommand> DeclaredSpec<C> for WrappedSpec {
    const SPEC: CommandSpec<C::Error> = C::SPEC;
}

/// The spec of the wrapped command with the Create Project kind, which no Project Command Challenge
/// has, so the Admission insert writes no row.
pub(crate) struct UnchallengedKind;

impl<C: ProjectCommand> DeclaredSpec<C> for UnchallengedKind {
    const SPEC: CommandSpec<C::Error> = CommandSpec {
        kind: CommandKind::CreateProject,
        applied: &[],
        ..C::SPEC
    };
}

/// A project command that fails after one step has written its rows.
pub(crate) struct Failing<C, S = WrappedSpec> {
    pub(crate) command: C,
    pub(crate) at: FailurePoint,
    pub(crate) spec: PhantomData<S>,
}

impl<C: ProjectCommand, S: DeclaredSpec<C>> ProjectCommand for Failing<C, S> {
    const SPEC: CommandSpec<C::Error> = S::SPEC;
    type Error = C::Error;
    type Profile = C::Profile;
    type Plan = C::Plan;
    type Applied = C::Applied;
    type Settlement = C::Settlement;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> Result<Classification<Self>, Self::Error> {
        let classified = self.command.classify(client, envelope, project).await?;
        match self.at {
            FailurePoint::Classify => Err(unavailable(Injected("classify")).into()),
            FailurePoint::ContendedClassify => Err(serialization_failure(client).await.into()),
            FailurePoint::Apply | FailurePoint::AfterAuthority | FailurePoint::NoFailure => {
                Ok(Classification {
                    outcome: classified.outcome,
                    admission: classified.admission,
                    heads: classified.heads,
                    zero_receipt: classified.zero_receipt,
                })
            }
        }
    }

    async fn diagnose_missing_admission(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
    ) -> Self::Error {
        self.command
            .diagnose_missing_admission(client, envelope)
            .await
    }

    fn applied_receipt(&self, applied: &Self::Applied, plan: &Self::Plan) -> AppliedReceipt {
        self.command.applied_receipt(applied, plan)
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
        sequences: &ProfileSequences<Self>,
        plan: Self::Plan,
        applied: Self::Applied,
    ) -> Result<CommandWrite<'_, Self>, ProjectCommandError> {
        let CommandWrite {
            profile,
            after_authority,
        } = self
            .command
            .apply(client, envelope, project, sequences, plan, applied)
            .await?;
        match self.at {
            FailurePoint::Apply => Err(unavailable(Injected("apply"))),
            FailurePoint::Classify
            | FailurePoint::AfterAuthority
            | FailurePoint::ContendedClassify
            | FailurePoint::NoFailure => Ok(CommandWrite::after_authority(
                profile,
                move |failing: &Self, client, envelope, applied| {
                    // The applied value need not be `Sync`, so only the inner future holds it.
                    let written = after_authority
                        .map(|write| write(&failing.command, client, envelope, applied));
                    let at = failing.at;
                    Box::pin(async move {
                        if let Some(written) = written {
                            written.await?;
                        }
                        match at {
                            FailurePoint::AfterAuthority => {
                                Err(unavailable(Injected("after authority")))
                            }
                            FailurePoint::Classify
                            | FailurePoint::Apply
                            | FailurePoint::ContendedClassify
                            | FailurePoint::NoFailure => Ok(()),
                        }
                    })
                },
            )),
        }
    }

    fn check_replay_binding(&self, replay: &CommandReplay) -> Result<(), ReplayFault> {
        self.command.check_replay_binding(replay)
    }

    fn decode(&self, replay: &CommandReplay) -> Result<Effect<Self>, ReplayFault> {
        self.command.decode(replay)
    }

    fn zero_authority(&self) -> Option<&(impl ZeroAuthority<Self> + Sync)> {
        Some(self)
    }
}

impl<C: ProjectCommand, S: DeclaredSpec<C>> ZeroAuthority<Self> for Failing<C, S> {
    fn zero_authority_rows(&self, outcome: &ZeroOutcome<'_, Self>) -> ZeroAuthorityRows {
        self.command
            .zero_authority()
            .map_or(ZeroAuthorityRows::None, |zero| {
                zero.zero_authority_rows(&inner_outcome(outcome))
            })
    }

    async fn write_zero_authority_effect(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        outcome: &ZeroOutcome<'_, Self>,
    ) -> Result<ZeroEffect<Self>, ProjectCommandError> {
        if let Some(zero) = self.command.zero_authority() {
            zero.write_zero_authority_effect(client, envelope, &inner_outcome(outcome))
                .await?;
        }
        Err(unavailable(Injected("zero-authority write")))
    }

    async fn write_zero_authority_activity(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
        activity: &ActivitySequences,
    ) -> Result<ZeroAuthorityWrite<ZeroEffect<Self>>, ProjectCommandError> {
        if let Some(zero) = self.command.zero_authority() {
            zero.write_zero_authority_activity(client, envelope, project, activity)
                .await?;
        }
        Err(unavailable(Injected("zero-authority write")))
    }

    fn decode_zero_authority_effect(
        &self,
        replay: &CommandReplay,
    ) -> Result<Option<ZeroEffect<Self>>, ReplayFault> {
        self.command
            .zero_authority()
            .map_or(Ok(None), |zero| zero.decode_zero_authority_effect(replay))
    }
}

impl<C: AdmittedCommand, S: DeclaredSpec<C>> AdmittedCommand for Failing<C, S> {
    fn admission(&self, envelope: &ProjectCommandEnvelope) -> Admission {
        self.command.admission(envelope)
    }

    fn admitted_refusal(refusal: AdmittedRefusal) -> Self::Error {
        C::admitted_refusal(refusal)
    }

    fn before_commit(&self) -> Result<(), Self::Error> {
        self.command.before_commit()
    }
}

/// Raises a serialization failure in the command transaction, and returns it as a store fault.
async fn serialization_failure(client: &Client) -> ProjectCommandError {
    match client
        .batch_execute(
            "DO $$ BEGIN RAISE EXCEPTION 'injected contention'
             USING ERRCODE = 'serialization_failure'; END $$",
        )
        .await
    {
        Err(error) => unavailable(error),
        Ok(()) => panic!("the injected statement must fail"),
    }
}

/// The zero-authority outcome of the wrapped command.
pub(crate) fn inner_outcome<'a, C: ProjectCommand, S: DeclaredSpec<C>>(
    outcome: &ZeroOutcome<'a, Failing<C, S>>,
) -> ZeroOutcome<'a, C> {
    match outcome {
        ZeroOutcome::NoEffect(reason) => ZeroOutcome::NoEffect(*reason),
        ZeroOutcome::Conflicted(reason) => ZeroOutcome::Conflicted(*reason),
        ZeroOutcome::Refused(reason) => ZeroOutcome::Refused(*reason),
    }
}

/// Fails the call at each failure point, and then settles it for real.
///
/// Returns, for each failure, the step of the injected failure and the rows of its Receipt, then
/// the Receipt result kind of the real settlement.
pub(crate) async fn failed_then_settled<C: ProjectCommand<Error: SequenceError> + Clone>(
    store: &PostgresProjectReader,
    admin: &Client,
    call: &CommandCall<C>,
) -> (Vec<(Option<&'static str>, [i64; 5])>, ReceiptResult) {
    let mut failures = Vec::new();
    for at in [
        FailurePoint::Classify,
        FailurePoint::Apply,
        FailurePoint::AfterAuthority,
    ] {
        let failing = Failing {
            command: call.input.clone(),
            at,
            spec: PhantomData::<WrappedSpec>,
        };
        let injected = match settle_project_command(store, &call.envelope, &failing)
            .await
            .map_err(SequenceError::sequence)
        {
            Err(ProjectCommandError::Unavailable(source)) => source
                .downcast_ref::<Injected>()
                .map(|Injected(step)| *step),
            Err(
                ProjectCommandError::BindingConflict
                | ProjectCommandError::HistoricalAcknowledgementUnavailable
                | ProjectCommandError::InvalidChallenge
                | ProjectCommandError::MissingProject,
            )
            | Ok(_) => None,
        };
        failures.push((
            injected,
            settlement_rows(admin, &call.envelope.ids.receipt_id).await,
        ));
    }
    let Ok(settled) = settle_project_command(store, &call.envelope, &call.input).await else {
        panic!("the unused Command Challenge must still settle");
    };
    (failures, settled.outcome.receipt_result())
}
