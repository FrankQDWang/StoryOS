use std::sync::Mutex;

use storyos_application::{
    CacheUse, DestinationRequest, ModelProviderAdapter, ModelResponse, ModelStreamSink, ModelUsage,
    NoContractFaults, Observation, PreDispatchRefusal, PreparedRequest, ReferenceRetrieval,
    ReportedBinding, ResponseReference, WirePayloadProjection, complete_agent_run,
};
use storyos_core::{
    AGENT_PLAN_REGISTRATION, DeploymentDestination, ModelAdapter, NativeStreamItem,
    ProseChangeCandidate, RetrievalBounds,
};
use tokio_postgres::Client;

use crate::agent_run_dispatch::tests::RuntimeClock;
use crate::agent_run_dispatch::tests::{claim_run, queued_run, stores};
use crate::agent_run_finish_rule::tests::{finish_item, propose_edit_item, text_item};
use crate::agent_run_retrieval::tests::{
    AfterExchange, adapter, broken_stream, destination, real_run, research_run, stored,
};
use crate::agent_run_tool_exposure::tests::{advisory_stream, finish_stream, function_call_stream};

/// A stream that reports `reference` and a function call item, and then ends.
fn tool_stream(reference: &str) -> String {
    sse(&[
        format!(
            r#"{{"type":"response.created","response":{{"id":"{reference}","status":"in_progress","output":[]}}}}"#
        ),
        r#"{"type":"response.output_item.done","output_index":0,"item":{"id":"fc_01","type":"function_call","status":"completed","call_id":"call_01","name":"edit","arguments":"{}"}}"#.to_owned(),
    ])
}

fn sse(events: &[String]) -> String {
    let mut reply = String::from(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
    );
    for event in events {
        reply.push_str(&format!("data: {event}\n\n"));
    }
    reply
}

