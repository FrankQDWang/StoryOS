use std::sync::Mutex;

use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    AuthorCommandAdmissionIds, ClaimedAgentRun, CompleteAgentRun, CompleteAgentRunError,
    ContractFaultObserver, ContractFaultPoint, ConversationSelection, CreateAgentRunCommand,
    DestinationRequest, EditorClientBinding, IssueProjectCommandChallenge, ModelProviderAdapter,
    ModelResponse, ModelStreamSink, ModelUsage, NoContractFaults, Observation, PreDispatchRefusal,
    PreparedRequest, ProjectScope, UpdateProjectAssistanceCommand, complete_agent_run,
    issue_project_command_challenge, request_create_agent_run, update_project_assistance,
};
use storyos_core::{
    AssistanceAvailability, DecisionCandidate, ModelOutput, NativeStreamItem, OutputPhase,
    StreamItemRole, StreamItemState,
};
use tokio_postgres::{Client, NoTls};

use crate::PostgresProjectReader;
use crate::update_volume_tests::{apply_chapter, apply_volume, named_issue, seed_project};

const AVAILABLE_BYTES: &[u8] = br#"{"availability":"available"}"#;
const RUN_BYTES: &[u8] = br#"{"conversation":{"kind":"new"}}"#;
const VOLUME_BYTES: &[u8] = br#"{"expected_tree_revision":"1","title":"Volume A"}"#;
const CHAPTER_BYTES: &[u8] = br#"{"expected_tree_revision":"2","title":"Chapter A"}"#;

/// The durable dispatch evidence of one AgentRun.
#[derive(Debug, PartialEq)]
struct DispatchEvidence {
    status: String,
    settlement: Option<String>,
    model_attempts: i64,
    disclosure_events: i64,
    destination_manifests: i64,
    items: Option<String>,
    decision_id: Option<String>,
}

#[derive(Debug, PartialEq)]
struct ExchangeProbe {
    claimed_attempts: i64,
    run_locked: bool,
}

#[derive(Default)]
struct RecordedPoints(Mutex<Vec<ContractFaultPoint>>);

impl ContractFaultObserver for RecordedPoints {
    async fn reached(&self, point: ContractFaultPoint) {
        self.0.lock().unwrap().push(point);
    }
}

enum Probe {
    Refuse,
    InspectBeforeExchange,
    TakeOverBeforeExchange,
    CancelBeforeExchange,
    StreamTwoBatches,
}

/// Wraps the fake adapter and acts on the database at a deterministic point of the sequence.
struct ProbingDestination<'a> {
    admin: &'a Client,
    run_id: String,
    probe: Probe,
    seen: Mutex<Vec<ExchangeProbe>>,
    streamed: Mutex<Option<serde_json::Value>>,
}

impl ModelProviderAdapter for ProbingDestination<'_> {
    type Prepared = <FakeDestination as ModelProviderAdapter>::Prepared;

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<Self::Prepared>, PreDispatchRefusal> {
        if matches!(self.probe, Probe::Refuse) {
            return Err(PreDispatchRefusal::CredentialUnavailable);
        }
        FakeDestination.prepare(request).await
    }

    async fn exchange(
        &self,
        prepared: Self::Prepared,
        sink: &mut impl ModelStreamSink,
    ) -> Observation {
        match self.probe {
            Probe::Refuse => unreachable!("a refused request has no exchange"),
            Probe::StreamTwoBatches => {
                let item = |item_id: &str, state, text: &str| NativeStreamItem {
                    item_id: item_id.to_owned(),
                    role: StreamItemRole::Assistant,
                    state,
                    text: Some(text.to_owned()),
                    summary: None,
                    call_id: None,
                    arguments: None,
                    refusal: None,
                    hosted_report: None,
                };
                let first = [
                    item("1", StreamItemState::Provisional, "Guard"),
                    item("3", StreamItemState::Provisional, "Hold"),
                ];
                let second = [
                    item("1", StreamItemState::Complete, "Guard the voice"),
                    item("2", StreamItemState::Complete, "Keep it"),
                ];
                sink.append(&first).await;
                sink.append(&second).await;
                let streamed: String = self
                    .admin
                    .query_one(
                        "SELECT payload->>'items' FROM storyos.model_attempts
                          WHERE run_id = $1::text::uuid AND attempt_role = 'decision'",
                        &[&self.run_id],
                    )
                    .await
                    .unwrap()
                    .get(0);
                *self.streamed.lock().unwrap() = serde_json::from_str(&streamed).ok();
                return Observation::Terminal(ModelResponse {
                    items: second.to_vec(),
                    output: Some(ModelOutput {
                        phase: OutputPhase::Commentary,
                        candidate: DecisionCandidate::ProseChange {
                            text: "Guard the voice".to_owned(),
                        },
                        prose_changes: Some(Vec::new()),
                    }),
                    usage: ModelUsage::Unknown,
                    response_reference: None,
                });
            }
            Probe::InspectBeforeExchange => {
                let claimed_attempts = self
                    .admin
                    .query_one(
                        "SELECT count(*) FROM storyos.model_attempts
                          WHERE run_id = $1::text::uuid
                            AND dispatch_state = 'uncertain'
                            AND outbound_disclosure_event_id IS NOT NULL
                            AND wire_payload_projection_id IS NOT NULL",
                        &[&self.run_id],
                    )
                    .await
                    .unwrap()
                    .get(0);
                let run_locked = self
                    .admin
                    .query_opt(
                        "SELECT 1 FROM storyos.agent_runs
                          WHERE run_id = $1::text::uuid FOR UPDATE NOWAIT",
                        &[&self.run_id],
                    )
                    .await
                    .is_err();
                self.seen.lock().unwrap().push(ExchangeProbe {
                    claimed_attempts,
                    run_locked,
                });
            }
            Probe::CancelBeforeExchange => {
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
            }
            Probe::TakeOverBeforeExchange => {
                self.admin
                    .execute(
                        "UPDATE storyos.agent_runs
                            SET claim_generation = claim_generation + 1,
                                fence_token = claim_generation + 1
                          WHERE run_id = $1::text::uuid",
                        &[&self.run_id],
                    )
                    .await
                    .unwrap();
            }
        }
        FakeDestination.exchange(prepared, sink).await
    }
}

