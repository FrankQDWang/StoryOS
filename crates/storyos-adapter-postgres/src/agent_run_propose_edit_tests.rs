use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    AcceptProposalInput, AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore,
    ConversationSelection, CreateRequest, DestinationRequest, EditorSessionStore,
    ModelProviderAdapter, ModelStreamSink, NoContractFaults, Observation, PreDispatchRefusal,
    PreparedRequest, ProjectScope, RejectProposalOperationsInput, RejectionNote,
    WithdrawProposalInput, WithdrawalNote, complete_agent_run,
};
use storyos_core::{DeploymentDestination, FINISH_DOES_NOT_COUNT_RESULT, PROPOSAL_WAITS_RESULT};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{
    RuntimeClock, admit_run_in, claim_run, queued_run_offering, stores,
};
use crate::command_sequence::tests::{
    ACCEPT_PROPOSAL, REJECT_PROPOSAL_OPERATIONS, WITHDRAW_PROPOSAL, issued,
};
use crate::set_current_chapter_authority_tests::open_session_request;

pub(crate) const REVISE: &str = "Revise these passages in the narrator voice.";

pub(super) async fn complete(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    run: &str,
) {
    let claim = claim_run(admin, scope.clone(), run.to_owned()).await;
    complete_agent_run(
        store,
        &FakeDestination,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await
    .unwrap();
}

/// Completes the first Run of a new Project of `destination`, then completes one Run with
/// `REVISE` and `script` in a new Conversation. `prefixes` give the identities of the two Runs.
async fn revise_run(
    store: &PostgresProjectReader,
    admin: &Client,
    prefixes: [&str; 2],
    destination: DeploymentDestination,
    script: &str,
) -> (ProjectScope, String) {
    let (scope, chapter_id, first) =
        queued_run_offering(store, prefixes[0], destination.clone(), None).await;
    complete(store, admin, &scope, &first).await;
    let run_id = admit_run_in(
        store,
        &scope,
        &chapter_id,
        prefixes[1],
        destination,
        &format!("{REVISE}{script}"),
        ConversationSelection::New,
        None,
    )
    .await;
    complete(store, admin, &scope, &run_id).await;
    (scope, run_id)
}

/// The Proposal records of the Run with no identity: the Proposal, its Revisions, its
/// Operations, and its Validation Receipts.
pub(super) async fn proposal_records(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT COALESCE(jsonb_agg(jsonb_build_object(
                      'proposal', jsonb_build_array(proposal.kind, proposal.bundle_policy),
                      'revisions', (SELECT jsonb_agg(jsonb_build_array(revision.generation,
                                      revision.validation, revision.closure, revision.candidate_text)
                                      ORDER BY revision.revision_id)
                                      FROM storyos.proposal_revisions AS revision
                                     WHERE revision.proposal_id = proposal.proposal_id),
                      'operations', (SELECT jsonb_agg(jsonb_build_array(operation.resolution,
                                       operation.reservation_state, operation.candidate_text,
                                       cardinality(operation.predecessor_operation_ids))
                                       ORDER BY operation.operation_id)
                                       FROM storyos.proposal_operations AS operation
                                      WHERE operation.proposal_id = proposal.proposal_id),
                      'receipts', (SELECT jsonb_agg(jsonb_build_array(receipt.result,
                                     receipt.candidate_text, receipt.reservation_state))
                                     FROM storyos.validation_receipts AS receipt
                                    WHERE receipt.proposal_id = proposal.proposal_id))
                      ORDER BY proposal.proposal_id), '[]')::text
               FROM storyos.proposals AS proposal
              WHERE proposal.source_run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get(/*idx*/ 0)).unwrap()
}

