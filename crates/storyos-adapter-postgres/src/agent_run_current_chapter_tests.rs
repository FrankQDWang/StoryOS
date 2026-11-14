//! A Run of the tool loop profile whose referenced Chapter text the author changes during the
//! Run (ADR 0062, ADR 0085, ADR 0091), with the fake model of that profile.

use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    AcceptProposalInput, AgentRunDecisionInspect, ChapterId, DeleteChapterInput,
};
use storyos_core::{AuthorEditPrimitive, DeploymentDestination, SelectionReference, TextPoint};
use tokio_postgres::Client;

use crate::agent_run_current_candidate::tests::settled;
use crate::agent_run_dispatch::tests::{admit_referenced_run, stores};
use crate::agent_run_propose_edit::revise_tests::Fixture;
use crate::agent_run_propose_edit::tests::{REVISE, opened, propose_edit_calls};
use crate::agent_run_steering::tests::{
    AuthorAction, AuthorAt, Moment, settle, steering, step_references,
};
use crate::command_sequence::tests::{ACCEPT_PROPOSAL, DELETE_CHAPTER, issued, named_call};

const MESSAGE: &str = "Help with this passage.";
const EDITED: &str = "Still, ";
const FINAL_TEXT: &str =
    "This passage is inspectable Host-fake advice. It is not Authoritative State.";