fn binding(issue: &IssueProjectCommandChallenge) -> EditorClientBinding {
    EditorClientBinding {
        binding_ref: issue.binding.client_session_binding_digest.clone(),
        session_generation: issue.binding.client_session_generation,
        client_contract_revision: issue.binding.client_contract_revision.clone(),
        security_policy_revision: issue.binding.security_policy_revision.clone(),
    }
}

fn ids(suffix: &str) -> AuthorCommandAdmissionIds {
    AuthorCommandAdmissionIds {
        command_id: format!("018f0000-0000-7001-8000-00000001{suffix}"),
        author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{suffix}"),
        receipt_id: format!("018f0000-0000-7001-8000-00000003{suffix}"),
    }
}

fn digest(kind: &str, bytes: &[u8]) -> String {
    format!(
        "sha256:storyos.command.{kind}.jcs.v1:{}",
        crate::author_edit::sha256_hex(bytes)
    )
}

/// Admits one fake-model AgentRun on a new chapter, then claims it with a known fence. The lease
/// is already expired, so a later Worker can drain a Run that a test leaves claimed.
async fn claimed_run(
    store: &PostgresProjectReader,
    admin: &Client,
    prefix: &str,
) -> ClaimedAgentRun {
    let scope: ProjectScope = seed_project(store, &format!("{prefix}0")).await;
    let assistance = named_issue(
        &scope,
        &format!("{prefix}1"),
        "PUT",
        "/api/v1/projects/{project_id}/assistance",
        "storyos.command.update-project-assistance.request.v1",
        "updateProjectAssistance",
        &digest("updateProjectAssistance", AVAILABLE_BYTES),
    );
    issue_project_command_challenge(store, &assistance)
        .await
        .unwrap();
    update_project_assistance(
        store,
        &UpdateProjectAssistanceCommand {
            project_scope: scope.clone(),
            client_binding: binding(&assistance),
            challenge_binding: assistance.binding.clone(),
            nonce_digest: assistance.nonce_digest.clone(),
            canonical_command_bytes: AVAILABLE_BYTES.to_vec(),
            correlation_id: format!("018f0000-0000-7001-8000-00000000{prefix}2"),
            availability: AssistanceAvailability::Available,
            expected_revision: 0,
            ids: ids(&format!("{prefix}2")),
        },
    )
    .await
    .unwrap();
    let volume_id = apply_volume(
        store,
        &scope,
        &format!("{prefix}3"),
        "Volume A",
        VOLUME_BYTES,
        &digest("createVolume", VOLUME_BYTES),
        /*expected_tree_revision*/ 1,
    )
    .await;
    let chapter_id = apply_chapter(
        store,
        &scope,
        &format!("{prefix}4"),
        &volume_id,
        "Chapter A",
        CHAPTER_BYTES,
        /*expected_tree_revision*/ 2,
    )
    .await;
    let run = named_issue(
        &scope,
        &format!("{prefix}5"),
        "POST",
        "/api/v1/projects/{project_id}/agent-runs",
        "storyos.command.create-agent-run.request.v2",
        "createAgentRun",
        &digest("createAgentRun", RUN_BYTES),
    );
    issue_project_command_challenge(store, &run).await.unwrap();
    let admitted = request_create_agent_run(
        store,
        &CreateAgentRunCommand {
            passage_targets: None,
            candidate_target: None,
            project_scope: scope.clone(),
            client_binding: binding(&run),
            challenge_binding: run.binding.clone(),
            nonce_digest: run.nonce_digest.clone(),
            canonical_command_bytes: RUN_BYTES.to_vec(),
            correlation_id: format!("018f0000-0000-7001-8000-00000000{prefix}6"),
            conversation: ConversationSelection::New,
            author_message: "Help with this passage.".to_owned(),
            chapter_id,
            ids: ids(&format!("{prefix}6")),
            run_id: format!("018f0000-0000-7001-8000-00000004{prefix}6"),
            conversation_id: format!("018f0000-0000-7001-8000-00000006{prefix}6"),
            project_agent_id: format!("018f0000-0000-7001-8000-00000005{prefix}6"),
        },
    )
    .await
    .unwrap();
    let fence_token = admin
        .query_one(
            "UPDATE storyos.agent_runs
                SET claim_generation = claim_generation + 1,
                    fence_token = claim_generation + 1,
                    lease_expires_at = clock_timestamp(),
                    status = 'claimed'
              WHERE run_id = $1::text::uuid
          RETURNING fence_token",
            &[&admitted.run_id],
        )
        .await
        .unwrap()
        .get(0);
    ClaimedAgentRun {
        project_scope: scope,
        run_id: admitted.run_id,
        fence_token,
    }
}

