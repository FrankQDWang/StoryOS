//! The shared test support from `agent_run_steering_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::command_sequence_agent_run::CANCEL_AGENT_RUN;
use crate::shared_tests::command_sequence_steer_agent_run::STEER_AGENT_RUN;
use crate::shared_tests::command_sequence_support::{CommandCall, issued};
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::model_gateway::{RuntimeClock, claim_run, queued_run_offering};
use serde_json::Value;
use std::sync::Mutex;
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    AgentRunControlRefusal, CancelAgentRunInput, ClaimedAgentRun, CompleteAgentRun,
    CompleteAgentRunError, DestinationRequest, ModelProviderAdapter, ModelStreamSink,
    NoContractFaults, Observation, PreDispatchRefusal, PreparedRequest, ProjectScope,
    RefusableCommandError, SteerAgentRunInput, complete_agent_run,
};
use storyos_core::{
    DeploymentDestination, SelectionReference, SteerAgentRunConflict, SteerAgentRunNoEffect,
    TransitionOutcome,
};
use tokio_postgres::Client;

/// When the author acts, by the count of the Create requests of the Run from 1: before the
/// request goes out, or while it is out.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Moment {
    BeforeRequest(usize),
    DuringRequest(usize),
}

/// The answer to one steering input.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum SteeringAnswer {
    Retained,
    /// The `conflicted` answer of a Run that ended.
    Conflicted,
    Refused(AgentRunControlRefusal),
}

pub(crate) enum AuthorAction {
    /// The steering inputs, in order.
    Steer(Vec<CommandCall<SteerAgentRunInput>>),
    Cancel(Box<CommandCall<CancelAgentRunInput>>),
    /// An Author Edit of a pending candidate.
    Edit(Box<storyos_application::ApplyAuthorEditCommand>),
    Accept(Box<CommandCall<storyos_application::AcceptProposalInput>>),
    Reject(Box<CommandCall<storyos_application::RejectProposalOperationsInput>>),
    Withdraw(Box<CommandCall<storyos_application::WithdrawProposalInput>>),
    DeleteChapter(Box<CommandCall<storyos_application::DeleteChapterInput>>),
    UpdateChapter(Box<CommandCall<storyos_application::UpdateChapterInput>>),
    CreateChapter(Box<CommandCall<storyos_application::CreateChapterInput>>),
    SetCurrentChapter(Box<CommandCall<storyos_application::SetCurrentChapterInput>>),
    /// Each action in order.
    Sequence(Vec<AuthorAction>),
}

/// The count of the Create requests of a Run of a fake model, from 1.
#[derive(Default)]
pub(crate) struct Creates(Mutex<usize>);

impl Creates {
    /// The count of `request` when it is a Create request.
    pub(crate) fn count(&self, request: &DestinationRequest) -> Option<usize> {
        match request {
            DestinationRequest::Create(_) => {
                let mut creates = self.0.lock().unwrap();
                *creates += 1;
                Some(*creates)
            }
            DestinationRequest::Retrieve(_) | DestinationRequest::Abort(_) => None,
        }
    }
}

/// The fake model of the tool loop profile, and the author who acts one time at `moment`.
pub(crate) struct AuthorAt<'a> {
    pub store: &'a PostgresProjectReader,
    pub moment: Moment,
    pub action: AuthorAction,
    creates: Creates,
    pub(crate) answers: Mutex<Vec<SteeringAnswer>>,
}

impl<'a> AuthorAt<'a> {
    pub(crate) fn new(
        store: &'a PostgresProjectReader,
        moment: Moment,
        action: AuthorAction,
    ) -> Self {
        Self {
            store,
            moment,
            action,
            creates: Creates::default(),
            answers: Mutex::new(Vec::new()),
        }
    }

    async fn act(&self) {
        self.act_on(&self.action).await;
    }

