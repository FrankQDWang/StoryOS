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
const REFERENCE: &str = storyos_core::AGENT_PLAN_CREDENTIAL_REFERENCE;

fn authorized() -> DeploymentDestination {
    DeploymentDestination::VolcengineAgentPlan {
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

/// An Agent Plan adapter whose exchange cancels the AgentRun, as an author cancellation during
/// the request does. It counts each refused Abort.
struct CancellingAgentPlan<'a> {
    admin: &'a Client,
    run_id: String,
    aborts: Mutex<usize>,
}

impl ModelProviderAdapter for CancellingAgentPlan<'_> {
    const ADAPTERS: &'static [ModelAdapter] = &[ModelAdapter::VolcengineAgentPlanResponses];
    type Prepared = ();

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<()>, PreDispatchRefusal> {
        if let DestinationRequest::Abort(_) = request {
            *self.aborts.lock().unwrap() += 1;
            return Err(PreDispatchRefusal::UnsupportedRequest);
        }
        ScriptedAgentPlan::new(Observation::NotSubmitted)
            .prepare(request)
            .await
    }

    async fn exchange(&self, _prepared: (), _sink: &mut impl ModelStreamSink) -> Observation {
        self.admin
            .execute(
                "UPDATE storyos.agent_runs
                    SET status = 'cancelled', fence_token = fence_token + 1,
                        lease_expires_at = NULL, wakeup_pending = true
                  WHERE run_id = $1::text::uuid AND status = 'claimed'",
                &[&self.run_id],
            )
            .await
            .unwrap();
        Observation::OutcomeUnknown {
            response_reference: None,
        }
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_cancelled_qualification_request_refuses_its_abort_once_before_dispatch() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, run_id) = queued_run(&store, "e9e", authorized()).await;
    let adapter = CancellingAgentPlan {
        admin: &admin,
        run_id: run_id.clone(),
        aborts: Mutex::default(),
    };
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    let fenced = complete_agent_run(&store, &adapter, &NoContractFaults, &claim).await;
    let fence_token = admin
        .query_one(
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
        .get(/*idx*/ 0);
    let cancelled = storyos_application::ClaimedAgentRun {
        fence_token,
        ..claim
    };
    let settled = complete_agent_run(&store, &adapter, &NoContractFaults, &cancelled).await;
    let row = admin
        .query_one(
            "SELECT run.status, run.wakeup_pending,
                    (SELECT count(*) FROM storyos.model_attempts AS attempt
                      WHERE attempt.run_id = run.run_id),
                    decision.decision_id::text,
                    decision.payload->>'cancellation_duties_refused'
               FROM storyos.agent_runs AS run
               JOIN storyos.model_attempts AS decision
                 ON decision.run_id = run.run_id AND decision.attempt_role = 'decision'
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();

    assert!(matches!(
        fenced,
        Err(storyos_application::CompleteAgentRunError::StaleFence)
    ));
    assert_eq!(
        (
            settled.ok(),
            *adapter.aborts.lock().unwrap(),
            row.get::<_, String>(0),
            row.get::<_, bool>(1),
            row.get::<_, i64>(2),
            row.get::<_, Option<String>>(3),
            row.get::<_, Option<String>>(4),
        ),
        (
            Some(storyos_application::CompleteAgentRun::AlreadySettled),
            1,
            "cancelled".to_owned(),
            false,
            1,
            None,
            Some("destination_request_unsupported".to_owned()),
        )
    );
}

/// Returns a test value that is not a real credential.
struct TestCredential;

impl storyos_application::CredentialResolver for TestCredential {
    async fn resolve(
        &self,
        _reference: &storyos_application::CredentialReference,
    ) -> Option<storyos_application::ResolvedCredential> {
        Some(storyos_application::ResolvedCredential::new(
            "storyos-test-credential".to_owned(),
        ))
    }
}

/// Answers one Responses request on a local port with `body`, and returns the endpoint.
async fn scripted_endpoint(body: String) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/api/plan/v3", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut chunk = [0_u8; 4096];
        while !request
            .windows(/*size*/ 4)
            .any(|window| window == b"\r\n\r\n")
        {
            let read = stream.read(&mut chunk).await.unwrap();
            request.extend_from_slice(&chunk[..read]);
        }
        let head = format!(
            "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all((head + &body).as_bytes()).await.unwrap();
    });
    endpoint
}

