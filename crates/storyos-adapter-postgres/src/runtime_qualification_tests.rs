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

use crate::agent_run_dispatch::tests::RuntimeClock;
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
                cache_setting: None,
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
    complete_agent_run(store, adapter, &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
}

/// Returns the Project to the binding of S4-01: the pending Agent Plan Registration, a pending
/// Decision, and that Registration as the head of its adapter. `run_id` pinned it.
async fn pin_pending_registration(admin: &Client, scope: &ProjectScope, run_id: &str) {
    let pending = &storyos_core::AGENT_PLAN_PENDING_REGISTRATION;
    let profile = serde_json::to_string(pending.capability_profile).unwrap();
    admin
        .batch_execute(&format!(
            "INSERT INTO storyos.model_capability_profiles (capability_profile_revision, profile)
             VALUES ('{profile_revision}', '{profile}'::jsonb) ON CONFLICT DO NOTHING;
             INSERT INTO storyos.model_registration_revisions
               (model_registration_revision, model_kind, api_surface, provider_model_id,
                capability_profile_revision)
             VALUES ('{revision}', 'volcengine_agent_plan_responses', '{surface}',
                     'doubao-seed-2.1-pro', '{profile_revision}') ON CONFLICT DO NOTHING;
             SET session_replication_role = replica;
             UPDATE storyos.project_external_use_binding_revisions
                SET model_registration_revision = '{revision}'
              WHERE project_id = '{project}' AND credential_source = 'operator_quota';
             UPDATE storyos.agent_runs SET model_registration_revision = '{revision}'
              WHERE run_id = '{run_id}';
             UPDATE storyos.external_contract_compatibility_decisions
                SET runtime_qualification = 'pending'
              WHERE project_id = '{project}' AND decision_kind = 'volcengine_agent_plan_compatible';
             SET session_replication_role = DEFAULT;
             UPDATE storyos.model_registration_heads SET model_registration_revision = '{revision}'
              WHERE model_kind = 'volcengine_agent_plan_responses';",
            profile_revision = pending.capability_profile.revision,
            revision = pending.revision,
            surface = pending.api_surface,
            project = scope.project_id.as_ref(),
        ))
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_qualified_registration_rebinds_the_project_and_dispatches_each_request_once() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, early_run) = queued_run(&store, "e9a", authorized()).await;
    pin_pending_registration(&admin, &scope, &early_run).await;
    let earlier = open_project_assistance(&store, &scope)
        .await
        .unwrap()
        .unwrap();
    // Another Project binds the qualified Registration, so it becomes the head again.
    let (other_scope, _, other_run) = queued_run(&store, "ed1", authorized()).await;
    let unknown = ScriptedAgentPlan::new(Observation::OutcomeUnknown {
        response_reference: None,
    });
    settle(&store, &admin, &scope, &early_run, &unknown).await;
    settle(&store, &admin, &other_scope, &other_run, &unknown).await;
    let unknown_run = admit_run(&store, &scope, &chapter_id, "e9b", authorized(), "Help.").await;
    let current = open_project_assistance(&store, &scope)
        .await
        .unwrap()
        .unwrap();
    let wrong_scope = ProjectScope::new(
        UserId::new("018f0000-0000-7001-8000-0000000e9f01"),
        scope.project_id.clone(),
    );
    let decision_facts = |decision: String| {
        let admin = &admin;
        async move {
            let row = admin
                .query_one(
                    "SELECT decision.runtime_qualification, decision.qualification_authorization,
                            binding.model_registration_revision::text
                       FROM storyos.external_contract_compatibility_decisions AS decision
                       JOIN storyos.project_external_use_binding_revisions AS binding
                         USING (owner_user_id, project_id, project_model_use_binding_revision)
                      WHERE decision.external_compatibility_decision = $1::text::uuid",
                    &[&decision],
                )
                .await
                .unwrap();
            (
                row.get::<_, String>(/*idx*/ 0),
                row.get::<_, Option<String>>(/*idx*/ 1),
                row.get::<_, String>(/*idx*/ 2),
            )
        }
    };

    assert_eq!(
        (
            current.clone(),
            open_project_assistance(&store, &wrong_scope).await.unwrap(),
            decision_facts(current.external_compatibility_decision.clone()).await,
            decision_facts(earlier.external_compatibility_decision.clone()).await,
        ),
        (
            ProjectAssistanceRecord {
                availability: AssistanceAvailability::Available,
                revision: 3,
                model_registration_revision: AGENT_PLAN_REGISTRATION.revision.to_owned(),
                destination: DestinationKind::VolcengineAgentPlan,
                runtime_qualification: RuntimeQualification::Qualified,
                ..current.clone()
            },
            None,
            (
                "qualified".to_owned(),
                None,
                AGENT_PLAN_REGISTRATION.revision.to_owned()
            ),
            (
                "pending".to_owned(),
                None,
                storyos_core::AGENT_PLAN_PENDING_REGISTRATION
                    .revision
                    .to_owned()
            ),
        )
    );
    assert_ne!(
        current.project_model_use_binding_revision,
        earlier.project_model_use_binding_revision
    );

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
        cache_use: storyos_application::CacheUse::Unknown,
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
        ConversationSelection::Existing {
            conversation_id: conversation_id.clone(),
        },
    )
    .await;
    settle(&store, &admin, &scope, &next, &advisory).await;
    // The prior binding of the Conversation came from the earlier Registration of the route.
    admin
        .execute(
            "UPDATE storyos.model_attempts
                SET payload = jsonb_set(payload, '{produced_binding,model_registration_revision}',
                                        to_jsonb($2::text))
              WHERE run_id = $1::text::uuid AND attempt_role = 'decision'",
            &[
                &next,
                &storyos_core::AGENT_PLAN_CONTINUATION_PENDING_REGISTRATION.revision,
            ],
        )
        .await
        .unwrap();
    let upgraded = admit_run_in(
        &store,
        &scope,
        &chapter_id,
        "ea7",
        authorized(),
        "Help once more.",
        ConversationSelection::Existing { conversation_id },
    )
    .await;
    settle(&store, &admin, &scope, &upgraded, &advisory).await;

    let route = RequestRoute {
        adapter: ModelAdapter::VolcengineAgentPlanResponses,
        provider_model_id: AGENT_PLAN_REGISTRATION.provider_model_id.to_owned(),
        mapping_revision: AGENT_PLAN_REGISTRATION.mapping_revision.to_owned(),
        endpoint: Some(AGENT_PLAN_ENDPOINT.to_owned()),
        credential_reference: Some(storyos_application::CredentialReference(
            REFERENCE.to_owned(),
        )),
        bounds: BudgetBounds::AgentPlanRequestV2.request_bounds(),
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
    let completed = |mapping: &str| {
        [
            text("completed"),
            None,
            text("1"),
            None,
            text("true"),
            text(mapping),
        ]
    };
    assert_eq!(
        (
            sent(unknown),
            sent(advisory),
            qualification_evidence(&admin, &early_run).await,
            qualification_evidence(&admin, &unknown_run).await,
            qualification_evidence(&admin, &first).await,
            qualification_evidence(&admin, &next).await,
            qualification_evidence(&admin, &upgraded).await,
        ),
        (
            // The unknown create sends its one bounded successor, which is also unknown.
            vec![(route.clone(), None); 4],
            vec![
                (route.clone(), None),
                (route.clone(), Some("resp_01".to_owned())),
                (route, None)
            ],
            [
                text("refused"),
                text(r#"{"kind": "execution_refused", "capability": "model_registration_drift"}"#),
                text("0"),
                None,
                None,
                None,
            ],
            [
                text("completed"),
                None,
                text("2"),
                None,
                text("true"),
                text("none"),
            ],
            completed("none"),
            completed("incremental"),
            completed("new_transport"),
        )
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
async fn a_cancelled_qualification_request_refuses_its_abort_once_and_records_no_remote_stop() {
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
    let fenced =
        complete_agent_run(&store, &adapter, &NoContractFaults, &RuntimeClock, &claim).await;
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
    let settled = complete_agent_run(
        &store,
        &adapter,
        &NoContractFaults,
        &RuntimeClock,
        &cancelled,
    )
    .await;
    let row = admin
        .query_one(
            "SELECT run.status, run.wakeup_pending,
                    (SELECT count(*) FROM storyos.model_attempts AS attempt
                      WHERE attempt.run_id = run.run_id),
                    decision.decision_id::text,
                    decision.payload->>'cancellation_duties_refused',
                    decision.payload->>'model_attempt_cancellation'
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
            row.get::<_, Option<String>>(5),
        ),
        (
            Some(storyos_application::CompleteAgentRun::AlreadySettled),
            1,
            "cancelled".to_owned(),
            false,
            1,
            None,
            None,
            Some(
                r#"{"provider_abort": "unsupported", "abort_attempt_id": null, "provider_confirmed": false}"#
                    .to_owned()
            ),
        )
    );
}

/// An Agent Plan adapter whose exchange cancels the AgentRun with the same fence. Then it
/// reports a complete response, as a response after a cancellation does.
struct LateResponseAgentPlan<'a> {
    admin: &'a Client,
    run_id: String,
}

impl ModelProviderAdapter for LateResponseAgentPlan<'_> {
    const ADAPTERS: &'static [ModelAdapter] = &[ModelAdapter::VolcengineAgentPlanResponses];
    type Prepared = ();

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<()>, PreDispatchRefusal> {
        let DestinationRequest::Create(_) = request else {
            return Err(PreDispatchRefusal::UnsupportedRequest);
        };
        let mut prepared = ScriptedAgentPlan::new(Observation::NotSubmitted)
            .prepare(request)
            .await?;
        prepared.projection.cache_setting =
            Some(storyos_application::CacheSetting::DestinationDefault);
        Ok(prepared)
    }

    async fn exchange(&self, _prepared: (), _sink: &mut impl ModelStreamSink) -> Observation {
        self.admin
            .execute(
                "UPDATE storyos.agent_runs SET status = 'cancelled'
                  WHERE run_id = $1::text::uuid AND status = 'claimed'",
                &[&self.run_id],
            )
            .await
            .unwrap();
        Observation::Terminal(ModelResponse {
            items: Vec::new(),
            output: Some(ModelOutput {
                phase: OutputPhase::FinalAnswer,
                candidate: DecisionCandidate::Advisory {
                    text: "Too late.".to_owned(),
                },
                prose_changes: None,
            }),
            usage: ModelUsage::Reported {
                input_tokens: 300,
                output_tokens: 20,
            },
            cache_use: storyos_application::CacheUse::Reported {
                cached_input_tokens: 128,
            },
            response_reference: Some("resp_late".to_owned()),
        })
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_response_after_cancellation_keeps_its_usage_and_cache_use_and_supplies_no_decision() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, run_id) = queued_run(&store, "ea8", authorized()).await;
    let adapter = LateResponseAgentPlan {
        admin: &admin,
        run_id: run_id.clone(),
    };
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    let _settled =
        complete_agent_run(&store, &adapter, &NoContractFaults, &RuntimeClock, &claim).await;
    let row = admin
        .query_one(
            "SELECT run.status, attempt.decision_id::text, attempt.payload->>'usage',
                    attempt.payload->>'cache', attempt.payload->>'observed_after_cancellation'
               FROM storyos.agent_runs AS run
               JOIN storyos.model_attempts AS attempt
                 ON attempt.run_id = run.run_id AND attempt.attempt_role = 'decision'
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();

    assert_eq!(
        (
            row.get::<_, String>(0),
            row.get::<_, Option<String>>(1),
            serde_json::from_str::<serde_json::Value>(&row.get::<_, String>(2)).unwrap(),
            serde_json::from_str::<serde_json::Value>(&row.get::<_, String>(3)).unwrap(),
            serde_json::from_str::<serde_json::Value>(&row.get::<_, String>(4)).unwrap(),
        ),
        (
            "cancelled".to_owned(),
            None,
            serde_json::json!({"kind": "reported", "input_tokens": 300, "output_tokens": 20}),
            serde_json::json!({
                "implicit": "destination_default",
                "explicit": "not_requested",
                "observed": {"kind": "reported", "cached_input_tokens": 128}
            }),
            serde_json::json!(true),
        )
    );
}

/// An Agent Plan adapter that replaces the stored reference of the prior binding during the
/// first preparation. This is a concurrent change before the dispatch claim.
struct ReferenceChangingAgentPlan<'a> {
    admin: &'a Client,
    prior_run_id: String,
    seen: Mutex<Vec<Option<String>>>,
}

