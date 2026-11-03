use std::sync::Mutex;

use storyos_application::{
    CacheUse, DestinationRequest, ModelProviderAdapter, ModelResponse, ModelStreamSink, ModelUsage,
    NoContractFaults, Observation, PreDispatchRefusal, PreparedRequest, ReferenceRetrieval,
    ReportedBinding, ResponseReference, WirePayloadProjection, complete_agent_run,
};
use storyos_core::{
    AGENT_PLAN_REGISTRATION, DecisionCandidate, DeploymentDestination, ModelAdapter, ModelOutput,
    OutputPhase, RetrievalBounds,
};
use tokio_postgres::Client;

use crate::agent_run_dispatch::tests::RuntimeClock;
use crate::agent_run_dispatch::tests::{claim_run, queued_run, stores};

const REFERENCE: &str = "resp_c45";

/// An Agent Plan route whose Create stream sees a committed cancellation, whose stored response
/// completed later, and which has no abort operation. It counts each refused Abort.
struct CancelledStream<'a> {
    admin: &'a Client,
    run_id: String,
    aborts: Mutex<usize>,
}

impl ModelProviderAdapter for CancelledStream<'_> {
    const ADAPTERS: &'static [ModelAdapter] = &[ModelAdapter::VolcengineAgentPlanResponses];
    type Prepared = bool;

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<bool>, PreDispatchRefusal> {
        let retrieve = match request {
            DestinationRequest::Create(_) => false,
            DestinationRequest::Retrieve(_) => true,
            DestinationRequest::Abort(_) => {
                *self.aborts.lock().unwrap() += 1;
                return Err(PreDispatchRefusal::UnsupportedRequest);
            }
        };
        Ok(PreparedRequest {
            projection: WirePayloadProjection {
                execution_profile: "storyos.volcengine-agent-plan.execution.v1".to_owned(),
                mapping_revision: AGENT_PLAN_REGISTRATION.mapping_revision.to_owned(),
                digest: format!("sha256:{retrieve}"),
                serialized_payload: None,
                cache_setting: None,
            },
            prepared: retrieve,
        })
    }

    async fn exchange(&self, retrieve: bool, sink: &mut impl ModelStreamSink) -> Observation {
        if retrieve {
            return Observation::Terminal(ModelResponse {
                items: Vec::new(),
                output: Some(ModelOutput {
                    phase: OutputPhase::FinalAnswer,
                    candidate: DecisionCandidate::Advisory {
                        text: "Too late.".to_owned(),
                    },
                    prose_changes: None,
                }),
                usage: ModelUsage::Reported {
                    input_tokens: 40,
                    output_tokens: 12,
                },
                cache_use: CacheUse::Unknown,
                response_reference: Some(REFERENCE.to_owned()),
            });
        }
        // The cancellation commits with the same fence, so the next stream commit stops.
        self.admin
            .execute(
                "UPDATE storyos.agent_runs SET status = 'cancelled', wakeup_pending = true
                  WHERE run_id = $1::text::uuid",
                &[&self.run_id],
            )
            .await
            .unwrap();
        sink.append(&[]).await;
        Observation::OutcomeUnknown {
            response_reference: Some(ResponseReference {
                reference_id: REFERENCE.to_owned(),
                retrieval: ReferenceRetrieval::Supported {
                    bounds: RetrievalBounds::DeclaredReadOnly,
                },
                reported_binding: ReportedBinding {
                    owner_user_id: None,
                    conversation_id: None,
                    destination_identity: None,
                    mapping_revision: AGENT_PLAN_REGISTRATION.mapping_revision.to_owned(),
                },
            }),
        }
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_refused_abort_keeps_the_fenced_retrieval_of_the_late_result() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, run_id) = queued_run(
        &store,
        "c45",
        DeploymentDestination::VolcengineAgentPlan {
            qualification_authorization: None,
        },
    )
    .await;
    let adapter = CancelledStream {
        admin: &admin,
        run_id: run_id.clone(),
        aborts: Mutex::default(),
    };
    let claim = claim_run(&admin, scope, run_id.clone()).await;
    complete_agent_run(&store, &adapter, &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    let row = admin
        .query_one(
            "SELECT run.status, run.wakeup_pending, decision.decision_id::text,
                    decision.payload->>'model_attempt_cancellation',
                    decision.payload->'original_result_retrieval'->>'disposition',
                    decision.payload->'usage'->>'kind',
                    (SELECT string_agg(attempt.attempt_role, ',' ORDER BY attempt.model_attempt_id)
                       FROM storyos.model_attempts AS attempt
                      WHERE attempt.run_id = run.run_id)
               FROM storyos.agent_runs AS run
               JOIN storyos.model_attempts AS decision
                 ON decision.run_id = run.run_id AND decision.attempt_role = 'decision'
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();

    assert_eq!(
        (
            *adapter.aborts.lock().unwrap(),
            row.get::<_, String>(0),
            row.get::<_, bool>(1),
            row.get::<_, Option<String>>(2),
            row.get::<_, Option<String>>(3),
            row.get::<_, Option<String>>(4),
            row.get::<_, Option<String>>(5),
            row.get::<_, Option<String>>(6),
        ),
        (
            1,
            "cancelled".to_owned(),
            false,
            None,
            Some(
                r#"{"provider_abort": "unsupported", "abort_attempt_id": null, "provider_confirmed": false}"#
                    .to_owned()
            ),
            Some("evidence_only".to_owned()),
            Some("reported".to_owned()),
            Some("decision,retrieval".to_owned()),
        )
    );
}
