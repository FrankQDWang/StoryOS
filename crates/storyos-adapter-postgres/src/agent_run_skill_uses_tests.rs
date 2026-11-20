//! The Skills that a Run used, in the Run read (ADR 0089 section 8).

use storyos_application::{AgentRunReadSelection, AgentRunReadStore, ConversationSelection};
use storyos_core::{SKILL_MD_PATH, SkillFileInput, SkillUse};

use crate::shared_tests::agent_run_finish_rule::complete;
use crate::shared_tests::agent_run_steering::queued;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::create_agent_run_skill::{fixture, package, set};
use crate::shared_tests::model_gateway::{admit_selected_run, stores};

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_run_read_names_each_selected_skill_then_each_settled_load_with_its_reason() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let outline = package(
        "plot-outline",
        vec![SkillFileInput {
            path: SKILL_MD_PATH.to_owned(),
            bytes:
                b"---\nname: plot-outline\ndescription: Use it to outline a plot.\n---\nOutline."
                    .to_vec(),
        }],
    );
    let (dialogue, comms) = (fixture("dialogue-style-guide"), fixture("internal-comms"));
    let selections = vec![comms.reference(), dialogue.reference()];
    let skills = set(vec![dialogue, comms, outline]);
    let (scope, chapter_id, first) = queued(&store, "ae1").await;
    complete(&store, &admin, &scope, &first).await;
    // The author selects two Skills out of name order. The Agent loads one Skill, and the Host
    // refuses its load of a name that the catalog does not have.
    let run_id = admit_selected_run(
        &store,
        &scope,
        &chapter_id,
        "ae2",
        "SCRIPT:load_skill plot-outline,no-such-skill",
        ConversationSelection::New,
        (skills, selections),
    )
    .await;
    complete(&store, &admin, &scope, &run_id).await;
    let read = |selection| {
        let (store, scope, run_id) = (&store, &scope, &run_id);
        async move {
            store
                .read_agent_run(scope, run_id, &selection)
                .await
                .unwrap()
                .unwrap()
        }
    };
    let run = read(AgentRunReadSelection::Current).await;
    let attempt_id = run.model.as_ref().unwrap().model_attempt_id.clone();
    let states: Vec<String> = admin
        .query(
            "SELECT state FROM storyos.tool_calls
              WHERE run_id = $1::text::uuid AND function_name = 'load_skill'
              ORDER BY decision_position, call_position",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(/*idx*/ 0))
        .collect();

    assert_eq!(
        (
            run.status,
            states,
            run.skill_uses,
            read(AgentRunReadSelection::ModelAttempt(attempt_id))
                .await
                .skill_uses,
        ),
        (
            storyos_core::AgentRunStatus::Completed,
            vec!["settled".to_owned(), "refused".to_owned()],
            Some(vec![
                SkillUse::AuthorSelected {
                    name: "dialogue-style-guide".to_owned(),
                },
                SkillUse::AuthorSelected {
                    name: "internal-comms".to_owned(),
                },
                SkillUse::AgentSelected {
                    name: "plot-outline".to_owned(),
                    reason: storyos_adapter_fake_destination::LOAD_REASON.to_owned(),
                },
            ]),
            None,
        )
    );
}