impl ModelProviderAdapter for ReferenceChangingAgentPlan<'_> {
    const ADAPTERS: &'static [ModelAdapter] = &[ModelAdapter::VolcengineAgentPlanResponses];
    type Prepared = ();

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<()>, PreDispatchRefusal> {
        let DestinationRequest::Create(create) = request else {
            return Err(PreDispatchRefusal::UnsupportedRequest);
        };
        let first = {
            let mut seen = self.seen.lock().unwrap();
            seen.push(create.previous_response_reference.clone());
            seen.len() == 1
        };
        if first {
            self.admin
                .execute(
                    "UPDATE storyos.model_attempts
                        SET payload = jsonb_set(payload, '{response_reference}', '\"resp_02\"')
                      WHERE run_id = $1::text::uuid AND attempt_role = 'decision'",
                    &[&self.prior_run_id],
                )
                .await
                .unwrap();
        }
        ScriptedAgentPlan::new(Observation::NotSubmitted)
            .prepare(request)
            .await
    }

    async fn exchange(&self, _prepared: (), _sink: &mut impl ModelStreamSink) -> Observation {
        Observation::NotSubmitted
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_dispatch_claim_refuses_a_prior_reference_that_changed_after_preparation() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued_run(&store, "ea9", authorized()).await;
    let advisory = ScriptedAgentPlan::new(Observation::Terminal(ModelResponse {
        items: Vec::new(),
        output: Some(ModelOutput {
            phase: OutputPhase::FinalAnswer,
            candidate: DecisionCandidate::Advisory {
                text: "Keep it.".to_owned(),
            },
            prose_changes: None,
        }),
        usage: ModelUsage::Unknown,
        cache_use: storyos_application::CacheUse::Unknown,
        response_reference: Some("resp_01".to_owned()),
    }));
    settle(&store, &admin, &scope, &first, &advisory).await;
    let next = admit_run_in(
        &store,
        &scope,
        &chapter_id,
        "eab",
        authorized(),
        "Help again.",
        ConversationSelection::Existing {
            conversation_id: "018f0000-0000-7001-8000-00000006ea96".to_owned(),
        },
    )
    .await;
    let adapter = ReferenceChangingAgentPlan {
        admin: &admin,
        prior_run_id: first,
        seen: Mutex::default(),
    };
    let claim = claim_run(&admin, scope.clone(), next.clone()).await;
    complete_agent_run(&store, &adapter, &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    let attempts: Vec<Option<String>> = admin
        .query(
            "SELECT payload->'wire'->>'prior_continuation_binding_id' IS NOT NULL
                    AND payload->'continuation'->>'input_mapping' = 'incremental'
               FROM storyos.model_attempts WHERE run_id = $1::text::uuid",
            &[&next],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| Some(row.get::<_, bool>(/*idx*/ 0).to_string()))
        .collect();

    assert_eq!(
        (adapter.seen.into_inner().unwrap(), attempts),
        (
            vec![Some("resp_01".to_owned()), Some("resp_02".to_owned())],
            vec![Some("true".to_owned())],
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

/// Answers one streamed Responses request on a local port with one terminal event that holds
/// `body`, and returns the endpoint.
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
        let event = format!("data: {{\"type\":\"response.completed\",\"response\":{body}}}\n\n");
        let head =
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n";
        stream
            .write_all((head.to_owned() + &event).as_bytes())
            .await
            .unwrap();
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
    Option<storyos_application::AttemptCache>,
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
    complete_agent_run(store, &adapter, &NoContractFaults, &RuntimeClock, &claim)
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
        model.cache,
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
        "eb1",
        response(
            "resp_eb1",
            Some(serde_json::json!({"input_tokens": 12, "output_tokens": 3,
                                    "input_tokens_details": {"cached_tokens": 8}})),
        ),
    )
    .await;
    let unknown =
        real_attempt_evidence(&store, &admin, "eb2", response("resp_eb2", /*usage*/ None)).await;
    let facts = |(adapter, mapping, evidence, cache): (
        ModelAdapter,
        String,
        Vec<storyos_application::AgentRunEvidence>,
        Option<storyos_application::AttemptCache>,
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
        (adapter, mapping, known, cache)
    };
    let cache = |observed| {
        Some(storyos_application::AttemptCache {
            setting: storyos_application::CacheSetting::DestinationDefault,
            observed,
        })
    };

    assert_eq!(
        [facts(reported), facts(unknown)],
        [
            (
                ModelAdapter::VolcengineAgentPlanResponses,
                AGENT_PLAN_REGISTRATION.mapping_revision.to_owned(),
                vec!["resp_eb1".to_owned(), "provider_usage_reported".to_owned()],
                cache(storyos_application::CacheUse::Reported {
                    cached_input_tokens: 8,
                }),
            ),
            (
                ModelAdapter::VolcengineAgentPlanResponses,
                AGENT_PLAN_REGISTRATION.mapping_revision.to_owned(),
                vec!["resp_eb2".to_owned(), "provider_usage_unknown".to_owned()],
                cache(storyos_application::CacheUse::Unknown),
            ),
        ]
    );
}
