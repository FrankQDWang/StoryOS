//! A Run of the tool loop profile whose referenced candidate the author changes during the Run
//! (ADR 0085, ADR 0091), with the fake model of that profile.

use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    AcceptProposalInput, AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore,
    ChapterId, DeleteChapterInput, ProjectScope, RejectProposalOperationsInput, RejectionNote,
    WithdrawProposalInput, WithdrawalNote,
};
use storyos_core::{DeploymentDestination, SelectionReference};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{admit_referenced_run, stores};
use crate::agent_run_propose_edit::revise_tests::{CandidateChange, Fixture};
use crate::agent_run_propose_edit::tests::{REVISE, opened, propose_edit_calls};
use crate::agent_run_steering::tests::{
    AuthorAction, AuthorAt, Moment, settle, steering, step_references,
};
use crate::command_sequence::tests::{
    ACCEPT_PROPOSAL, DELETE_CHAPTER, REJECT_PROPOSAL_OPERATIONS, WITHDRAW_PROPOSAL, delete_chapter,
    issued, named_call,
};

const EDITED: &str = "Still, ";
const FINAL_TEXT: &str =
    "This passage is inspectable Host-fake advice. It is not Authoritative State.";

/// The revision of the head of the Proposal of `reference`, and its candidate text.
async fn current(admin: &Client, reference: &SelectionReference) -> (String, String) {
    let target = reference.candidate_target().expect("candidate reference");
    let row = admin
        .query_one(
            "SELECT head.current_revision_id::text, operation.candidate_text
               FROM storyos.proposal_heads AS head
               JOIN storyos.proposal_operations AS operation USING (owner_user_id, project_id, proposal_id)
              WHERE head.proposal_id = $1::text::uuid AND operation.operation_id = $2::text::uuid",
            &[&target.proposal_id, &target.operation_id],
        )
        .await
        .unwrap();
    (row.get(/*idx*/ 0), row.get(/*idx*/ 1))
}

/// `reference` at `revision_id`, with a range over all of `text`.
fn moved(reference: &SelectionReference, revision_id: &str, text: &str) -> Value {
    let mut moved = json!(reference);
    moved["revision_id"] = json!(revision_id);
    moved["start_offset"] = json!(0);
    moved["end_offset"] = json!(text.encode_utf16().count());
    moved
}

