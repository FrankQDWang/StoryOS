use std::sync::Mutex;

use storyos_application::{
    ConversationSelection, CreateRequest, DestinationRequest, ModelProviderAdapter, ModelResponse,
    ModelStreamSink, ModelUsage, NoContractFaults, Observation, PreDispatchRefusal,
    PreparedRequest, ProjectAssistanceRecord, ProjectScope, RequestRoute, UserId,
    WirePayloadProjection, complete_agent_run, open_project_assistance,
};
use storyos_core::{
    AGENT_PLAN_ENDPOINT, AGENT_PLAN_REGISTRATION, AssistanceAvailability, BudgetBounds,
    DecisionCandidate, DeploymentDestination, DestinationKind, ModelAdapter, ModelOutput,
    OutputPhase, RuntimeQualification,
};
use tokio_postgres::Client;

use crate::agent_run_dispatch::tests::{admit_run, admit_run_in, claim_run, queued_run, stores};

const AUTHORIZATION: &str =
    "https://github.com/FrankQDWang/StoryOS/issues/393#issuecomment-6098554693";
const REFERENCE: &str = "macos-keychain:storyos-volcengine-agent-plan/frankqdwang";

fn authorized() -> DeploymentDestination {
    DeploymentDestination::VolcengineAgentPlan {
        credential_reference: REFERENCE.to_owned(),
        qualification_authorization: Some(AUTHORIZATION.to_owned()),
    }
}

/// An Agent Plan adapter that records each Create request and reports one scripted observation.
struct ScriptedAgentPlan {
    observation: Observation,
    seen: Mutex<Vec<CreateRequest>>,
}

impl ScriptedAgentPlan {
    fn new(observation: Observation) -> Self {
        Self {
            observation,
            seen: Mutex::default(),
        }
    }
}

impl ModelProviderAdapter for ScriptedAgentPlan {
    const ADAPTERS: &'static [ModelAdapter] = &[ModelAdapter::VolcengineAgentPlanResponses];
    type Prepared = ();

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<()>, PreDispatchRefusal> {
        let DestinationRequest::Create(create) = request else {
            return Err(PreDispatchRefusal::UnsupportedRequest);
        };
        self.seen.lock().unwrap().push(create.clone());
        Ok(PreparedRequest {
            projection: WirePayloadProjection {
                execution_profile: "storyos.volcengine-agent-plan.execution.v1".to_owned(),
                mapping_revision: "storyos.volcengine-agent-plan.responses.create.v1".to_owned(),
                digest: "sha256:scripted".to_owned(),
                serialized_payload: None,
            },
            prepared: (),
        })
    }

    async fn exchange(&self, _prepared: (), _sink: &mut impl ModelStreamSink) -> Observation {
        self.observation.clone()
    }
}

