use std::sync::Mutex;

use storyos_application::{
    CacheUse, DestinationRequest, ModelProviderAdapter, ModelResponse, ModelStreamSink, ModelUsage,
    NoContractFaults, Observation, PreDispatchRefusal, PreparedRequest, ReferenceRetrieval,
    ReportedBinding, ResponseReference, WirePayloadProjection, complete_agent_run,
};
use storyos_core::{
    AGENT_PLAN_REGISTRATION, DecisionCandidate, DeploymentDestination, ModelAdapter, ModelOutput,
    NativeStreamItem, OutputPhase, ProseChangeCandidate, RetrievalBounds, StreamItemRole,
    StreamItemState,
};
use tokio_postgres::Client;

use crate::agent_run_dispatch::tests::RuntimeClock;
use crate::agent_run_dispatch::tests::{claim_run, queued_run, stores};
use crate::agent_run_retrieval::tests::{
    AfterExchange, adapter, broken_stream, destination, real_run, stored,
};

/// A stream that reports `reference` and ends with a complete Advisory.
fn completed_stream(reference: &str) -> String {
    let message = r#"{"id":"msg_01","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"{\"kind\":\"advisory\",\"text\":\"Keep it.\"}"}]}"#;
    let events = [
        format!(
            r#"{{"type":"response.created","response":{{"id":"{reference}","status":"in_progress","output":[]}}}}"#
        ),
        format!(r#"{{"type":"response.output_item.done","output_index":0,"item":{message}}}"#),
        format!(
            r#"{{"type":"response.completed","response":{{"id":"{reference}","status":"completed","output":[{message}],"usage":{{"input_tokens":40,"output_tokens":12}}}}}}"#
        ),
    ];
    sse(&events)
}

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
                 ON decision.run_id = run.run_id AND decision.attempt_role = 'decision',
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
        completed_stream("resp_c46s"),
        stored("resp_c46", "completed"),
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
                  WHERE decision.run_id = $1::text::uuid AND decision.attempt_role = 'decision'",
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
                Some("Keep it.".to_owned()),
                Some("decision,retrieval,successor,late_retrieval".to_owned()),
            ],
            true,
            vec![
                "POST /api/plan/v3/responses HTTP/1.1".to_owned(),
                "GET /api/plan/v3/responses/resp_c46 HTTP/1.1".to_owned(),
                "POST /api/plan/v3/responses HTTP/1.1".to_owned(),
                "GET /api/plan/v3/responses/resp_c46 HTTP/1.1".to_owned(),
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

/// An Agent Plan route whose Create outcome is unknown and whose stored response is a complete
/// prose change of each declared target. It records the declared targets of the Retrieve.
struct StoredProseChange {
    declared: Mutex<Vec<String>>,
}

impl ModelProviderAdapter for StoredProseChange {
    const ADAPTERS: &'static [ModelAdapter] = &[ModelAdapter::VolcengineAgentPlanResponses];
    type Prepared = Option<Vec<ProseChangeCandidate>>;

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<Self::Prepared>, PreDispatchRefusal> {
        let prepared = match request {
            DestinationRequest::Create(_) => None,
            DestinationRequest::Retrieve(retrieve) => {
                *self.declared.lock().unwrap() = retrieve
                    .declared_targets
                    .iter()
                    .map(|target| target.block_id.clone())
                    .collect();
                Some(
                    retrieve
                        .declared_targets
                        .iter()
                        .map(|target| ProseChangeCandidate {
                            chapter_id: target.chapter_id.clone(),
                            base_authoritative_revision_id: target.base_revision_id.clone(),
                            manuscript_block_id: target.block_id.clone(),
                            candidate_text: "Rain hit the roof.".to_owned(),
                            explanation: "Shorter.".to_owned(),
                        })
                        .collect(),
                )
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
        let Some(changes) = prepared else {
            return Observation::OutcomeUnknown {
                response_reference: Some(ResponseReference {
                    reference_id: "resp_c49".to_owned(),
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
            };
        };
        Observation::Terminal(ModelResponse {
            items: vec![NativeStreamItem {
                item_id: "msg_01".to_owned(),
                role: StreamItemRole::Assistant,
                state: StreamItemState::Complete,
                text: Some("Shorter.".to_owned()),
                summary: None,
                call_id: None,
                arguments: None,
                refusal: None,
                hosted_report: None,
            }],
            output: Some(ModelOutput {
                phase: OutputPhase::FinalAnswer,
                candidate: DecisionCandidate::ProseChange {
                    text: "Shorter.".to_owned(),
                },
                prose_changes: Some(changes),
            }),
            usage: ModelUsage::Reported {
                input_tokens: 40,
                output_tokens: 12,
            },
            cache_use: CacheUse::Unknown,
            response_reference: Some("resp_c49".to_owned()),
        })
    }
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
                    decision.payload->'decision'->'opened_proposal'->>'kind',
                    decision.payload->'original_result_retrieval'->>'disposition',
                    decision.payload->'reservation'->>'released',
                    decision.continuation_binding_id IS NOT NULL,
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
            Some("prose_change".to_owned()),
            Some("present".to_owned()),
            Some("settled".to_owned()),
            Some("true".to_owned()),
            true,
            Some("decision,retrieval".to_owned()),
        )
    );
}

/// An Agent Plan route whose first Create outcome stays unknown. The successor Create returns a
/// prose change of `successor_block`, or of each declared target when it is `None`. The late
/// Retrieve of the predecessor returns a complete prose change of each declared target.
struct SuccessorProseChange {
    successor_block: Option<&'static str>,
}

enum SuccessorPlan {
    Unknown,
    Changes(Vec<ProseChangeCandidate>),
}

impl ModelProviderAdapter for SuccessorProseChange {
    const ADAPTERS: &'static [ModelAdapter] = &[ModelAdapter::VolcengineAgentPlanResponses];
    type Prepared = SuccessorPlan;

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<Self::Prepared>, PreDispatchRefusal> {
        let changes = |targets: &[storyos_application::DeclaredTarget], block: Option<&str>| {
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
        };
        let (digest, prepared) = match request {
            DestinationRequest::Create(create) if create.successor_of.is_some() => (
                "sha256:create",
                SuccessorPlan::Changes(changes(&create.declared_targets, self.successor_block)),
            ),
            DestinationRequest::Create(_) => ("sha256:create", SuccessorPlan::Unknown),
            DestinationRequest::Retrieve(retrieve)
                if retrieve.purpose == storyos_application::RetrievePurpose::LateResult =>
            {
                (
                    "sha256:retrieve",
                    SuccessorPlan::Changes(changes(&retrieve.declared_targets, None)),
                )
            }
            DestinationRequest::Retrieve(_) => ("sha256:retrieve", SuccessorPlan::Unknown),
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
        let SuccessorPlan::Changes(changes) = prepared else {
            return Observation::OutcomeUnknown {
                response_reference: Some(ResponseReference {
                    reference_id: "resp_1d".to_owned(),
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
            };
        };
        Observation::Terminal(ModelResponse {
            items: vec![NativeStreamItem {
                item_id: "msg_01".to_owned(),
                role: StreamItemRole::Assistant,
                state: StreamItemState::Complete,
                text: Some("Shorter.".to_owned()),
                summary: None,
                call_id: None,
                arguments: None,
                refusal: None,
                hosted_report: None,
            }],
            output: Some(ModelOutput {
                phase: OutputPhase::FinalAnswer,
                candidate: DecisionCandidate::ProseChange {
                    text: "Shorter.".to_owned(),
                },
                prose_changes: Some(changes),
            }),
            usage: ModelUsage::Reported {
                input_tokens: 40,
                output_tokens: 12,
            },
            cache_use: CacheUse::Unknown,
            response_reference: Some("resp_1d".to_owned()),
        })
    }
}

/// Completes one Run on `route`. Returns its status, the successor decision kind and Proposal
/// kind, the Proposals of the Run as matches of the successor decision, the late evidence fact,
/// and the Attempt roles.
async fn successor_proposal_facts(
    route: &SuccessorProseChange,
    prefix: &str,
) -> [Option<String>; 6] {
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
                    successor.payload->'decision'->'opened_proposal'->>'kind',
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
               JOIN storyos.model_attempts AS successor
                 ON successor.run_id = run.run_id AND successor.attempt_role = 'successor'
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    [0, 1, 2, 3, 4, 5].map(|index| row.get(index))
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_successor_prose_change_opens_one_proposal_and_a_late_result_opens_none() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let route = SuccessorProseChange {
        successor_block: None,
    };

    assert_eq!(
        successor_proposal_facts(&route, "1d1").await,
        [
            Some("completed".to_owned()),
            Some("prose_change".to_owned()),
            Some("present".to_owned()),
            Some("true".to_owned()),
            Some("true".to_owned()),
            Some("decision,retrieval,successor,late_retrieval".to_owned()),
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
    };

    assert_eq!(
        successor_proposal_facts(&route, "1d2").await,
        [
            Some("completed".to_owned()),
            None,
            None,
            None,
            Some("true".to_owned()),
            Some("decision,retrieval,successor,late_retrieval".to_owned()),
        ]
    );
}