    fn act_on<'b>(
        &'b self,
        action: &'b AuthorAction,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'b>> {
        Box::pin(async move {
            match action {
                AuthorAction::Steer(calls) => {
                    for call in calls {
                        let answer = match self
                            .store
                            .steer_agent_run(&call.envelope, &call.input)
                            .await
                        {
                            Ok(settlement) => match settlement.outcome {
                                TransitionOutcome::NoEffect(
                                    SteerAgentRunNoEffect::SteeringRetained,
                                ) => SteeringAnswer::Retained,
                                TransitionOutcome::Conflicted(
                                    SteerAgentRunConflict::TerminalRun,
                                ) => SteeringAnswer::Conflicted,
                                TransitionOutcome::Applied(applied) => match applied.effect {},
                                TransitionOutcome::Refused(reason) => match reason {},
                            },
                            Err(RefusableCommandError::RefusedBeforeAdmission(refusal)) => {
                                SteeringAnswer::Refused(refusal)
                            }
                            Err(RefusableCommandError::Command(error)) => panic!("{error:?}"),
                        };
                        self.answers.lock().unwrap().push(answer);
                    }
                }
                AuthorAction::Cancel(call) => {
                    self.store
                        .cancel_agent_run(&call.envelope, &call.input)
                        .await
                        .unwrap();
                }
                AuthorAction::Edit(command) => {
                    self.store.apply_author_edit(command).await.unwrap();
                }
                AuthorAction::Accept(call) => {
                    self.store
                        .accept_proposal(&call.envelope, &call.input)
                        .await
                        .unwrap();
                }
                AuthorAction::Reject(call) => {
                    self.store
                        .reject_proposal_operations(&call.envelope, &call.input)
                        .await
                        .unwrap();
                }
                AuthorAction::Withdraw(call) => {
                    self.store
                        .withdraw_proposal(&call.envelope, &call.input)
                        .await
                        .unwrap();
                }
                AuthorAction::DeleteChapter(call) => {
                    crate::shared_tests::command_sequence_structure::delete_chapter(
                        self.store, call,
                    )
                    .await
                    .unwrap();
                }
                AuthorAction::UpdateChapter(call) => {
                    crate::shared_tests::command_sequence_structure::update_chapter(
                        self.store, call,
                    )
                    .await
                    .unwrap();
                }
                AuthorAction::CreateChapter(call) => {
                    crate::shared_tests::command_sequence_structure::create_chapter(
                        self.store, call,
                    )
                    .await
                    .unwrap();
                }
                AuthorAction::SetCurrentChapter(call) => {
                    self.store
                        .set_current_chapter(&call.envelope, &call.input)
                        .await
                        .unwrap();
                }
                AuthorAction::Sequence(actions) => {
                    for action in actions {
                        self.act_on(action).await;
                    }
                }
            }
        })
    }
}

impl ModelProviderAdapter for AuthorAt<'_> {
    const ADAPTERS: &'static [storyos_core::ModelAdapter] = FakeDestination::ADAPTERS;
    type Prepared = (
        Option<usize>,
        <FakeDestination as ModelProviderAdapter>::Prepared,
    );

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<Self::Prepared>, PreDispatchRefusal> {
        let create = self.creates.count(request);
        if let (Some(count), Moment::BeforeRequest(at)) = (create, self.moment)
            && count == at
        {
            self.act().await;
        }
        let prepared = FakeDestination.prepare(request).await?;
        Ok(PreparedRequest {
            prepared: (create, prepared.prepared),
            projection: prepared.projection,
        })
    }

    async fn exchange(
        &self,
        (create, prepared): Self::Prepared,
        sink: &mut impl ModelStreamSink,
    ) -> Observation {
        if let (Some(count), Moment::DuringRequest(at)) = (create, self.moment)
            && count == at
        {
            self.act().await;
        }
        FakeDestination.exchange(prepared, sink).await
    }
}

/// One queued Run of the tool loop profile with `message` in a new Project of `prefix`.
pub(crate) async fn queued(
    store: &PostgresProjectReader,
    prefix: &str,
) -> (ProjectScope, String, String) {
    queued_run_offering(store, prefix, DeploymentDestination::HostFakeToolLoop, None).await
}

/// The steering input `message` with `references` for the Run `run_id` of a new Conversation,
/// with a fresh challenge `suffix`.
pub(crate) async fn steering(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: u16,
    run_id: &str,
    message: &str,
    references: Option<Vec<SelectionReference>>,
) -> CommandCall<SteerAgentRunInput> {
    let conversation_id = format!(
        "018f0000-0000-7001-8000-00000006{}",
        &run_id[run_id.len() - 4..]
    );
    let input = SteerAgentRunInput {
        run_id: run_id.to_owned(),
        conversation_id,
        author_message: message.to_owned(),
        selection_references: references,
    };
    issued(store, scope, suffix, &STEER_AGENT_RUN, input).await
}