/// The status, settlement, Attempt count, and the qualification, network, and input mapping
/// facts of the decision Attempt of one AgentRun.
async fn qualification_evidence(admin: &Client, run_id: &str) -> [Option<String>; 6] {
    let row = admin
        .query_one(
            "SELECT run.status, run.settlement::text,
                    (SELECT count(*)::text FROM storyos.model_attempts AS counted
                      WHERE counted.run_id = run.run_id),
                    attempt.payload->>'runtime_qualification',
                    attempt.payload->'execution_profile'->>'network_io',
                    attempt.payload->'continuation'->>'input_mapping'
               FROM storyos.agent_runs AS run
               LEFT JOIN storyos.model_attempts AS attempt
                 ON attempt.run_id = run.run_id AND attempt.attempt_role = 'decision'
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    [0, 1, 2, 3, 4, 5].map(|index| row.get(index))
}

async fn settle(
    store: &crate::PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    run_id: &str,
    adapter: &ScriptedAgentPlan,
) {
    let claim = claim_run(admin, scope.clone(), run_id.to_owned()).await;
    complete_agent_run(store, adapter, &NoContractFaults, &claim)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_authorized_qualification_rebinds_and_dispatches_each_request_once_with_its_bounds() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let pending = DeploymentDestination::VolcengineAgentPlan {
        credential_reference: REFERENCE.to_owned(),
        qualification_authorization: None,
    };
    let (scope, chapter_id, pending_run) = queued_run(&store, "e9a", pending).await;
    let unknown_run = admit_run(&store, &scope, &chapter_id, "e9b", authorized(), "Help.").await;
    let current = open_project_assistance(&store, &scope)
        .await
        .unwrap()
        .unwrap();
    let wrong_scope = ProjectScope::new(
        UserId::new("018f0000-0000-7001-8000-0000000e9f01"),
        scope.project_id.clone(),
    );

    assert_eq!(
        (
            current.clone(),
            open_project_assistance(&store, &wrong_scope).await.unwrap(),
        ),
        (
            ProjectAssistanceRecord {
                availability: AssistanceAvailability::Available,
                revision: 3,
                model_registration_revision: AGENT_PLAN_REGISTRATION.revision.to_owned(),
                destination: DestinationKind::VolcengineAgentPlan,
                runtime_qualification: RuntimeQualification::QualificationAuthorized,
                ..current.clone()
            },
            None,
        )
    );
    let authorization: Option<String> = admin
        .query_one(
            "SELECT qualification_authorization
               FROM storyos.external_contract_compatibility_decisions
              WHERE external_compatibility_decision = $1::text::uuid",
            &[&current.external_compatibility_decision],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    assert_eq!(authorization.as_deref(), Some(AUTHORIZATION));

    let unknown = ScriptedAgentPlan::new(Observation::OutcomeUnknown {
        response_reference: None,
    });
    settle(&store, &admin, &scope, &pending_run, &unknown).await;
    settle(&store, &admin, &scope, &unknown_run, &unknown).await;

    let advisory = ScriptedAgentPlan::new(Observation::Terminal(ModelResponse {
        items: Vec::new(),
        output: Some(ModelOutput {
            phase: OutputPhase::FinalAnswer,
            candidate: DecisionCandidate::Advisory {
                text: "Keep the second sentence.".to_owned(),
            },
            prose_changes: None,
        }),
        usage: ModelUsage::Reported {
            input_tokens: 412,
            output_tokens: 133,
        },
        response_reference: Some("resp_01".to_owned()),
    }));
    let first = admit_run(&store, &scope, &chapter_id, "e9c", authorized(), "Help.").await;
    settle(&store, &admin, &scope, &first, &advisory).await;
    let conversation_id = "018f0000-0000-7001-8000-00000006e9c6".to_owned();
    let next = admit_run_in(
        &store,
        &scope,
        &chapter_id,
        "e9d",
        authorized(),
        "Help again.",
        ConversationSelection::Existing { conversation_id },
    )
    .await;
    settle(&store, &admin, &scope, &next, &advisory).await;

    let route = RequestRoute {
        adapter: ModelAdapter::VolcengineAgentPlanResponses,
        provider_model_id: "doubao-seed-2.1-pro".to_owned(),
        endpoint: Some(AGENT_PLAN_ENDPOINT.to_owned()),
        credential_reference: Some(storyos_application::CredentialReference(
            REFERENCE.to_owned(),
        )),
        bounds: BudgetBounds::AgentPlanRequestV1.request_bounds(),
    };
    let sent = |adapter: ScriptedAgentPlan| {
        adapter
            .seen
            .into_inner()
            .unwrap()
            .into_iter()
            .map(|request| (request.route, request.previous_response_reference))
            .collect::<Vec<_>>()
    };
    let text = |value: &str| Some(value.to_owned());
    assert_eq!(
        (
            sent(unknown),
            sent(advisory),
            qualification_evidence(&admin, &pending_run).await,
            qualification_evidence(&admin, &first).await,
            qualification_evidence(&admin, &next).await,
        ),
        (
            vec![(route.clone(), None)],
            vec![(route.clone(), None), (route, None)],
            [
                text("refused"),
                text(r#"{"kind": "execution_refused", "capability": "model_use_binding_stale"}"#),
                text("0"),
                None,
                None,
                None,
            ],
            [
                text("completed"),
                None,
                text("1"),
                text("qualification_authorized"),
                text("true"),
                text("none"),
            ],
            [
                text("completed"),
                None,
                text("1"),
                text("qualification_authorized"),
                text("true"),
                text("full"),
            ],
        )
    );
    assert_eq!(
        qualification_evidence(&admin, &unknown_run).await,
        [
            text("waiting"),
            text(r#"{"kind": "outcome_unknown", "reason": "missing_reference"}"#),
            text("1"),
            text("qualification_authorized"),
            text("true"),
            text("none")
        ]
    );
}