async fn dispatch_evidence(admin: &Client, run_id: &str) -> DispatchEvidence {
    let row = admin
        .query_one(
            "SELECT run.status, run.settlement::text,
                    (SELECT count(*) FROM storyos.model_attempts AS attempt
                      WHERE attempt.run_id = run.run_id),
                    (SELECT count(outbound_disclosure_event_id) FROM storyos.model_attempts AS attempt
                      WHERE attempt.run_id = run.run_id),
                    (SELECT count(destination_context_manifest_id)
                       FROM storyos.context_assembly_manifests AS assembly
                      WHERE assembly.run_id = run.run_id),
                    (SELECT attempt.payload->>'items' FROM storyos.model_attempts AS attempt
                      WHERE attempt.run_id = run.run_id AND attempt.attempt_role = 'decision'),
                    (SELECT attempt.decision_id::text FROM storyos.model_attempts AS attempt
                      WHERE attempt.run_id = run.run_id AND attempt.attempt_role = 'decision')
               FROM storyos.agent_runs AS run
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    DispatchEvidence {
        status: row.get(0),
        settlement: row.get(1),
        model_attempts: row.get(2),
        disclosure_events: row.get(3),
        destination_manifests: row.get(4),
        items: row.get(5),
        decision_id: row.get(6),
    }
}

async fn stores() -> (PostgresProjectReader, Client) {
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let (admin, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(connection);
    (PostgresProjectReader::new(runtime_url), admin)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn prepare_refusal_records_no_dispatch_evidence() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let claim = claimed_run(&store, &admin, "b91").await;
    let destination = ProbingDestination {
        admin: &admin,
        run_id: claim.run_id.clone(),
        probe: Probe::Refuse,
        seen: Mutex::default(),
        streamed: Mutex::default(),
    };
    let points = RecordedPoints::default();

    let result = complete_agent_run(&store, &destination, &points, &claim).await;

    assert_eq!(
        (result.ok(), points.0.into_inner().unwrap()),
        (Some(CompleteAgentRun::AlreadySettled), Vec::new())
    );
    assert_eq!(
        dispatch_evidence(&admin, &claim.run_id).await,
        DispatchEvidence {
            status: "refused".to_owned(),
            settlement: Some(
                r#"{"kind": "execution_refused", "capability": "destination_credential_unavailable"}"#
                    .to_owned()
            ),
            model_attempts: 0,
            disclosure_events: 0,
            destination_manifests: 0,
            items: None,
            decision_id: None,
        }
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn dispatch_claim_commits_before_an_exchange_with_no_open_transaction() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let claim = claimed_run(&store, &admin, "b92").await;
    let destination = ProbingDestination {
        admin: &admin,
        run_id: claim.run_id.clone(),
        probe: Probe::InspectBeforeExchange,
        seen: Mutex::default(),
        streamed: Mutex::default(),
    };
    let points = RecordedPoints::default();

    let result = complete_agent_run(&store, &destination, &points, &claim).await;

    assert_eq!(
        (
            result.ok(),
            destination.seen.into_inner().unwrap(),
            points.0.into_inner().unwrap()
        ),
        (
            Some(CompleteAgentRun::Settled),
            vec![ExchangeProbe {
                claimed_attempts: 1,
                run_locked: false,
            }],
            vec![
                ContractFaultPoint::DispatchClaimed,
                ContractFaultPoint::StreamCommitted
            ],
        )
    );
    let evidence = dispatch_evidence(&admin, &claim.run_id).await;
    assert_eq!(
        (
            evidence.status.as_str(),
            evidence.model_attempts,
            evidence.disclosure_events,
            evidence.decision_id.is_some()
        ),
        ("completed", 1, 1, true)
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn stale_run_lease_fence_rejects_the_observation() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let claim = claimed_run(&store, &admin, "b93").await;
    let destination = ProbingDestination {
        admin: &admin,
        run_id: claim.run_id.clone(),
        probe: Probe::TakeOverBeforeExchange,
        seen: Mutex::default(),
        streamed: Mutex::default(),
    };
    let points = RecordedPoints::default();

    let result = complete_agent_run(&store, &destination, &points, &claim).await;

    assert!(matches!(result, Err(CompleteAgentRunError::StaleFence)));
    assert_eq!(
        points.0.into_inner().unwrap(),
        vec![ContractFaultPoint::DispatchClaimed]
    );
    assert_eq!(
        dispatch_evidence(&admin, &claim.run_id).await,
        DispatchEvidence {
            status: "claimed".to_owned(),
            settlement: None,
            model_attempts: 1,
            disclosure_events: 1,
            destination_manifests: 1,
            items: Some("[]".to_owned()),
            decision_id: None,
        }
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_new_claim_of_a_cancelled_run_sends_one_abort_through_the_same_sequence() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let claim = claimed_run(&store, &admin, "b94").await;
    let destination = ProbingDestination {
        admin: &admin,
        run_id: claim.run_id.clone(),
        probe: Probe::CancelBeforeExchange,
        seen: Mutex::default(),
        streamed: Mutex::default(),
    };
    let fenced_points = RecordedPoints::default();
    let fenced = complete_agent_run(&store, &destination, &fenced_points, &claim).await;
    let fence_token = admin
        .query_one(
            "UPDATE storyos.agent_runs
                SET claim_generation = claim_generation + 1,
                    fence_token = claim_generation + 1,
                    lease_expires_at = clock_timestamp()
              WHERE run_id = $1::text::uuid AND status = 'cancelled' AND wakeup_pending
          RETURNING fence_token",
            &[&claim.run_id],
        )
        .await
        .unwrap()
        .get(0);
    let cancelled_claim = ClaimedAgentRun {
        fence_token,
        ..claim.clone()
    };
    let points = RecordedPoints::default();

    let result = complete_agent_run(&store, &FakeDestination, &points, &cancelled_claim).await;

    let abort = admin
        .query_one(
            "SELECT count(*), count(DISTINCT abort.outbound_disclosure_event_id),
                    min(abort.payload->>'result'),
                    bool_and(abort.payload->>'original_model_attempt_id'
                             = decision.model_attempt_id::text),
                    bool_and(run.wakeup_pending)
               FROM storyos.model_attempts AS abort
               JOIN storyos.model_attempts AS decision
                 ON decision.run_id = abort.run_id AND decision.attempt_role = 'decision'
               JOIN storyos.agent_runs AS run ON run.run_id = abort.run_id
              WHERE abort.run_id = $1::text::uuid AND abort.attempt_role = 'abort'",
            &[&claim.run_id],
        )
        .await
        .unwrap();
    assert!(matches!(fenced, Err(CompleteAgentRunError::StaleFence)));
    assert_eq!(
        (
            fenced_points.0.into_inner().unwrap(),
            result.ok(),
            points.0.into_inner().unwrap(),
            abort.get::<_, i64>(0),
            abort.get::<_, i64>(1),
            abort.get::<_, Option<String>>(2),
            abort.get::<_, Option<bool>>(3),
            abort.get::<_, Option<bool>>(4),
        ),
        (
            vec![ContractFaultPoint::DispatchClaimed],
            Some(CompleteAgentRun::AlreadySettled),
            vec![ContractFaultPoint::DispatchClaimed],
            1,
            1,
            Some("acknowledged".to_owned()),
            Some(true),
            Some(false),
        )
    );
    assert_eq!(
        dispatch_evidence(&admin, &claim.run_id).await,
        DispatchEvidence {
            status: "cancelled".to_owned(),
            settlement: None,
            model_attempts: 2,
            disclosure_events: 2,
            destination_manifests: 1,
            items: Some("[]".to_owned()),
            decision_id: None,
        }
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn stream_batches_keep_earlier_items_and_replace_by_item_id() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let claim = claimed_run(&store, &admin, "b95").await;
    let destination = ProbingDestination {
        admin: &admin,
        run_id: claim.run_id.clone(),
        probe: Probe::StreamTwoBatches,
        seen: Mutex::default(),
        streamed: Mutex::default(),
    };
    let points = RecordedPoints::default();

    let result = complete_agent_run(&store, &destination, &points, &claim).await;

    let persisted: String = admin
        .query_one(
            "SELECT payload->>'items' FROM storyos.model_attempts
              WHERE run_id = $1::text::uuid AND attempt_role = 'decision'",
            &[&claim.run_id],
        )
        .await
        .unwrap()
        .get(0);
    let item = |item_id: &str, state: &str, text: &str| {
        serde_json::json!({
            "item_id": item_id, "role": "assistant", "state": state, "phase": state,
            "text": text, "summary": null, "call_id": null, "arguments": null,
            "refusal": null, "hosted_report": null
        })
    };
    let kept = serde_json::json!([
        item("1", "complete", "Guard the voice"),
        item("3", "provisional", "Hold"),
        item("2", "complete", "Keep it"),
    ]);
    assert_eq!(
        (
            result.ok(),
            destination.streamed.into_inner().unwrap(),
            serde_json::from_str::<serde_json::Value>(&persisted).ok(),
            points.0.into_inner().unwrap(),
        ),
        (
            Some(CompleteAgentRun::AlreadySettled),
            Some(kept.clone()),
            Some(kept),
            vec![
                ContractFaultPoint::DispatchClaimed,
                ContractFaultPoint::StreamCommitted,
                ContractFaultPoint::StreamCommitted,
            ],
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_refused_cancellation_duty_ends_the_cancelled_claim() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let claim = claimed_run(&store, &admin, "b96").await;
    let cancelling = ProbingDestination {
        admin: &admin,
        run_id: claim.run_id.clone(),
        probe: Probe::CancelBeforeExchange,
        seen: Mutex::default(),
        streamed: Mutex::default(),
    };
    let fenced = complete_agent_run(&store, &cancelling, &NoContractFaults, &claim).await;
    let fence_token = admin
        .query_one(
            "UPDATE storyos.agent_runs
                SET claim_generation = claim_generation + 1,
                    fence_token = claim_generation + 1,
                    lease_expires_at = clock_timestamp()
              WHERE run_id = $1::text::uuid AND status = 'cancelled' AND wakeup_pending
          RETURNING fence_token",
            &[&claim.run_id],
        )
        .await
        .unwrap()
        .get(0);
    let refusing = ProbingDestination {
        admin: &admin,
        run_id: claim.run_id.clone(),
        probe: Probe::Refuse,
        seen: Mutex::default(),
        streamed: Mutex::default(),
    };

    let result = complete_agent_run(
        &store,
        &refusing,
        &NoContractFaults,
        &ClaimedAgentRun {
            fence_token,
            ..claim.clone()
        },
    )
    .await;

    let row = admin
        .query_one(
            "SELECT run.wakeup_pending, attempt.payload->>'cancellation_duties_refused',
                    (SELECT count(*) FROM storyos.model_attempts AS abort
                      WHERE abort.run_id = run.run_id AND abort.attempt_role = 'abort')
               FROM storyos.agent_runs AS run
               JOIN storyos.model_attempts AS attempt
                 ON attempt.run_id = run.run_id AND attempt.attempt_role = 'decision'
              WHERE run.run_id = $1::text::uuid",
            &[&claim.run_id],
        )
        .await
        .unwrap();
    assert!(matches!(fenced, Err(CompleteAgentRunError::StaleFence)));
    assert_eq!(
        (
            result.ok(),
            row.get::<_, bool>(0),
            row.get::<_, Option<String>>(1),
            row.get::<_, i64>(2),
        ),
        (
            Some(CompleteAgentRun::AlreadySettled),
            false,
            Some("destination_credential_unavailable".to_owned()),
            0,
        )
    );
}