/// The current head of the Chapter of the fixture.
async fn head(admin: &Client, fixture: &Fixture) -> String {
    admin
        .query_one(
            "SELECT current_revision_id::text FROM storyos.authoritative_heads
              WHERE manuscript_object_id = $1::text::uuid",
            &[&fixture.chapter_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// A Chapter text reference of the fixture at `base`, from `start` to `end` as (Block, offset).
fn range(
    fixture: &Fixture,
    base: &str,
    (start_block, start): (usize, u32),
    (end_block, end): (usize, u32),
) -> SelectionReference {
    SelectionReference::ChapterText {
        chapter_id: fixture.chapter_id.clone(),
        base_authoritative_revision_id: base.to_owned(),
        start: TextPoint {
            manuscript_block_id: fixture.block(start_block).to_owned(),
            offset: start,
        },
        end: TextPoint {
            manuscript_block_id: fixture.block(end_block).to_owned(),
            offset: end,
        },
    }
}

/// The reference that a RunStep sends: the reference, its covered Blocks of the fixture, and its
/// selected text.
fn sent_reference(
    fixture: &Fixture,
    reference: &SelectionReference,
    blocks: &[usize],
    text: &str,
) -> Value {
    json!({
        "reference": reference,
        "manuscript_block_ids": blocks.iter().map(|index| fixture.block(*index)).collect::<Vec<_>>(),
        "selected_text": text,
    })
}

/// For the RunStep at `position`: the references of its Context Assembly record and the
/// references that its request sent.
async fn sent(admin: &Client, run_id: &str, position: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_build_array(
                      (SELECT payload #> '{operation_requirement,selection_references}'
                         FROM storyos.context_assembly_manifests
                        WHERE run_id = $1::text::uuid AND decision_position = $2::text::numeric),
                      (SELECT (payload #>> '{wire,serialized_payload}')::jsonb -> 'selection_references'
                         FROM storyos.model_attempts
                        WHERE run_id = $1::text::uuid AND attempt_role = 'decision'
                          AND decision_position = $2::text::numeric))::text",
            &[&run_id, &position],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get(/*idx*/ 0)).unwrap()
}

/// `text` before the text of Block `index`.
fn prefix(fixture: &Fixture, index: usize) -> AuthorEditPrimitive {
    AuthorEditPrimitive::ReplaceBlockSelection {
        manuscript_block_id: fixture.block(index).to_owned(),
        from: 0,
        to: 0,
        text: EDITED.to_owned(),
    }
}

fn completed() -> (String, String) {
    ("Completed".to_owned(), FINAL_TEXT.to_owned())
}

fn refused() -> (String, String) {
    ("Refused".to_owned(), "blocked_context".to_owned())
}

/// The status of the Run and the text or the refusal of its public decision.
async fn outcome(
    store: &crate::PostgresProjectReader,
    fixture: &Fixture,
    run_id: &str,
) -> (String, String) {
    let (status, decision) = settled(store, &fixture.scope, run_id).await;
    let summary = match decision {
        AgentRunDecisionInspect::Advisory { text, .. } => text,
        AgentRunDecisionInspect::ExecutionRefused { capability } => capability,
        other => format!("{other:?}"),
    };
    (status, summary)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_closing_run_step_reads_the_chapter_head_after_an_author_edit_of_another_block() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let tool_loop = DeploymentDestination::HostFakeToolLoop;
    let fixture = Fixture::new(&store, &admin, 0x2c0, tool_loop.clone()).await;
    let reference = fixture.chapter_text(1, 2);
    let run_id = admit_referenced_run(
        &store,
        &fixture.scope,
        &fixture.chapter_id,
        "2c2",
        tool_loop,
        MESSAGE,
        vec![reference.clone()],
    )
    .await;
    let edit = fixture
        .chapter_edit(&store, 2, vec![prefix(&fixture, 0)])
        .await;
    // The author edits a Block that the reference does not cover while the first request is out.
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(1),
        AuthorAction::Edit(Box::new(edit)),
    );
    settle(&store, &admin, &fixture.scope, &run_id, &author)
        .await
        .unwrap();
    let moved = range(&fixture, &head(&admin, &fixture).await, (1, 0), (2, 11));
    let sent_moved = sent_reference(&fixture, &moved, &[1, 2], "Wind rose.\nNight came.");

    // The first RunStep calls only `finish`. The closing RunStep sends the reference with the
    // head as its base, the same range, and the same text, and the Run completes.
    assert_eq!(
        (
            outcome(&store, &fixture, &run_id).await,
            step_references(&admin, &run_id).await,
            sent(&admin, &run_id, "1").await,
        ),
        (
            completed(),
            json!([[0, [reference]], [1, [moved]]]),
            json!([[sent_moved], [sent_moved]]),
        )
    );
}

/// Admits a Run of `message` with `references` and settles it while the author makes `edit`
/// during the first request.
async fn run_with_edit(
    store: &crate::PostgresProjectReader,
    admin: &Client,
    fixture: &Fixture,
    run_prefix: &str,
    message: &str,
    references: Vec<SelectionReference>,
    action: AuthorAction,
) -> String {
    let run_id = admit_referenced_run(
        store,
        &fixture.scope,
        &fixture.chapter_id,
        run_prefix,
        DeploymentDestination::HostFakeToolLoop,
        message,
        references,
    )
    .await;
    let author = AuthorAt::new(store, Moment::DuringRequest(1), action);
    settle(store, admin, &fixture.scope, &run_id, &author)
        .await
        .unwrap();
    run_id
}

/// The candidate text of the pending Proposal of the fixture.
async fn candidate_text(admin: &Client, reference: &SelectionReference) -> String {
    admin
        .query_one(
            "SELECT candidate_text FROM storyos.proposal_operations
              WHERE operation_id = $1::text::uuid",
            &[&reference.candidate_target().unwrap().operation_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_changed_start_or_end_block_moves_the_range_and_the_next_run_step_sends_the_current_text()
{
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    let mut expected = Vec::new();
    for (lead, run_prefix, change) in [
        (0x2c8, "2ca", "start"),
        (0x1c0, "1c2", "end"),
        (0x1c8, "1ca", "both"),
        (0x1b0, "1b2", "accepted_end"),
    ] {
        let fixture = Fixture::new(
            &store,
            &admin,
            lead,
            DeploymentDestination::HostFakeToolLoop,
        )
        .await;
        let (editor_session_id, fixture_head) = fixture.writer();
        let fixture_head = fixture_head.to_owned();
        // The range starts in "Rain fell." and ends in "Night came.", or for the accepted
        // Proposal starts in "Night came." and ends in "Dawn broke.", the Block of the Proposal.
        let (reference, action) = if change == "accepted_end" {
            let candidate = fixture.candidates(&admin).await.remove(0);
            let target = candidate.candidate_target().unwrap();
            let receipt: String = admin
                .query_one(
                    "SELECT validation_receipt_id::text FROM storyos.validation_receipts
                      WHERE proposal_revision_id = $1::text::uuid",
                    &[&target.revision_id],
                )
                .await
                .unwrap()
                .get(/*idx*/ 0);
            let input = AcceptProposalInput {
                editor_session_id,
                proposal_id: target.proposal_id,
                proposal_revision_id: target.revision_id,
                validation_receipt_id: receipt,
                selected_operation_ids: vec![target.operation_id],
                expected_authoritative_revision_id: fixture_head.clone(),
            };
            let call = issued(&store, &fixture.scope, 0x1b50, &ACCEPT_PROPOSAL, input).await;
            (
                range(&fixture, &fixture_head, (2, 5), (3, 4)),
                AuthorAction::Accept(Box::new(call)),
            )
        } else {
            let primitives = match change {
                "start" => vec![prefix(&fixture, 0)],
                "end" => vec![prefix(&fixture, 2)],
                _ => vec![prefix(&fixture, 0), prefix(&fixture, 2)],
            };
            let edit = fixture.chapter_edit(&store, 2, primitives).await;
            (
                range(&fixture, &fixture_head, (0, 5), (2, 5)),
                AuthorAction::Edit(Box::new(edit)),
            )
        };
        let accepted = match change {
            "accepted_end" => candidate_text(&admin, &fixture.candidates(&admin).await[0]).await,
            _ => String::new(),
        };
        let run_id = run_with_edit(
            &store,
            &admin,
            &fixture,
            run_prefix,
            MESSAGE,
            vec![reference],
            action,
        )
        .await;
        let head = head(&admin, &fixture).await;
        let edited_end = (EDITED.len() + "Night came.".len()) as u32;
        let (moved, blocks, text) = match change {
            "start" => (
                range(&fixture, &head, (0, 0), (2, 5)),
                vec![0, 1, 2],
                "Still, Rain fell.\nWind rose.\nNight".to_owned(),
            ),
            "end" => (
                range(&fixture, &head, (0, 5), (2, edited_end)),
                vec![0, 1, 2],
                "fell.\nWind rose.\nStill, Night came.".to_owned(),
            ),
            "both" => (
                range(&fixture, &head, (0, 0), (2, edited_end)),
                vec![0, 1, 2],
                "Still, Rain fell.\nWind rose.\nStill, Night came.".to_owned(),
            ),
            _ => (
                range(
                    &fixture,
                    &head,
                    (2, 5),
                    (3, accepted.encode_utf16().count() as u32),
                ),
                vec![2, 3],
                format!(" came.\n{accepted}"),
            ),
        };
        let sent_moved = sent_reference(&fixture, &moved, &blocks, &text);
        observed.push((
            change,
            outcome(&store, &fixture, &run_id).await,
            step_references(&admin, &run_id).await[1].clone(),
            sent(&admin, &run_id, "1").await,
        ));
        expected.push((
            change,
            completed(),
            json!([1, [moved]]),
            json!([[sent_moved], [sent_moved]]),
        ));
    }

    assert_eq!(observed, expected);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_steering_reference_moves_to_the_chapter_head_that_the_author_edited() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let fixture = Fixture::new(
        &store,
        &admin,
        0x1b8,
        DeploymentDestination::HostFakeToolLoop,
    )
    .await;
    let reference = fixture.chapter_text(1, 2);
    let run_id = admit_referenced_run(
        &store,
        &fixture.scope,
        &fixture.chapter_id,
        "1ba",
        DeploymentDestination::HostFakeToolLoop,
        MESSAGE,
        Vec::new(),
    )
    .await;
    let input = steering(
        &store,
        &fixture.scope,
        0x1bd0,
        &run_id,
        MESSAGE,
        Some(vec![reference]),
    )
    .await;
    let edit = fixture
        .chapter_edit(&store, 2, vec![prefix(&fixture, 2)])
        .await;
    // The steering input names the text, and then the author edits its end Block.
    let action = AuthorAction::Sequence(vec![
        AuthorAction::Steer(vec![input]),
        AuthorAction::Edit(Box::new(edit)),
    ]);
    let author = AuthorAt::new(&store, Moment::DuringRequest(1), action);
    settle(&store, &admin, &fixture.scope, &run_id, &author)
        .await
        .unwrap();
    let edited_end = (EDITED.len() + "Night came.".len()) as u32;
    let moved = range(
        &fixture,
        &head(&admin, &fixture).await,
        (1, 0),
        (2, edited_end),
    );
    let sent_moved = sent_reference(&fixture, &moved, &[1, 2], "Wind rose.\nStill, Night came.");

    // The RunStep that reads the input reads the current text of its reference.
    assert_eq!(
        (
            outcome(&store, &fixture, &run_id).await,
            step_references(&admin, &run_id).await[1].clone(),
            sent(&admin, &run_id, "1").await,
        ),
        (
            completed(),
            json!([1, [moved]]),
            json!([[sent_moved], [sent_moved]]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_run_drops_a_reference_whose_covered_text_the_author_removed_and_continues() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    // First the author joins the covered Block to the Block before it, then the author removes
    // all of its text.
    for (lead, run_prefix, change) in [(0x5b0, "5b2", "joined"), (0x5b8, "5ba", "emptied")] {
        let fixture = Fixture::new(
            &store,
            &admin,
            lead,
            DeploymentDestination::HostFakeToolLoop,
        )
        .await;
        let primitive = match change {
            "joined" => AuthorEditPrimitive::JoinBlocks {
                left_manuscript_block_id: fixture.block(0).to_owned(),
                right_manuscript_block_id: fixture.block(1).to_owned(),
            },
            _ => AuthorEditPrimitive::ReplaceBlockSelection {
                manuscript_block_id: fixture.block(1).to_owned(),
                from: 0,
                to: "Wind rose.".len() as u32,
                text: String::new(),
            },
        };
        let edit = fixture.chapter_edit(&store, 2, vec![primitive]).await;
        let reference = fixture.chapter_text(1, 1);
        let run_id = run_with_edit(
            &store,
            &admin,
            &fixture,
            run_prefix,
            MESSAGE,
            vec![reference.clone()],
            AuthorAction::Edit(Box::new(edit)),
        )
        .await;
        observed.push((
            change,
            outcome(&store, &fixture, &run_id).await,
            step_references(&admin, &run_id).await,
            sent(&admin, &run_id, "1").await,
        ));
    }
    let dropped = |change, reference: &Value| {
        (
            change,
            completed(),
            json!([[0, [reference]], [1, []]]),
            json!([[], []]),
        )
    };
    let references: Vec<Value> = observed
        .iter()
        .map(|(_, _, steps, _)| steps[0][1][0].clone())
        .collect();

    // The closing RunStep sends no reference, and the Run completes.
    assert_eq!(
        observed,
        vec![
            dropped("joined", &references[0]),
            dropped("emptied", &references[1]),
        ]
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_later_propose_edit_call_on_the_moved_reference_opens_the_proposal_on_the_current_head() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let fixture = Fixture::new(
        &store,
        &admin,
        0x7d0,
        DeploymentDestination::HostFakeToolLoop,
    )
    .await;
    let edit = fixture
        .chapter_edit(&store, 2, vec![prefix(&fixture, 0)])
        .await;
    let run_id = run_with_edit(
        &store,
        &admin,
        &fixture,
        "7d2",
        &format!("{REVISE} SCRIPT:propose_again"),
        vec![fixture.chapter_text(1, 2)],
        AuthorAction::Edit(Box::new(edit)),
    )
    .await;
    let head = head(&admin, &fixture).await;
    let bases: Value = serde_json::from_str(
        admin
            .query_one(
                "SELECT jsonb_agg(DISTINCT location ->> 'base_authoritative_revision_id')::text
                   FROM storyos.model_attempts AS attempt,
                        jsonb_array_elements(attempt.payload #> '{proposed_edit,locations}') AS location
                  WHERE attempt.run_id = $1::text::uuid",
                &[&run_id],
            )
            .await
            .unwrap()
            .get(/*idx*/ 0),
    )
    .unwrap();
    let changed_head = json!([
        0,
        "refused",
        "changed_head",
        "The Host refused this call because the Chapter changed after this request read it, \
         so no Proposal opened."
    ]);

    // The call of the first RunStep names the old head and is refused. The next RunStep reads
    // the head, and its call opens the Proposal on the head.
    assert_eq!(
        (
            propose_edit_calls(&admin, &run_id).await,
            bases,
            settled(&store, &fixture.scope, &run_id).await.0,
        ),
        (
            vec![changed_head, opened(1)],
            json!([head]),
            "Completed".to_owned()
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_removed_chapter_a_chapter_over_the_item_limit_an_invalid_range_and_the_first_run_step_keep_the_rule_of_today()
 {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    let mut expected = Vec::new();
    for (lead, run_prefix, change) in [
        (0x7d8, "7da", "removed"),
        (0x9b0, "9b2", "over_limit"),
        (0x9b8, "9ba", "first_run_step"),
        (0x6c0, "6c2", "invalid_range"),
    ] {
        let fixture = Fixture::new(
            &store,
            &admin,
            lead,
            DeploymentDestination::HostFakeToolLoop,
        )
        .await;
        let reference = match change {
            "invalid_range" => {
                let (_, fixture_head) = fixture.writer();
                range(&fixture, fixture_head, (1, 0), (2, 99))
            }
            _ => fixture.chapter_text(1, 2),
        };
        let run_id = match change {
            "removed" | "over_limit" => {
                let action = if change == "removed" {
                    let expected_tree_revision: i64 = admin
                        .query_one(
                            "SELECT tree_revision FROM storyos.projects
                              WHERE project_id = $1::text::uuid",
                            &[&fixture.scope.project_id.as_ref()],
                        )
                        .await
                        .unwrap()
                        .get(/*idx*/ 0);
                    let input = DeleteChapterInput {
                        chapter_id: ChapterId::new(fixture.chapter_id.as_str()),
                        expected_tree_revision: expected_tree_revision as u64,
                    };
                    let call =
                        named_call(&store, &fixture.scope, "7dd0", &DELETE_CHAPTER, input).await;
                    AuthorAction::DeleteChapter(Box::new(call))
                } else {
                    let primitive = AuthorEditPrimitive::ReplaceBlockSelection {
                        manuscript_block_id: fixture.block(1).to_owned(),
                        from: 0,
                        to: 0,
                        text: "a".repeat(32_001),
                    };
                    let edit = fixture.chapter_edit(&store, 2, vec![primitive]).await;
                    AuthorAction::Edit(Box::new(edit))
                };
                run_with_edit(
                    &store,
                    &admin,
                    &fixture,
                    run_prefix,
                    MESSAGE,
                    vec![reference],
                    action,
                )
                .await
            }
            _ => {
                let run_id = admit_referenced_run(
                    &store,
                    &fixture.scope,
                    &fixture.chapter_id,
                    run_prefix,
                    DeploymentDestination::HostFakeToolLoop,
                    MESSAGE,
                    vec![reference],
                )
                .await;
                // The Chapter changes after the admission of the Run, before its first request.
                if change == "first_run_step" {
                    fixture.edit_chapter(&store, 2, EDITED).await;
                }
                settle(&store, &admin, &fixture.scope, &run_id, &FakeDestination)
                    .await
                    .unwrap();
                run_id
            }
        };
        let head = head(&admin, &fixture).await;
        let references = step_references(&admin, &run_id).await;
        let admitted = references[0][1][0].clone();
        let at_head = json!(range(&fixture, &head, (1, 0), (2, 11)));
        observed.push((change, outcome(&store, &fixture, &run_id).await, references));
        expected.push(match change {
            // The next RunStep of a removed Chapter is blocked as before.
            "removed" => (change, refused(), json!([[0, [admitted]], [1, []]])),
            // The reference moves, and the item limit blocks the next RunStep.
            "over_limit" => (change, refused(), json!([[0, [admitted]], [1, [at_head]]])),
            // The first RunStep sends the reference of the Admission, and the next RunStep moves it.
            "first_run_step" => (
                change,
                completed(),
                json!([[0, [admitted]], [1, [at_head]]]),
            ),
            _ => (change, refused(), json!([[0, [admitted]]])),
        });
    }

    assert_eq!(observed, expected);
}