/// The cancelAgentRun of the Run `run_id`, with a fresh challenge `suffix`.
pub(crate) async fn cancellation(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: u16,
    run_id: &str,
) -> CommandCall<CancelAgentRunInput> {
    let input = CancelAgentRunInput {
        run_id: run_id.to_owned(),
    };
    issued(store, scope, suffix, &CANCEL_AGENT_RUN, input).await
}

/// Claims and completes the Run with `adapter`, with a bounded wait: a RunStep that opens again
/// and again does not end the test.
pub(crate) async fn settle(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    run_id: &str,
    adapter: &impl ModelProviderAdapter,
) -> Result<CompleteAgentRun, CompleteAgentRunError> {
    let claim = claim_run(admin, scope.clone(), run_id.to_owned()).await;
    let gateway = complete_agent_run(store, adapter, &NoContractFaults, &RuntimeClock, &claim);
    tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 120), gateway)
        .await
        .expect("the Run settles")
}

/// For each decision Model Attempt of the Run in RunStep order: its position, its decision kind,
/// the author message and the `tool_choice` of its request, and the call of each Tool output
/// that its request sent.
pub(crate) async fn step_requests(admin: &Client, run_id: &str) -> Value {
    let rows = admin
        .query(
            "SELECT jsonb_build_array(decision_position, payload #>> '{decision,kind}',
                                      wire ->> 'author_message', wire ->> 'tool_choice',
                                      jsonb_path_query_array(wire, '$.tool_outputs[*].call_id'))::text
               FROM storyos.model_attempts,
                    LATERAL (SELECT (payload #>> '{wire,serialized_payload}')::jsonb) AS w(wire)
              WHERE run_id = $1::text::uuid AND attempt_role = 'decision'
              ORDER BY decision_position",
            &[&run_id],
        )
        .await
        .unwrap();
    rows.iter()
        .map(|row| serde_json::from_str::<Value>(row.get(/*idx*/ 0)).unwrap())
        .collect()
}

/// The references of the Operation Requirement of each RunStep of the Run, by position.
pub(crate) async fn step_references(admin: &Client, run_id: &str) -> Value {
    let rows = admin
        .query(
            "SELECT jsonb_build_array(decision_position, jsonb_path_query_array(payload,
                      '$.operation_requirement.selection_references[*].reference'))::text
               FROM storyos.operation_requirements
              WHERE run_id = $1::text::uuid AND requirement_role = 'primary'
              ORDER BY decision_position",
            &[&run_id],
        )
        .await
        .unwrap();
    rows.iter()
        .map(|row| serde_json::from_str::<Value>(row.get(/*idx*/ 0)).unwrap())
        .collect()
}

/// Each requirement position of the Run, and each decision Model Attempt as its position and
/// decision kind.
pub(crate) async fn run_steps(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
        "SELECT jsonb_build_object(
                      'status', run.status,
                      'requirements', (SELECT jsonb_agg(requirement.decision_position
                                        ORDER BY requirement.decision_position)
                                         FROM storyos.operation_requirements AS requirement
                                        WHERE requirement.run_id = run.run_id
                                          AND requirement.requirement_role = 'primary'),
                      'attempts', (SELECT jsonb_agg(jsonb_build_array(attempt.decision_position,
                                     attempt.payload #>> '{decision,kind}')
                                     ORDER BY attempt.decision_position)
                                     FROM storyos.model_attempts AS attempt
                                    WHERE attempt.run_id = run.run_id
                                      AND attempt.attempt_role = 'decision'))::text
               FROM storyos.agent_runs AS run WHERE run.run_id = $1::text::uuid",
        &[&run_id],
    )
    .await
}

/// The claim of a cancelled Run that has cancellation duties, as the Worker claims it.
pub(crate) async fn claim_duties(
    admin: &Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Option<ClaimedAgentRun> {
    admin
        .query_opt(
            "UPDATE storyos.agent_runs
                SET claim_generation = claim_generation + 1,
                    fence_token = claim_generation + 1,
                    lease_expires_at = clock_timestamp()
              WHERE run_id = $1::text::uuid AND status = 'cancelled' AND wakeup_pending
          RETURNING fence_token",
            &[&run_id],
        )
        .await
        .unwrap()
        .map(|row| ClaimedAgentRun {
            project_scope: scope.clone(),
            run_id: run_id.to_owned(),
            fence_token: row.get(/*idx*/ 0),
        })
}
