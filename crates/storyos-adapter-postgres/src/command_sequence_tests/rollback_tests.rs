use std::fmt::Debug;

use storyos_application::{ProjectCommandEnvelope, ProjectCommandError};
use storyos_core::ReceiptResult;
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    ActivitySequences, Classification, CommandSpec, LockedProject, ProfileApplied,
    ProfileSequences, ProfileWrite, ProjectCommand, ReceiptRefs, ZeroAuthorityRows,
    ZeroAuthorityWrite, ZeroOutcome, settle_project_command, unavailable,
};

use super::agent_run::{cancel_agent_run_call, pause_agent_run_call};
use super::draft::{close_editor_flow_draft_call, expand_refused_edit_draft_call};
use super::project_session::{
    archive_project_call, take_over_project_writer_call, update_project_assistance_call,
    update_project_call,
};
use super::proposal_decision::{
    reject_proposal_operations_call, reopen_rejected_operations_call,
    reopen_withdrawn_proposal_call, replan_proposal_call, withdraw_proposal_call,
};
use super::proposal_generation::{
    complete_ready_partial_proposal_call, continue_proposal_generation_call,
};
use super::structure::{
    create_chapter_call, create_volume_call, delete_chapter_call, delete_volume_call,
    set_current_chapter_call, update_chapter_call, update_volume_call,
};
use super::support::{CommandCall, SequenceError, settlement_rows, stores};

#[derive(Clone, Copy, Debug)]
enum FailurePoint {
    Classify,
    Apply,
    AfterAuthority,
}

/// The failure that `Failing` injects, identified by its step.
#[derive(Debug)]
struct Injected(&'static str);

impl std::fmt::Display for Injected {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "injected {} failure", self.0)
    }
}

impl std::error::Error for Injected {}

/// A project command that fails after one step has written its rows.
struct Failing<C> {
    command: C,
    at: FailurePoint,
}

impl<C: ProjectCommand> ProjectCommand for Failing<C> {
    const SPEC: CommandSpec = C::SPEC;
    type Error = C::Error;
    type Profile = C::Profile;
    type Response = C::Response;
    type ZeroEffect = C::ZeroEffect;
    type Applied = C::Applied;
    type Plan = C::Plan;
    type Effect = C::Effect;
    type NoEffect = C::NoEffect;
    type Conflict = C::Conflict;
    type Refusal = C::Refusal;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> Result<Classification<Self>, Self::Error> {
        let classified = self.command.classify(client, envelope, project).await?;
        match self.at {
            FailurePoint::Classify => Err(unavailable(Injected("classify")).into()),
            FailurePoint::Apply | FailurePoint::AfterAuthority => Ok(Classification {
                outcome: classified.outcome,
                admission: classified.admission,
                heads: classified.heads,
                zero_receipt: classified.zero_receipt,
            }),
        }
    }

    fn applied_receipt_payload(&self, applied: &Self::Applied, plan: &Self::Plan) -> String {
        self.command.applied_receipt_payload(applied, plan)
    }

    fn applied_receipt_refs(&self, applied: &Self::Applied, plan: &Self::Plan) -> ReceiptRefs {
        self.command.applied_receipt_refs(applied, plan)
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
        sequences: &ProfileSequences<Self>,
        plan: Self::Plan,
        applied: Self::Applied,
    ) -> Result<ProfileWrite<Self>, ProjectCommandError> {
        let write = self
            .command
            .apply(client, envelope, project, sequences, plan, applied)
            .await?;
        match self.at {
            FailurePoint::Apply => Err(unavailable(Injected("apply"))),
            FailurePoint::Classify | FailurePoint::AfterAuthority => Ok(write),
        }
    }

    fn apply_after_authority(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        applied: &ProfileApplied<Self>,
    ) -> impl std::future::Future<Output = Result<(), ProjectCommandError>> + Send {
        // The applied value need not be `Sync`, so only the inner future holds it.
        let written = self
            .command
            .apply_after_authority(client, envelope, applied);
        async move {
            written.await?;
            match self.at {
                FailurePoint::AfterAuthority => Err(unavailable(Injected("after authority"))),
                FailurePoint::Classify | FailurePoint::Apply => Ok(()),
            }
        }
    }

    fn check_replay_binding(&self, replay: &CommandReplay) -> Result<(), ReplayFault> {
        self.command.check_replay_binding(replay)
    }

    fn decode(&self, replay: &CommandReplay) -> Result<Self::Effect, ReplayFault> {
        self.command.decode(replay)
    }