/// For the RunStep at `position`: the content of each selected item of the candidate revision
/// `revision_id` in its Context Assembly record, and the selected text of each reference that
/// its request sent.
async fn sent_revision(admin: &Client, run_id: &str, position: &str, revision_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_build_array(
                      (SELECT jsonb_path_query_array(manifest.payload,
                                '$.selected[*] ? (@.source_version == $revision).content',
                                jsonb_build_object('revision', $3::text))
                         FROM storyos.context_assembly_manifests AS manifest
                        WHERE manifest.run_id = $1::text::uuid
                          AND manifest.decision_position = $2::text::numeric),
                      (SELECT jsonb_path_query_array(
                                (attempt.payload #>> '{wire,serialized_payload}')::jsonb,
                                '$.selection_references[*].selected_text')
                         FROM storyos.model_attempts AS attempt
                        WHERE attempt.run_id = $1::text::uuid AND attempt.attempt_role = 'decision'
                          AND attempt.decision_position = $2::text::numeric))::text",
            &[&run_id, &position, &revision_id],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get(/*idx*/ 0)).unwrap()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_next_run_step_reads_the_candidate_that_the_author_edited_and_a_later_call_revises_it()
{
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let tool_loop = DeploymentDestination::HostFakeToolLoop;
    let fixture = Fixture::new(&store, &admin, 0x6f0, tool_loop.clone()).await;
    let (scope, chapter_id) = (&fixture.scope, &fixture.chapter_id);
    let reference = fixture.candidates(&admin).await.remove(0);
    let (_, before) = current(&admin, &reference).await;
    let message = format!("{REVISE} SCRIPT:propose_again");
    let run_id = admit_referenced_run(
        &store,
        scope,
        chapter_id,
        "6f2",
        tool_loop,
        &message,
        vec![reference.clone()],
    )
    .await;
    let edit = fixture
        .candidate_edit(
            &store,
            &admin,
            &reference,
            2,
            CandidateChange::Prefix(EDITED),
        )
        .await;
    let edited_text = format!("{EDITED}{before}");
    // The author edits the candidate while the first request is out.
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(1),
        AuthorAction::Edit(Box::new(edit)),
    );
    settle(&store, &admin, scope, &run_id, &author)
        .await
        .unwrap();
    let (revised, revised_text) = current(&admin, &reference).await;
    let edited_revision: String = admin
        .query_one(
            "SELECT revision_id::text FROM storyos.proposal_revisions
              WHERE candidate_text = $1 AND proposal_id = $2::text::uuid",
            &[
                &edited_text,
                &reference.candidate_target().unwrap().proposal_id,
            ],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let stale = json!([
        0,
        "refused",
        "stale_candidate",
        "The Host refused this call because the candidate changed after this request read it, \
         so no Proposal opened."
    ]);

    // The first call is refused. The next RunStep reads the edited revision, and its call
    // revises that revision.
    assert_eq!(
        (
            propose_edit_calls(&admin, &run_id).await,
            step_references(&admin, &run_id).await,
            sent_revision(&admin, &run_id, "1", &edited_revision).await,
            crate::agent_run_steering::tests::run_steps(&admin, &run_id).await["status"].clone(),
        ),
        (
            vec![stale, opened(1)],
            json!([
                [0, [reference]],
                [1, [moved(&reference, &edited_revision, &edited_text)]],
                [2, [moved(&reference, &revised, &revised_text)]],
                [3, [moved(&reference, &revised, &revised_text)]]
            ]),
            json!([[edited_text], [edited_text]]),
            json!("completed"),
        )
    );
}

/// The status and the public decision of the Run.
async fn settled(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    run_id: &str,
) -> (String, AgentRunDecisionInspect) {
    let record = store
        .read_agent_run(scope, run_id, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    (format!("{:?}", record.status), record.decision)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_run_with_no_accepted_call_after_the_author_edit_completes_with_its_final_text() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let tool_loop = DeploymentDestination::HostFakeToolLoop;
    let fixture = Fixture::new(&store, &admin, 0x6f8, tool_loop.clone()).await;
    let (scope, chapter_id) = (&fixture.scope, &fixture.chapter_id);
    let reference = fixture.candidates(&admin).await.remove(0);
    let run_id = admit_referenced_run(
        &store,
        scope,
        chapter_id,
        "6fa",
        tool_loop,
        REVISE,
        vec![reference.clone()],
    )
    .await;
    let edit = fixture
        .candidate_edit(
            &store,
            &admin,
            &reference,
            2,
            CandidateChange::Prefix(EDITED),
        )
        .await;
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(1),
        AuthorAction::Edit(Box::new(edit)),
    );
    settle(&store, &admin, scope, &run_id, &author)
        .await
        .unwrap();
    let (edited, edited_text) = current(&admin, &reference).await;
    let (status, decision) = settled(&store, scope, &run_id).await;
    let AgentRunDecisionInspect::Advisory { text, .. } = decision else {
        panic!("{decision:?}");
    };

    // The call of the first RunStep is refused and its `finish` call does not count. The next
    // RunStep calls only `finish`, and the closing RunStep reads the edited text and answers.
    assert_eq!(
        (
            status,
            text,
            step_references(&admin, &run_id).await,
            sent_revision(&admin, &run_id, "2", &edited).await,
        ),
        (
            "Completed".to_owned(),
            FINAL_TEXT.to_owned(),
            json!([
                [0, [reference]],
                [1, [moved(&reference, &edited, &edited_text)]],
                [2, [moved(&reference, &edited, &edited_text)]]
            ]),
            json!([[edited_text], [edited_text]]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_candidate_that_the_author_resolved_keeps_the_refusal_of_the_run() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let tool_loop = DeploymentDestination::HostFakeToolLoop;
    let mut observed = Vec::new();
    for (lead, run_prefix, suffix, decision) in [
        (0x3f0, "3f2", 0x3fe0_u16, "accept"),
        (0x3f8, "3fa", 0x3fe2, "reject"),
        (0x6d0, "6d2", 0x6de0, "withdraw"),
    ] {
        let fixture = Fixture::new(&store, &admin, lead, tool_loop.clone()).await;
        let (scope, chapter_id) = (&fixture.scope, &fixture.chapter_id);
        let reference = fixture.candidates(&admin).await.remove(0);
        let target = reference.candidate_target().unwrap();
        let run_id = admit_referenced_run(
            &store,
            scope,
            chapter_id,
            run_prefix,
            tool_loop.clone(),
            REVISE,
            vec![reference.clone()],
        )
        .await;
        let (editor_session_id, head) = fixture.writer();
        let receipt: String = admin
            .query_one(
                "SELECT validation_receipt_id::text FROM storyos.validation_receipts
                  WHERE proposal_revision_id = $1::text::uuid",
                &[&target.revision_id],
            )
            .await
            .unwrap()
            .get(/*idx*/ 0);
        let operations = vec![target.operation_id.clone()];
        let action = match decision {
            "accept" => {
                let input = AcceptProposalInput {
                    editor_session_id,
                    proposal_id: target.proposal_id,
                    proposal_revision_id: target.revision_id,
                    validation_receipt_id: receipt,
                    selected_operation_ids: operations,
                    expected_authoritative_revision_id: head.to_owned(),
                };
                let call = issued(&store, scope, suffix, &ACCEPT_PROPOSAL, input).await;
                AuthorAction::Accept(Box::new(call))
            }
            "reject" => {
                let input = RejectProposalOperationsInput {
                    editor_session_id,
                    proposal_id: target.proposal_id,
                    proposal_revision_id: target.revision_id,
                    selected_pending_operation_ids: operations,
                    expected_authoritative_revision_id: head.to_owned(),
                    rejection_note: RejectionNote::Omitted,
                };
                let call = issued(&store, scope, suffix, &REJECT_PROPOSAL_OPERATIONS, input).await;
                AuthorAction::Reject(Box::new(call))
            }
            _ => {
                let input = WithdrawProposalInput {
                    editor_session_id,
                    proposal_id: target.proposal_id,
                    proposal_revision_id: target.revision_id,
                    expected_authoritative_revision_id: head.to_owned(),
                    withdrawal_note: WithdrawalNote::Omitted,
                };
                let call = issued(&store, scope, suffix, &WITHDRAW_PROPOSAL, input).await;
                AuthorAction::Withdraw(Box::new(call))
            }
        };
        let author = AuthorAt::new(&store, Moment::DuringRequest(1), action);
        settle(&store, &admin, scope, &run_id, &author)
            .await
            .unwrap();
        observed.push((
            decision,
            settled(&store, scope, &run_id).await,
            step_references(&admin, &run_id).await,
        ));
    }
    let expected = |decision, reference: &Value| {
        (
            decision,
            (
                "Refused".to_owned(),
                AgentRunDecisionInspect::ExecutionRefused {
                    capability: "blocked_context".to_owned(),
                },
            ),
            json!([[0, [reference]], [1, [reference]]]),
        )
    };
    let references: Vec<Value> = observed
        .iter()
        .map(|(_, _, steps)| steps[0][1][0].clone())
        .collect();

    // The references stay at their revision, and the next RunStep is blocked as before.
    assert_eq!(
        observed,
        vec![
            expected("accept", &references[0]),
            expected("reject", &references[1]),
            expected("withdraw", &references[2]),
        ]
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_steering_reference_to_a_candidate_that_the_author_edited_moves_to_its_current_revision()
{
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let tool_loop = DeploymentDestination::HostFakeToolLoop;
    let fixture = Fixture::new(&store, &admin, 0x8e0, tool_loop.clone()).await;
    let (scope, chapter_id) = (&fixture.scope, &fixture.chapter_id);
    let reference = fixture.candidates(&admin).await.remove(0);
    let run_id = admit_referenced_run(
        &store,
        scope,
        chapter_id,
        "8e2",
        tool_loop,
        "Help with this passage.",
        Vec::new(),
    )
    .await;
    let input = steering(
        &store,
        scope,
        0x8ee0,
        &run_id,
        REVISE,
        Some(vec![reference.clone()]),
    )
    .await;
    let edit = fixture
        .candidate_edit(
            &store,
            &admin,
            &reference,
            2,
            CandidateChange::Prefix(EDITED),
        )
        .await;
    // The steering input names the candidate, and then the author edits it.
    let action = AuthorAction::Sequence(vec![
        AuthorAction::Steer(vec![input]),
        AuthorAction::Edit(Box::new(edit)),
    ]);
    let author = AuthorAt::new(&store, Moment::DuringRequest(1), action);
    settle(&store, &admin, scope, &run_id, &author)
        .await
        .unwrap();
    let edited: String = admin
        .query_one(
            "SELECT revision_id::text FROM storyos.proposal_revisions
              WHERE candidate_text LIKE $1 AND proposal_id = $2::text::uuid",
            &[
                &format!("{EDITED}%"),
                &reference.candidate_target().unwrap().proposal_id,
            ],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let edited_text: String = admin
        .query_one(
            "SELECT candidate_text FROM storyos.proposal_revisions WHERE revision_id = $1::text::uuid",
            &[&edited],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let (status, _) = settled(&store, scope, &run_id).await;
    let steps = step_references(&admin, &run_id).await;

    // The RunStep that reads the input reads the edited revision of its reference.
    assert_eq!(
        (
            status,
            steps[1].clone(),
            sent_revision(&admin, &run_id, "1", &edited).await
        ),
        (
            "Completed".to_owned(),
            json!([1, [moved(&reference, &edited, &edited_text)]]),
            json!([[edited_text], [edited_text]]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_run_whose_candidate_the_author_emptied_completes_with_its_final_text() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let tool_loop = DeploymentDestination::HostFakeToolLoop;
    let fixture = Fixture::new(&store, &admin, 0x8e8, tool_loop.clone()).await;
    let (scope, chapter_id) = (&fixture.scope, &fixture.chapter_id);
    let reference = fixture.candidates(&admin).await.remove(0);
    let run_id = admit_referenced_run(
        &store,
        scope,
        chapter_id,
        "8ea",
        tool_loop,
        REVISE,
        vec![reference.clone()],
    )
    .await;
    let edit = fixture
        .candidate_edit(&store, &admin, &reference, 2, CandidateChange::RemoveAll)
        .await;
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(1),
        AuthorAction::Edit(Box::new(edit)),
    );
    settle(&store, &admin, scope, &run_id, &author)
        .await
        .unwrap();
    let (emptied, emptied_text) = current(&admin, &reference).await;
    let (status, decision) = settled(&store, scope, &run_id).await;
    let AgentRunDecisionInspect::Advisory { text, .. } = decision else {
        panic!("{decision:?}");
    };

    // The reference covers the empty candidate, and the closing RunStep answers.
    assert_eq!(
        (
            status,
            text,
            emptied_text.clone(),
            step_references(&admin, &run_id).await[2].clone(),
            sent_revision(&admin, &run_id, "2", &emptied).await,
        ),
        (
            "Completed".to_owned(),
            FINAL_TEXT.to_owned(),
            String::new(),
            json!([2, [moved(&reference, &emptied, "")]]),
            json!([[""], [""]]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_removed_chapter_or_a_chapter_over_the_item_limit_keeps_the_refusal_of_the_run() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let tool_loop = DeploymentDestination::HostFakeToolLoop;
    let mut observed = Vec::new();
    for (lead, run_prefix, change) in [(0x3d0, "3d2", "removed"), (0x3d8, "3da", "over_limit")] {
        let fixture = Fixture::new(&store, &admin, lead, tool_loop.clone()).await;
        let (scope, chapter_id) = (&fixture.scope, &fixture.chapter_id);
        let reference = fixture.candidates(&admin).await.remove(0);
        let run_id = admit_referenced_run(
            &store,
            scope,
            chapter_id,
            run_prefix,
            tool_loop.clone(),
            REVISE,
            vec![reference],
        )
        .await;
        // The Chapter changes after the admission of the Run, before its first request.
        if change == "removed" {
            let expected_tree_revision: i64 = admin
                .query_one(
                    "SELECT tree_revision FROM storyos.projects WHERE project_id = $1::text::uuid",
                    &[&scope.project_id.as_ref()],
                )
                .await
                .unwrap()
                .get(/*idx*/ 0);
            let input = DeleteChapterInput {
                chapter_id: ChapterId::new(chapter_id.as_str()),
                expected_tree_revision: expected_tree_revision as u64,
            };
            let call = named_call(&store, scope, "3de0", &DELETE_CHAPTER, input).await;
            delete_chapter(&store, &call).await.unwrap();
        } else {
            fixture.edit_chapter(&store, 2, &"a".repeat(32_001)).await;
        }
        settle(&store, &admin, scope, &run_id, &FakeDestination)
            .await
            .unwrap();
        observed.push((change, settled(&store, scope, &run_id).await));
    }
    let refused = (
        "Refused".to_owned(),
        AgentRunDecisionInspect::ExecutionRefused {
            capability: "blocked_context".to_owned(),
        },
    );

    assert_eq!(
        observed,
        vec![("removed", refused.clone()), ("over_limit", refused)]
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_steering_reference_keeps_its_range_when_the_author_edits_another_candidate_of_its_proposal()
 {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let tool_loop = DeploymentDestination::HostFakeToolLoop;
    let grow = format!("{REVISE} SCRIPT:grow_proposal");
    let mut observed = Vec::new();
    let mut expected = Vec::new();
    // The steering input names one candidate of a grown Proposal, and the author edits another:
    // first a joined candidate and the candidate of the Proposal block, then the reverse.
    for (lead, run_prefix, suffix, referenced, edited) in [
        (0xbf0, "bf3", 0xbfe0_u16, 0, 2),
        (0xbf8, "bfb", 0xbfe8, 2, 0),
    ] {
        let fixture = Fixture::new(&store, &admin, lead, tool_loop.clone()).await;
        let (scope, chapter_id) = (&fixture.scope, &fixture.chapter_id);
        let mut references = fixture.candidates(&admin).await;
        references.insert(0, fixture.chapter_text(1, 2));
        fixture.run(&store, &admin, 2, &grow, references).await;
        let candidates = fixture.candidates(&admin).await;
        let mut part = json!(candidates[referenced]);
        part["end_offset"] = json!(3);
        let part: SelectionReference = serde_json::from_value(part).unwrap();
        let run_id = admit_referenced_run(
            &store,
            scope,
            chapter_id,
            run_prefix,
            tool_loop.clone(),
            "Help with this passage.",
            Vec::new(),
        )
        .await;
        let input = steering(
            &store,
            scope,
            suffix,
            &run_id,
            REVISE,
            Some(vec![part.clone()]),
        )
        .await;
        let edit = fixture
            .candidate_edit(
                &store,
                &admin,
                &candidates[edited],
                2,
                CandidateChange::Prefix(EDITED),
            )
            .await;
        let action = AuthorAction::Sequence(vec![
            AuthorAction::Steer(vec![input]),
            AuthorAction::Edit(Box::new(edit)),
        ]);
        let author = AuthorAt::new(&store, Moment::DuringRequest(1), action);
        settle(&store, &admin, scope, &run_id, &author)
            .await
            .unwrap();
        let edited_revision: String = admin
            .query_one(
                "SELECT revision_id::text FROM storyos.proposal_revisions
                  WHERE edited_operation_id = $1::text::uuid",
                &[&candidates[edited].candidate_target().unwrap().operation_id],
            )
            .await
            .unwrap()
            .get(/*idx*/ 0);
        let mut moved = json!(part);
        moved["revision_id"] = json!(edited_revision);
        observed.push(step_references(&admin, &run_id).await[1].clone());
        expected.push(json!([1, [moved]]));
    }

    assert_eq!(observed, expected);
}