/// The status and settlement of the Run, the successor marker facts, and the Attempt roles.
async fn successor_facts(admin: &Client, run_id: &str) -> [Option<String>; 8] {
    let row = admin
        .query_one(
            "SELECT run.status, run.settlement::text,
                    marker->>'disposition', marker->>'pause_reason',
                    (marker->>'allowance_consumed') || ',' || (marker->>'predecessor_fenced'),
                    (marker->>'late_evidence_applied') || ',' || (marker->>'predecessor_usage_kind'),
                    (SELECT successor.payload->'decision'->>'text'
                       FROM storyos.model_attempts AS successor
                      WHERE successor.run_id = run.run_id AND successor.attempt_role = 'successor'),
                    (SELECT string_agg(attempt.attempt_role, ',' ORDER BY attempt.model_attempt_id)
                       FROM storyos.model_attempts AS attempt
                      WHERE attempt.run_id = run.run_id)
               FROM storyos.agent_runs AS run
               JOIN storyos.model_attempts AS decision
                 ON decision.run_id = run.run_id AND decision.attempt_role = 'decision'
                AND decision.decision_position = 0,
                    LATERAL (SELECT decision.payload->'unknown_create_successor' AS marker) AS facts
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    [0, 1, 2, 3, 4, 5, 6, 7].map(|index| row.get(index))
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_unknown_real_create_sends_the_same_request_once_and_keeps_its_late_result_as_evidence()
{
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (base, served) = destination(vec![
        broken_stream("resp_c46"),
        stored("resp_c46", "in_progress"),
        finish_stream("resp_c46s"),
        stored("resp_c46", "completed"),
        advisory_stream("resp_c46c"),
    ])
    .await;
    let (scope, run_id) = real_run(&store, &admin, "c46", &base).await;
    let claim = claim_run(&admin, scope, run_id.clone()).await;
    complete_agent_run(&store, &adapter(), &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    let digests: (String, String) = {
        let row = admin
            .query_one(
                "SELECT decision.payload->'wire'->>'digest',
                        successor.payload->'projection'->>'digest'
                   FROM storyos.model_attempts AS decision
                   JOIN storyos.model_attempts AS successor
                     ON successor.run_id = decision.run_id AND successor.attempt_role = 'successor'
                  WHERE decision.run_id = $1::text::uuid AND decision.attempt_role = 'decision'
                    AND decision.decision_position = 0",
                &[&run_id],
            )
            .await
            .unwrap();
        (row.get(/*idx*/ 0), row.get(/*idx*/ 1))
    };

    assert_eq!(
        (
            successor_facts(&admin, &run_id).await,
            digests.0 == digests.1,
            served.await.unwrap(),
        ),
        (
            [
                Some("completed".to_owned()),
                None,
                Some("dispatched".to_owned()),
                None,
                Some("true,true".to_owned()),
                Some("true,reported".to_owned()),
                None,
                Some("decision,retrieval,successor,late_retrieval,decision".to_owned()),
            ],
            true,
            vec![
                "POST /api/plan/v3/responses HTTP/1.1".to_owned(),
                "GET /api/plan/v3/responses/resp_c46 HTTP/1.1".to_owned(),
                "POST /api/plan/v3/responses HTTP/1.1".to_owned(),
                "GET /api/plan/v3/responses/resp_c46 HTTP/1.1".to_owned(),
                "POST /api/plan/v3/responses HTTP/1.1".to_owned(),
            ],
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_successor_call_of_an_unexposed_name_is_a_refused_call_and_the_run_continues() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (base, served) = destination(vec![
        broken_stream("resp_f74"),
        stored("resp_f74", "in_progress"),
        function_call_stream("resp_f74s"),
        stored("resp_f74", "in_progress"),
        finish_stream("resp_f74c"),
        advisory_stream("resp_f74d"),
    ])
    .await;
    let (scope, run_id) = real_run(&store, &admin, "f74", &base).await;
    let claim = claim_run(&admin, scope, run_id.clone()).await;
    complete_agent_run(&store, &adapter(), &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    let successor_items: Option<String> = admin
        .query_one(
            "SELECT payload->'items'->0->>'name' FROM storyos.model_attempts
              WHERE run_id = $1::text::uuid AND attempt_role = 'successor'",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(0);

    assert_eq!(
        (
            successor_facts(&admin, &run_id).await,
            successor_items,
            served.await.unwrap(),
        ),
        (
            [
                Some("completed".to_owned()),
                None,
                Some("dispatched".to_owned()),
                None,
                Some("true,true".to_owned()),
                Some("false,unknown".to_owned()),
                None,
                Some("decision,retrieval,successor,late_retrieval,decision,decision".to_owned()),
            ],
            Some("web_search".to_owned()),
            vec![
                "POST /api/plan/v3/responses HTTP/1.1".to_owned(),
                "GET /api/plan/v3/responses/resp_f74 HTTP/1.1".to_owned(),
                "POST /api/plan/v3/responses HTTP/1.1".to_owned(),
                "GET /api/plan/v3/responses/resp_f74 HTTP/1.1".to_owned(),
                "POST /api/plan/v3/responses HTTP/1.1".to_owned(),
                "POST /api/plan/v3/responses HTTP/1.1".to_owned(),
            ],
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_committed_tool_item_or_a_changed_request_pauses_the_successor_before_egress() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (tool_base, tool_served) = destination(vec![
        tool_stream("resp_c47"),
        stored("resp_c47", "in_progress"),
    ])
    .await;
    let (tool_scope, tool_run) = real_run(&store, &admin, "c47", &tool_base).await;
    let claim = claim_run(&admin, tool_scope, tool_run.clone()).await;
    complete_agent_run(&store, &adapter(), &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    let (changed_base, changed_served) = destination(vec![
        broken_stream("resp_c48"),
        stored("resp_c48", "in_progress"),
    ])
    .await;
    let (changed_scope, changed_run) = real_run(&store, &admin, "c48", &changed_base).await;
    // The committed request differs from the request that a successor prepares.
    let changed = AfterExchange {
        admin: &admin,
        statement: format!(
            "UPDATE storyos.model_attempts
                SET payload = jsonb_set(payload, '{{wire,digest}}', '\"sha256:changed\"')
              WHERE run_id = '{changed_run}' AND attempt_role = 'decision'"
        ),
    };
    let claim = claim_run(&admin, changed_scope, changed_run.clone()).await;
    complete_agent_run(&store, &changed, &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    let paused = |reason: &str| {
        [
            Some("paused".to_owned()),
            Some(format!(
                r#"{{"kind": "unknown_create_successor", "reason": "{reason}"}}"#
            )),
            Some("paused".to_owned()),
            Some(reason.to_owned()),
        ]
    };
    let lines = |reference: &str| {
        vec![
            "POST /api/plan/v3/responses HTTP/1.1".to_owned(),
            format!("GET /api/plan/v3/responses/{reference} HTTP/1.1"),
        ]
    };
    let tool = successor_facts(&admin, &tool_run).await;
    let changed = successor_facts(&admin, &changed_run).await;

    assert_eq!(
        (
            tool[..4].to_vec(),
            tool[4].clone(),
            tool[7].clone(),
            tool_served.await.unwrap(),
            changed[..4].to_vec(),
            changed[4].clone(),
            changed[7].clone(),
            changed_served.await.unwrap(),
        ),
        (
            paused("unresolved_effect").to_vec(),
            Some("false,false".to_owned()),
            Some("decision,retrieval".to_owned()),
            lines("resp_c47"),
            paused("request_changed").to_vec(),
            Some("true,true".to_owned()),
            Some("decision,retrieval".to_owned()),
            lines("resp_c48"),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_exposure_change_before_the_successor_claim_pauses_the_successor_before_egress() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (base, served) = destination(vec![
        broken_stream("resp_e32"),
        stored("resp_e32", "in_progress"),
    ])
    .await;
    let (scope, run_id) = research_run(&store, &admin, "e32", &base).await;
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    // A Tool Contract Drift quarantines the research revision after the successor request was
    // prepared.
    let route = crate::agent_run_tool_exposure::tests::QuarantineBeforeClaim {
        admin: &admin,
        successor: true,
        quarantined: std::sync::atomic::AtomicBool::new(/*v*/ false),
        prepared: Mutex::new(Vec::new()),
    };
    complete_agent_run(&store, &route, &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    crate::agent_run_tool_call::tests::reactivate_research_revision(&admin).await;
    let facts = successor_facts(&admin, &run_id).await;
    let declared: Vec<bool> = route
        .prepared
        .into_inner()
        .unwrap()
        .iter()
        .map(|(snapshot, tools)| snapshot["tools"] != serde_json::json!([]) && tools.is_some())
        .collect();

    assert_eq!(
        (
            facts[..4].to_vec(),
            facts[4].clone(),
            facts[7].clone(),
            declared,
            served.await.unwrap(),
        ),
        (
            vec![
                Some("paused".to_owned()),
                Some(
                    r#"{"kind": "unknown_create_successor", "reason": "request_changed"}"#
                        .to_owned()
                ),
                Some("paused".to_owned()),
                Some("request_changed".to_owned()),
            ],
            Some("true,true".to_owned()),
            Some("decision,retrieval".to_owned()),
            vec![true, true],
            vec![
                "POST /api/plan/v3/responses HTTP/1.1".to_owned(),
                "GET /api/plan/v3/responses/resp_e32 HTTP/1.1".to_owned(),
            ],
        )
    );
}

/// An Agent Plan route whose first Create outcome is unknown and whose stored response is a
/// `propose_edit` call of each declared target beside a `finish` call. A later Create gets the
/// closing text. It records the declared targets of the Retrieve.
struct StoredProseChange {
    declared: Mutex<Vec<String>>,
}

impl ModelProviderAdapter for StoredProseChange {
    const ADAPTERS: &'static [ModelAdapter] = &[ModelAdapter::VolcengineAgentPlanResponses];
    type Prepared = Option<Vec<NativeStreamItem>>;

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<Self::Prepared>, PreDispatchRefusal> {
        let prepared = match request {
            DestinationRequest::Create(_) if self.declared.lock().unwrap().is_empty() => None,
            DestinationRequest::Create(create) => Some(match create.tool_choice {
                storyos_core::ToolChoice::None => vec![text_item("Shorter.")],
                _ => vec![finish_item("call_finish_2")],
            }),
            DestinationRequest::Retrieve(retrieve) => {
                *self.declared.lock().unwrap() = retrieve
                    .declared_targets
                    .iter()
                    .map(|target| target.block_id.clone())
                    .collect();
                let changes = prose_changes(&retrieve.declared_targets, /*block*/ None);
                Some(vec![
                    propose_edit_item("call_c49", &changes),
                    finish_item("call_finish"),
                ])
            }
            DestinationRequest::Abort(_) => return Err(PreDispatchRefusal::UnsupportedRequest),
        };
        Ok(PreparedRequest {
            projection: WirePayloadProjection {
                execution_profile: "storyos.volcengine-agent-plan.execution.v1".to_owned(),
                mapping_revision: AGENT_PLAN_REGISTRATION.mapping_revision.to_owned(),
                digest: format!("sha256:{}", prepared.is_some()),
                serialized_payload: None,
                cache_setting: None,
            },
            prepared,
        })
    }

    async fn exchange(
        &self,
        prepared: Self::Prepared,
        _sink: &mut impl ModelStreamSink,
    ) -> Observation {
        let Some(items) = prepared else {
            return Observation::OutcomeUnknown {
                response_reference: Some(stored_reference("resp_c49")),
            };
        };
        complete(items, "resp_c49")
    }
}

/// A reference of the route that a Retrieve can read.
fn stored_reference(reference: &str) -> ResponseReference {
    ResponseReference {
        reference_id: reference.to_owned(),
        retrieval: ReferenceRetrieval::Supported {
            bounds: RetrievalBounds::DeclaredReadOnly,
        },
        reported_binding: ReportedBinding {
            owner_user_id: None,
            conversation_id: None,
            destination_identity: None,
            mapping_revision: AGENT_PLAN_REGISTRATION.mapping_revision.to_owned(),
        },
    }
}

/// A complete response of `items` with the reference `reference`.
fn complete(items: Vec<NativeStreamItem>, reference: &str) -> Observation {
    Observation::Terminal(ModelResponse {
        state: storyos_core::StreamItemState::Complete,
        items,
        usage: ModelUsage::Reported {
            input_tokens: 40,
            output_tokens: 12,
        },
        cache_use: CacheUse::Unknown,
        response_reference: Some(reference.to_owned()),
    })
}

/// A change of each of `targets`, of `block` when it is set.
fn prose_changes(
    targets: &[storyos_application::DeclaredTarget],
    block: Option<&str>,
) -> Vec<ProseChangeCandidate> {
    targets
        .iter()
        .map(|target| ProseChangeCandidate {
            chapter_id: target.chapter_id.clone(),
            base_authoritative_revision_id: target.base_revision_id.clone(),
            manuscript_block_id: block.unwrap_or(&target.block_id).to_owned(),
            candidate_text: "Rain hit the roof.".to_owned(),
            explanation: "Shorter.".to_owned(),
        })
        .collect()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_retrieved_prose_change_settles_with_its_proposal_and_sends_no_successor() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, run_id) = queued_run(
        &store,
        "c49",
        DeploymentDestination::VolcengineAgentPlan {
            qualification_authorization: None,
        },
    )
    .await;
    let route = StoredProseChange {
        declared: Mutex::default(),
    };
    let claim = claim_run(&admin, scope, run_id.clone()).await;
    complete_agent_run(&store, &route, &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    let row = admin
        .query_one(
            "SELECT run.status, decision.payload->'decision'->>'kind',
                    (SELECT string_agg(
                              (proposal.source_decision_id = decision.decision_id)::text, ',')
                       FROM storyos.proposals AS proposal
                      WHERE proposal.source_run_id = run.run_id),
                    decision.payload->'original_result_retrieval'->>'disposition',
                    decision.payload->'reservation'->>'released',
                    decision.continuation_binding_id IS NOT NULL,
                    (SELECT string_agg(attempt.attempt_role, ',' ORDER BY attempt.model_attempt_id)
                       FROM storyos.model_attempts AS attempt
                      WHERE attempt.run_id = run.run_id)
               FROM storyos.agent_runs AS run
               JOIN storyos.model_attempts AS decision
                 ON decision.run_id = run.run_id AND decision.attempt_role = 'decision'
                AND decision.decision_position = 0
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();

    assert_eq!(
        (
            route.declared.lock().unwrap().is_empty(),
            row.get::<_, String>(0),
            row.get::<_, Option<String>>(1),
            row.get::<_, Option<String>>(2),
            row.get::<_, Option<String>>(3),
            row.get::<_, Option<String>>(4),
            row.get::<_, bool>(5),
            row.get::<_, Option<String>>(6),
        ),
        (
            false,
            "completed".to_owned(),
            Some("tool_request".to_owned()),
            Some("true".to_owned()),
            Some("settled".to_owned()),
            Some("true".to_owned()),
            true,
            Some("decision,retrieval,decision".to_owned()),
        )
    );
}

/// An Agent Plan route whose first Create outcome stays unknown. The successor Create returns a
/// `propose_edit` call of `successor_block`, or of each declared target when it is `None`, beside
/// a `finish` call. The late Retrieve of the predecessor returns a `propose_edit` call of each
/// declared target. A later Create gets a `finish` call, or the closing text in a closing
/// request.
struct SuccessorProseChange {
    successor_block: Option<&'static str>,
    created: std::sync::atomic::AtomicBool,
}

impl ModelProviderAdapter for SuccessorProseChange {
    const ADAPTERS: &'static [ModelAdapter] = &[ModelAdapter::VolcengineAgentPlanResponses];
    type Prepared = Option<Vec<NativeStreamItem>>;

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<Self::Prepared>, PreDispatchRefusal> {
        let proposed = |targets, block| {
            Some(vec![
                propose_edit_item("call_1d", &prose_changes(targets, block)),
                finish_item("call_finish"),
            ])
        };
        let (digest, prepared) = match request {
            DestinationRequest::Create(create) if create.successor_of.is_some() => (
                "sha256:create",
                proposed(&create.declared_targets, self.successor_block),
            ),
            DestinationRequest::Create(_)
                if !self.created.swap(true, std::sync::atomic::Ordering::SeqCst) =>
            {
                ("sha256:create", None)
            }
            DestinationRequest::Create(create) => (
                "sha256:create",
                Some(match create.tool_choice {
                    storyos_core::ToolChoice::None => vec![text_item("Shorter.")],
                    _ => vec![finish_item("call_finish_2")],
                }),
            ),
            DestinationRequest::Retrieve(retrieve)
                if retrieve.purpose == storyos_application::RetrievePurpose::LateResult =>
            {
                (
                    "sha256:retrieve",
                    proposed(&retrieve.declared_targets, /*block*/ None),
                )
            }
            DestinationRequest::Retrieve(_) => ("sha256:retrieve", None),
            DestinationRequest::Abort(_) => return Err(PreDispatchRefusal::UnsupportedRequest),
        };
        Ok(PreparedRequest {
            projection: WirePayloadProjection {
                execution_profile: "storyos.volcengine-agent-plan.execution.v1".to_owned(),
                mapping_revision: AGENT_PLAN_REGISTRATION.mapping_revision.to_owned(),
                digest: digest.to_owned(),
                serialized_payload: None,
                cache_setting: None,
            },
            prepared,
        })
    }

    async fn exchange(
        &self,
        prepared: Self::Prepared,
        _sink: &mut impl ModelStreamSink,
    ) -> Observation {
        let Some(items) = prepared else {
            return Observation::OutcomeUnknown {
                response_reference: Some(stored_reference("resp_1d")),
            };
        };
        complete(items, "resp_1d")
    }
}

/// Completes one Run on `route` and returns its facts. Each Proposal of the Run shows if its
/// source is the successor decision.
async fn successor_proposal_facts(
    route: &SuccessorProseChange,
    prefix: &str,
) -> [Option<String>; 5] {
    let (store, admin) = stores().await;
    let (scope, _, run_id) = queued_run(
        &store,
        prefix,
        DeploymentDestination::VolcengineAgentPlan {
            qualification_authorization: None,
        },
    )
    .await;
    let claim = claim_run(&admin, scope, run_id.clone()).await;
    complete_agent_run(&store, route, &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    let row = admin
        .query_one(
            "SELECT run.status, successor.payload->'decision'->>'kind',
                    (SELECT string_agg(
                              (proposal.source_decision_id = successor.decision_id)::text, ',')
                       FROM storyos.proposals AS proposal
                      WHERE proposal.source_run_id = run.run_id),
                    decision.payload->'unknown_create_successor'->>'late_evidence_applied',
                    (SELECT string_agg(attempt.attempt_role, ',' ORDER BY attempt.model_attempt_id)
                       FROM storyos.model_attempts AS attempt
                      WHERE attempt.run_id = run.run_id)
               FROM storyos.agent_runs AS run
               JOIN storyos.model_attempts AS decision
                 ON decision.run_id = run.run_id AND decision.attempt_role = 'decision'
                AND decision.decision_position = 0
               JOIN storyos.model_attempts AS successor
                 ON successor.run_id = run.run_id AND successor.attempt_role = 'successor'
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    [0, 1, 2, 3, 4].map(|index| row.get(index))
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_successor_prose_change_opens_one_proposal_and_a_late_result_opens_none() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let route = SuccessorProseChange {
        successor_block: None,
        created: Default::default(),
    };

    assert_eq!(
        successor_proposal_facts(&route, "1d1").await,
        [
            Some("completed".to_owned()),
            Some("tool_request".to_owned()),
            Some("true".to_owned()),
            Some("true".to_owned()),
            Some("decision,retrieval,successor,late_retrieval,decision".to_owned()),
        ]
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_successor_prose_change_of_an_undeclared_target_opens_no_proposal() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let route = SuccessorProseChange {
        successor_block: Some("018f0000-0000-7000-8000-0000000001d2"),
        created: Default::default(),
    };

    assert_eq!(
        successor_proposal_facts(&route, "1d2").await,
        // The refused call keeps the `finish` call from counting, so the Run continues.
        [
            Some("completed".to_owned()),
            Some("tool_request".to_owned()),
            None,
            Some("true".to_owned()),
            Some("decision,retrieval,successor,late_retrieval,decision,decision".to_owned()),
        ]
    );
}