    fn zero_authority_rows(&self, outcome: &ZeroOutcome<'_, Self>) -> ZeroAuthorityRows {
        self.command.zero_authority_rows(&inner_outcome(outcome))
    }

    async fn write_zero_authority_effect(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        outcome: &ZeroOutcome<'_, Self>,
    ) -> Result<Self::ZeroEffect, ProjectCommandError> {
        self.command
            .write_zero_authority_effect(client, envelope, &inner_outcome(outcome))
            .await?;
        Err(unavailable(Injected("zero-authority write")))
    }

    async fn write_zero_authority_activity(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
        activity: &ActivitySequences,
    ) -> Result<ZeroAuthorityWrite<Self::ZeroEffect>, ProjectCommandError> {
        self.command
            .write_zero_authority_activity(client, envelope, project, activity)
            .await?;
        Err(unavailable(Injected("zero-authority write")))
    }

    fn decode_zero_authority_effect(
        &self,
        replay: &CommandReplay,
    ) -> Result<Option<Self::ZeroEffect>, ReplayFault> {
        self.command.decode_zero_authority_effect(replay)
    }
}

/// The zero-authority outcome of the wrapped command.
fn inner_outcome<'a, C: ProjectCommand>(
    outcome: &ZeroOutcome<'a, Failing<C>>,
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
async fn failed_then_settled<C: ProjectCommand<Error: SequenceError> + Clone>(
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
                | ProjectCommandError::MissingProject
                | ProjectCommandError::WriterIneligible,
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

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_failing_step_rolls_back_every_row_and_keeps_the_challenge_unused() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let observed = vec![
        failed_then_settled(
            &store,
            &admin,
            &create_volume_call(&store, /*base*/ 0x5f00).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &update_volume_call(&store, /*base*/ 0x5f10).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &delete_volume_call(&store, /*base*/ 0x5f20).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &create_chapter_call(&store, /*base*/ 0x5f30).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &update_chapter_call(&store, /*base*/ 0x5f40).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &delete_chapter_call(&store, /*base*/ 0x5f50).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &update_project_call(&store, /*base*/ 0x5f60).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &archive_project_call(&store, /*base*/ 0x5f70).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &set_current_chapter_call(&store, /*base*/ 0x5f80).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &update_project_assistance_call(&store, /*base*/ 0x5f90).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &take_over_project_writer_call(&store, /*base*/ 0x5fa0).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &reopen_withdrawn_proposal_call(&store, &admin, /*base*/ 0x5fb0).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &reject_proposal_operations_call(&store, &admin, /*base*/ 0x7b30).await,
        )
        .await,
        failed_then_settled(&store, &admin, &{
            let mut stale = reject_proposal_operations_call(&store, &admin, /*base*/ 0x7b40).await;
            stale.input.proposal_revision_id = Uuid::now_v7().to_string();
            stale
        })
        .await,
        failed_then_settled(
            &store,
            &admin,
            &replan_proposal_call(&store, &admin, /*base*/ 0x7260).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &reopen_rejected_operations_call(&store, &admin, /*base*/ 0x7270).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &withdraw_proposal_call(&store, &admin, /*base*/ 0x7480).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &close_editor_flow_draft_call(&store, &admin, /*base*/ 0x7d30).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &complete_ready_partial_proposal_call(&store, &admin, /*base*/ 0x7530).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &continue_proposal_generation_call(&store, &admin, /*base*/ 0x7540).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &expand_refused_edit_draft_call(&store, &admin, /*base*/ 0x8e30).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &pause_agent_run_call(&store, &admin, /*base*/ 0xb230).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &cancel_agent_run_call(&store, &admin, /*base*/ 0xd530).await,
        )
        .await,
    ];
    let rolled_back = |result| {
        let after_classify = match result {
            ReceiptResult::AuthoritativeApplied => ["apply", "after authority"],
            ReceiptResult::NoEffect | ReceiptResult::Conflicted | ReceiptResult::Refused => {
                ["zero-authority write"; 2]
            }
        };
        let failures = std::iter::once("classify")
            .chain(after_classify)
            .map(|step| (Some(step), [0; 5]))
            .collect::<Vec<_>>();
        (failures, result)
    };
    let mut expected = vec![rolled_back(ReceiptResult::AuthoritativeApplied); 10];
    expected.push(rolled_back(ReceiptResult::NoEffect));
    expected.extend(vec![rolled_back(ReceiptResult::AuthoritativeApplied); 2]);
    expected.push(rolled_back(ReceiptResult::Refused));
    expected.extend(vec![rolled_back(ReceiptResult::AuthoritativeApplied); 9]);
    assert_eq!(observed, expected);
}