/// The Provider evidence of the AgentRun that the real adapter settles through `endpoint`.
async fn real_attempt_evidence(
    store: &crate::PostgresProjectReader,
    admin: &Client,
    prefix: &str,
    body: serde_json::Value,
) -> (
    ModelAdapter,
    String,
    Vec<storyos_application::AgentRunEvidence>,
) {
    let (scope, _, run_id) = queued_run(store, prefix, authorized()).await;
    let endpoint = scripted_endpoint(body.to_string()).await;
    admin
        .execute(
            "UPDATE storyos.processing_destination_identity_evidence_revisions
                SET endpoint = $2 WHERE project_id = $1::text::uuid AND endpoint IS NOT NULL",
            &[&scope.project_id.as_ref(), &endpoint],
        )
        .await
        .unwrap();
    let adapter = storyos_adapter_volcengine_responses::AgentPlanResponses {
        resolver: TestCredential,
    };
    let claim = claim_run(admin, scope.clone(), run_id.clone()).await;
    complete_agent_run(store, &adapter, &NoContractFaults, &claim)
        .await
        .unwrap();
    let record = storyos_application::open_agent_run(store, &scope, &run_id)
        .await
        .unwrap()
        .unwrap();
    let model = record.model.unwrap();
    (
        record.adapter,
        model.admission.adapter_mapping,
        model.evidence,
    )
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_real_adapter_attempt_records_its_mapping_usage_report_and_response_reference() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let response = |id: &str, usage: Option<serde_json::Value>| {
        let mut body = serde_json::json!({"id": id, "status": "completed", "output": [
            {"id": "msg_01", "type": "message", "role": "assistant", "status": "completed",
             "content": [{"type": "output_text", "text": "{\"kind\":\"advisory\",\"text\":\"Keep it.\"}"}]}
        ]});
        if let Some(usage) = usage {
            body["usage"] = usage;
        }
        body
    };
    let reported = real_attempt_evidence(
        &store,
        &admin,
        "e9f",
        response(
            "resp_e9f",
            Some(serde_json::json!({"input_tokens": 12, "output_tokens": 3})),
        ),
    )
    .await;
    let unknown =
        real_attempt_evidence(&store, &admin, "e9g", response("resp_e9g", /*usage*/ None)).await;
    let facts = |(adapter, mapping, evidence): (
        ModelAdapter,
        String,
        Vec<storyos_application::AgentRunEvidence>,
    )| {
        let known: Vec<_> = evidence
            .into_iter()
            .filter_map(|item| match item {
                storyos_application::AgentRunEvidence::StoredReference { reference_id, .. } => {
                    Some(reference_id)
                }
                storyos_application::AgentRunEvidence::ProviderReport { report, .. } => {
                    Some(report)
                }
                _ => None,
            })
            .filter(|fact| !fact.starts_with("01"))
            .collect();
        (adapter, mapping, known)
    };

    assert_eq!(
        [facts(reported), facts(unknown)],
        [
            (
                ModelAdapter::VolcengineAgentPlanResponses,
                storyos_core::AGENT_PLAN_MAPPING_REVISION.to_owned(),
                vec!["resp_e9f".to_owned(), "provider_usage_reported".to_owned()],
            ),
            (
                ModelAdapter::VolcengineAgentPlanResponses,
                storyos_core::AGENT_PLAN_MAPPING_REVISION.to_owned(),
                vec!["resp_e9g".to_owned(), "provider_usage_unknown".to_owned()],
            ),
        ]
    );
}
