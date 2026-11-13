//! A Run of the tool loop profile whose referenced candidate the author changes during the Run
//! (ADR 0085, ADR 0091), with the fake model of that profile.

use serde_json::{Value, json};
use storyos_core::{DeploymentDestination, SelectionReference};
use tokio_postgres::Client;

use crate::agent_run_dispatch::tests::{admit_referenced_run, stores};
use crate::agent_run_propose_edit::revise_tests::Fixture;
use crate::agent_run_propose_edit::tests::{REVISE, opened, propose_edit_calls};
use crate::agent_run_steering::tests::{AuthorAction, AuthorAt, Moment, settle, step_references};

const EDITED: &str = "Still, ";

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
        .candidate_edit(&store, &admin, &reference, 2, EDITED)
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
