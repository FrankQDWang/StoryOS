//! The working text of a Run whose ordinary passage request the Host cannot resolve (ADR 0091),
//! with the fake model of the tool loop profile.

use serde_json::Value;
use storyos_application::{AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore};
use storyos_core::DeploymentDestination;
use tokio_postgres::Client;

use crate::agent_run_dispatch::tests::{admit_run_in, queued_run_offering, stores};
use crate::agent_run_propose_edit::tests::{complete, opened, propose_edit_calls};

/// The message of Run 3 of the live run record of 2026-11-14. It has an edit word, and no clause
/// names a Chapter, so the passage request does not resolve.
const UNRESOLVED: &str = "请修改第三段：把林三敲钟的动作写得更具体，写出他的手和呼吸，\
                          并按雾航规则的意思让他一直敲下去。只改这一段，不要联网。";

/// The first Block of the head of `chapter_id`.
async fn first_block(admin: &Client, chapter_id: &str) -> String {
    admin
        .query_one(
            "SELECT member.manuscript_block_id::text
               FROM storyos.authoritative_heads AS head
               JOIN storyos.manuscript_revision_members AS member
                 ON member.revision_id = head.current_revision_id
              WHERE head.manuscript_object_id = $1::text::uuid
              ORDER BY member.block_order LIMIT 1",
            &[&chapter_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// The working text that the Context Assembly of the first RunStep selected.
async fn first_working_text(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT COALESCE(jsonb_agg(item -> 'content'), '[]')::text
               FROM storyos.context_assembly_manifests AS manifest,
                    jsonb_array_elements(manifest.payload -> 'selected') AS item
              WHERE manifest.run_id = $1::text::uuid AND manifest.decision_position = 0
                AND item ->> 'source_class' = 'working_target'",
            &[&run_id],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get(/*idx*/ 0)).unwrap()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_unresolved_passage_request_works_on_the_current_chapter() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let tool_loop = DeploymentDestination::HostFakeToolLoop;
    let (scope, chapter_id, first) =
        queued_run_offering(&store, "471", tool_loop.clone(), None).await;
    complete(&store, &admin, &scope, &first).await;
    let block = first_block(&admin, &chapter_id).await;
    // The fake proposes a change of the first Block of the Chapter, as the model of Run 3 did.
    let run_id = admit_run_in(
        &store,
        &scope,
        &chapter_id,
        "472",
        tool_loop,
        &format!("{UNRESOLVED} SCRIPT:propose_block {block}"),
        storyos_application::ConversationSelection::New,
        None,
    )
    .await;
    complete(&store, &admin, &scope, &run_id).await;
    let record = store
        .read_agent_run(&scope, &run_id, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    let opened_proposal = matches!(
        record.decision,
        AgentRunDecisionInspect::ProseChange {
            opened_proposal_id: Some(_),
            ..
        }
    );

    // The first request has the working text of a message with no passage request: the text of
    // the Current Chapter. The call on its Block opens a Proposal, and the Run shows it.
    assert_eq!(
        (
            first_working_text(&admin, &run_id).await,
            propose_edit_calls(&admin, &run_id).await,
            opened_proposal,
        ),
        (
            first_working_text(&admin, &first).await,
            vec![opened(0)],
            true
        )
    );
}