/// Each `propose_edit` call of the Run in RunStep and item order: its RunStep, state, refusal,
/// and result.
pub(crate) async fn propose_edit_calls(admin: &Client, run_id: &str) -> Vec<Value> {
    admin
        .query(
            "SELECT jsonb_build_array(decision_position, state, refusal, result_text)::text
               FROM storyos.tool_calls
              WHERE run_id = $1::text::uuid AND function_name = 'propose_edit'
              ORDER BY decision_position, call_position",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| serde_json::from_str(row.get(/*idx*/ 0)).unwrap())
        .collect()
}

pub(crate) fn opened(position: u64) -> Value {
    json!([position, "settled", null, PROPOSAL_WAITS_RESULT])
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_valid_call_opens_the_proposal_of_the_prose_change_reply_and_the_run_shows_it() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let reply_form = DeploymentDestination::HostFake;
    let (_, reply_run) = revise_run(&store, &admin, ["6b0", "6b1"], reply_form, "").await;
    let tool_loop = DeploymentDestination::HostFakeToolLoop;
    let (scope, run_id) = revise_run(&store, &admin, ["6b2", "6b3"], tool_loop, "").await;
    let source = admin
        .query_one(
            "SELECT proposal.proposal_id::text, proposal.source_lineage IS NULL,
                    proposal.source_decision_id = call.decision_id
               FROM storyos.proposals AS proposal
               JOIN storyos.tool_calls AS call ON call.run_id = proposal.source_run_id
              WHERE proposal.source_run_id = $1::text::uuid
                AND call.function_name = 'propose_edit'",
            &[&run_id],
        )
        .await
        .unwrap();
    let record = store
        .read_agent_run(&scope, &run_id, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    let AgentRunDecisionInspect::ProseChange {
        text,
        locations: Some(locations),
        opened_proposal_id,
        ..
    } = record.decision
    else {
        panic!("{:?}", record.decision);
    };
    let outcomes: Vec<Value> = locations
        .iter()
        .map(|location| json!(location.outcome)["proposal_id"].clone())
        .collect();
    let proposal_id: String = source.get(/*idx*/ 0);

    assert_eq!(
        (
            proposal_records(&admin, &run_id).await,
            [source.get::<_, bool>(1), source.get(2)],
            propose_edit_calls(&admin, &run_id).await,
            (text, opened_proposal_id, outcomes),
        ),
        (
            proposal_records(&admin, &reply_run).await,
            [true, true],
            vec![opened(0)],
            (
                "Kept the narrator voice.".to_owned(),
                Some(proposal_id.clone()),
                vec![json!(proposal_id)]
            ),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_refused_call_gives_its_reason_and_the_limits_keep_one_proposal_for_each_run() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let refused = |position: u64, label: &str, reason: &str| {
        let text = format!("The Host refused this call because {reason}, so no Proposal opened.");
        json!([position, "refused", label, text])
    };
    let undeclared = |position| {
        let reason = "it names a block that is not a target block of this request";
        refused(position, "undeclared_block", reason)
    };
    let limit = |position: u64| {
        let text = "The Host refused this call because this Run already has 3 refused \
                    propose_edit calls, so call finish and tell the author in the final answer \
                    that you could not make the change.";
        json!([position, "refused", "refusal_limit", text])
    };
    let second = "one response can make only one propose_edit call";
    let after = "this Run already opened a Proposal that waits for the decision of the author";
    let cases = [
        (
            ["6b4", "6b5"],
            "refused_once",
            vec![undeclared(0), opened(1)],
            1,
        ),
        (
            ["6b6", "6b7"],
            "always_refused",
            vec![
                undeclared(0),
                undeclared(1),
                undeclared(2),
                limit(3),
                limit(4),
            ],
            0,
        ),
        (
            ["6b8", "6b9"],
            "twice",
            vec![opened(0), refused(0, "second_in_response", second)],
            1,
        ),
        (
            ["6ba", "6bb"],
            "again",
            vec![opened(0), refused(1, "after_proposal", after)],
            1,
        ),
        // Each refusal of one response counts, and the limit replaces the intake refusal.
        (
            ["6bc", "6bd"],
            "mixed",
            vec![
                undeclared(0),
                refused(0, "second_in_response", second),
                undeclared(1),
                limit(2),
            ],
            0,
        ),
    ];
    let mut runs = Vec::new();
    for (prefixes, script, expected, proposals) in cases {
        let tool_loop = DeploymentDestination::HostFakeToolLoop;
        let message = format!(" SCRIPT:propose_{script}");
        let (_, run_id) = revise_run(&store, &admin, prefixes, tool_loop, &message).await;
        let records = proposal_records(&admin, &run_id).await;
        assert_eq!(
            (
                propose_edit_calls(&admin, &run_id).await,
                records.as_array().unwrap().len()
            ),
            (expected, proposals),
            "{script}"
        );
        runs.push(run_id);
    }
    // A refused call keeps the arguments text of the model.
    let refused_text = admin
        .query_one(
            "SELECT arguments #>> '{}' FROM storyos.tool_calls
              WHERE run_id = $1::text::uuid AND state = 'refused'",
            &[&runs[0]],
        )
        .await
        .unwrap();
    // The corrected call came after a Tool result, so its Proposal has the lineage of ADR 0085.
    let row = admin
        .query_one(
            "SELECT proposal.source_lineage::text, requirement.operation_requirement_id::text,
                    attempt.model_attempt_id::text, refused.tool_call_id::text,
                    proposal.source_decision_id = attempt.decision_id
               FROM storyos.proposals AS proposal
               JOIN storyos.operation_requirements AS requirement
                 ON requirement.run_id = proposal.source_run_id
                AND requirement.decision_position = 1 AND requirement.requirement_role = 'primary'
               JOIN storyos.model_attempts AS attempt
                 ON attempt.run_id = proposal.source_run_id AND attempt.decision_position = 1
                AND attempt.attempt_role = 'decision'
               JOIN storyos.tool_calls AS refused
                 ON refused.run_id = proposal.source_run_id AND refused.decision_position = 0
              WHERE proposal.source_run_id = $1::text::uuid",
            &[&runs[0]],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            serde_json::from_str::<Value>(row.get::<_, &str>(/*idx*/ 0)).unwrap(),
            row.get::<_, bool>(/*idx*/ 4),
            refused_text.get::<_, String>(/*idx*/ 0).contains('\n'),
        ),
        (
            json!({"kind": "tool_result_continuation", "run_id": runs[0],
                   "decision_position": 1,
                   "operation_requirement_id": row.get::<_, String>(/*idx*/ 1),
                   "model_attempt_id": row.get::<_, String>(/*idx*/ 2),
                   "tool_call_ids": [row.get::<_, String>(/*idx*/ 3)]}),
            true,
            true,
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_author_accepts_rejects_and_withdraws_the_proposal_with_the_commands_of_today() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut states = Vec::new();
    for (prefixes, suffix, command) in [
        (["b60", "b61"], 0xb6f0_u16, "accept"),
        (["b62", "b63"], 0xb6f2, "reject"),
        (["b64", "b65"], 0xb6f4, "withdraw"),
    ] {
        let (scope, run_id) = revise_run(
            &store,
            &admin,
            prefixes,
            DeploymentDestination::HostFakeToolLoop,
            "",
        )
        .await;
        let row = admin
            .query_one(
                "SELECT proposal.proposal_id::text, head.current_revision_id::text,
                        receipt.validation_receipt_id::text,
                        array_agg(operation.operation_id::text)
                   FROM storyos.proposals AS proposal
                   JOIN storyos.proposal_heads AS head USING (owner_user_id, project_id, proposal_id)
                   JOIN storyos.validation_receipts AS receipt
                     USING (owner_user_id, project_id, proposal_id)
                   JOIN storyos.proposal_operations AS operation
                     USING (owner_user_id, project_id, proposal_id)
                  WHERE proposal.source_run_id = $1::text::uuid
                  GROUP BY 1, 2, 3",
                &[&run_id],
            )
            .await
            .unwrap();
        let request = open_session_request(&store, &scope, &format!("{suffix:04x}")).await;
        let session = store.create_editor_session(&request).await.unwrap();
        let head = session.base_snapshot.authoritative_revision_id;
        let editor_session_id = request.editor_session_id.clone();
        let (proposal_id, revision_id): (String, String) = (row.get(0), row.get(1));
        let applied = match command {
            "accept" => {
                let input = AcceptProposalInput {
                    editor_session_id,
                    proposal_id: proposal_id.clone(),
                    proposal_revision_id: revision_id,
                    validation_receipt_id: row.get(/*idx*/ 2),
                    selected_operation_ids: row.get(/*idx*/ 3),
                    expected_authoritative_revision_id: head,
                };
                let call = issued(&store, &scope, suffix + 1, &ACCEPT_PROPOSAL, input).await;
                store
                    .accept_proposal(&call.envelope, &call.input)
                    .await
                    .is_ok()
            }
            "reject" => {
                let input = RejectProposalOperationsInput {
                    editor_session_id,
                    proposal_id: proposal_id.clone(),
                    proposal_revision_id: revision_id,
                    selected_pending_operation_ids: row.get(/*idx*/ 3),
                    expected_authoritative_revision_id: head,
                    rejection_note: RejectionNote::Omitted,
                };
                let call = issued(
                    &store,
                    &scope,
                    suffix + 1,
                    &REJECT_PROPOSAL_OPERATIONS,
                    input,
                )
                .await;
                store
                    .reject_proposal_operations(&call.envelope, &call.input)
                    .await
                    .is_ok()
            }
            _ => {
                let input = WithdrawProposalInput {
                    editor_session_id,
                    proposal_id: proposal_id.clone(),
                    proposal_revision_id: revision_id,
                    expected_authoritative_revision_id: head,
                    withdrawal_note: WithdrawalNote::Omitted,
                };
                let call = issued(&store, &scope, suffix + 1, &WITHDRAW_PROPOSAL, input).await;
                store
                    .withdraw_proposal(&call.envelope, &call.input)
                    .await
                    .is_ok()
            }
        };
        let state = admin
            .query_one(
                "SELECT jsonb_build_array(operation.resolution, revision.closure)::text
                   FROM storyos.proposal_heads AS head
                   JOIN storyos.proposal_revisions AS revision
                     ON (revision.proposal_id, revision.revision_id) =
                        (head.proposal_id, head.current_revision_id)
                   JOIN storyos.proposal_operations AS operation
                     ON operation.proposal_id = head.proposal_id
                  WHERE head.proposal_id = $1::text::uuid",
                &[&proposal_id],
            )
            .await
            .unwrap();
        states.push((
            command,
            applied,
            serde_json::from_str::<Value>(state.get(/*idx*/ 0)).unwrap(),
        ));
    }

    assert_eq!(
        states,
        vec![
            ("accept", true, json!(["applied", "open"])),
            ("reject", true, json!(["rejected", "open"])),
            ("withdraw", true, json!(["pending", "withdrawn"])),
        ]
    );
}

/// The fake destination with the response of `REVISE` and `script` for each create, except the
/// first create with `lost` Tool outputs, which keeps the author message of the Run, and the
/// first create with `refused` Tool outputs, which the destination refuses.
struct ReviseScript {
    lost: Option<usize>,
    refused: Option<usize>,
    script: &'static str,
}

impl ModelProviderAdapter for ReviseScript {
    const ADAPTERS: &'static [storyos_core::ModelAdapter] = FakeDestination::ADAPTERS;
    type Prepared = <FakeDestination as ModelProviderAdapter>::Prepared;

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<Self::Prepared>, PreDispatchRefusal> {
        let mut prepared = FakeDestination.prepare(request).await?;
        if let DestinationRequest::Create(create) = request {
            let first = create
                .successor_of
                .is_none()
                .then_some(create.tool_outputs.len());
            if first.is_some() && first == self.refused {
                return Err(PreDispatchRefusal::UnsupportedRequest);
            }
            if first.is_none() || first != self.lost {
                let revise = DestinationRequest::Create(CreateRequest {
                    author_message: format!("{REVISE}{}", self.script),
                    ..create.clone()
                });
                prepared.prepared = FakeDestination.prepare(&revise).await?.prepared;
            }
        }
        Ok(prepared)
    }

    async fn exchange(
        &self,
        prepared: Self::Prepared,
        sink: &mut impl ModelStreamSink,
    ) -> Observation {
        FakeDestination.exchange(prepared, sink).await
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_successor_call_or_a_later_refusal_keeps_the_proposal_of_the_decision_of_the_call() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let undeclared = json!([
        0,
        "refused",
        "undeclared_block",
        "The Host refused this call because it names a block that is not a target block of \
         this request, so no Proposal opened."
    ]);
    let lost = "SCRIPT:successor-once";
    // The successor of the first RunStep, the successor of a continuation RunStep, and a
    // refusal of the destination after the call.
    let cases = [
        (
            ["b66", "b67"],
            lost,
            (Some(0), None, ""),
            vec![opened(0)],
            "successor",
            None,
        ),
        (
            ["b6b", "b6c"],
            lost,
            (Some(1), None, " SCRIPT:propose_refused_once"),
            vec![undeclared, opened(1)],
            "successor",
            Some(true),
        ),
        (
            ["b6d", "b6e"],
            "Revise these passages in the narrator voice. SCRIPT:propose_again",
            (None, Some(1), " SCRIPT:propose_again"),
            vec![opened(0)],
            "decision",
            None,
        ),
    ];
    for (prefixes, message, (lost, refused, script), expected, role, lineage) in cases {
        let destination = DeploymentDestination::HostFakeToolLoop;
        let (scope, chapter_id, first) =
            queued_run_offering(&store, prefixes[0], destination.clone(), None).await;
        complete(&store, &admin, &scope, &first).await;
        let new = ConversationSelection::New;
        let run_id = admit_run_in(
            &store,
            &scope,
            &chapter_id,
            prefixes[1],
            destination,
            message,
            new,
            None,
        )
        .await;
        let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
        let adapter = ReviseScript {
            lost,
            refused,
            script,
        };
        complete_agent_run(&store, &adapter, &NoContractFaults, &RuntimeClock, &claim)
            .await
            .unwrap();
        let source = admin
            .query_one(
                "SELECT proposal.source_decision_id = call.decision_id, attempt.attempt_role,
                        proposal.source_lineage ->> 'model_attempt_id' = call.model_attempt_id::text
                   FROM storyos.proposals AS proposal
                   JOIN storyos.tool_calls AS call ON call.run_id = proposal.source_run_id
                    AND call.function_name = 'propose_edit' AND call.state = 'settled'
                   JOIN storyos.model_attempts AS attempt
                     ON attempt.model_attempt_id = call.model_attempt_id
                  WHERE proposal.source_run_id = $1::text::uuid",
                &[&run_id],
            )
            .await
            .unwrap();
        let record = store
            .read_agent_run(&scope, &run_id, &AgentRunReadSelection::Current)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            (
                propose_edit_calls(&admin, &run_id).await,
                source.get::<_, bool>(/*idx*/ 0),
                source.get::<_, String>(/*idx*/ 1),
                source.get::<_, Option<bool>>(/*idx*/ 2),
                matches!(
                    record.decision,
                    AgentRunDecisionInspect::ProseChange {
                        locations: Some(_),
                        ..
                    }
                ),
            ),
            (expected, true, role.to_owned(), lineage, true),
            "{prefixes:?}"
        );
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_finish_call_settles_after_the_other_host_calls_of_its_response() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let tool_loop = DeploymentDestination::HostFakeToolLoop;
    let uncaptured = storyos_core::SubmitResearchRefusal::Synthesis(
        storyos_core::SynthesisRefusal::UncapturedCitation,
    )
    .result()
    .text;
    // Each response has `finish` first, but `finish` settles last, so its result counts the
    // refusal of a Host call of its response.
    let cases = [
        (
            ["b68", "b69"],
            " SCRIPT:propose_finish_first",
            vec![
                ("propose_edit", 1, PROPOSAL_WAITS_RESULT),
                ("finish", 0, ""),
            ],
        ),
        (
            ["87a", "87b"],
            " SCRIPT:propose_research_finish_first",
            vec![
                ("propose_edit", 1, PROPOSAL_WAITS_RESULT),
                ("submit_research", 2, uncaptured.as_str()),
                ("finish", 0, FINISH_DOES_NOT_COUNT_RESULT),
            ],
        ),
    ];
    for (prefixes, script, expected) in cases {
        let (_, run_id) = revise_run(&store, &admin, prefixes, tool_loop.clone(), script).await;
        let settled = admin
            .query(
                "SELECT call.function_name, call.call_position, call.result_text
                   FROM storyos.agent_run_events AS event
                   JOIN storyos.tool_calls AS call
                     ON call.tool_call_id = (event.payload ->> 'tool_call_id')::uuid
                  WHERE event.run_id = $1::text::uuid AND event.event_kind = 'tool_call_settled'
                    AND call.decision_position = 0
                  ORDER BY event.event_position",
                &[&run_id],
            )
            .await
            .unwrap();
        let settled: Vec<(String, i32, String)> = settled
            .iter()
            .map(|row| (row.get(/*idx*/ 0), row.get(/*idx*/ 1), row.get(/*idx*/ 2)))
            .collect();
        let expected: Vec<(String, i32, String)> = expected
            .into_iter()
            .map(|(name, position, result)| (name.to_owned(), position, result.to_owned()))
            .collect();
        assert_eq!(settled, expected, "{script}");
    }
}
